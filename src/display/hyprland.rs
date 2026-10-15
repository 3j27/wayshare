//! Hyprland IPC (`$XDG_RUNTIME_DIR/hypr/$HYPRLAND_INSTANCE_SIGNATURE/.socket.sock`).

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail, ensure};
use serde_json::Value;
use tracing::{debug, warn};

use super::{DisplayManager, DisplayMode, OutputInfo};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfigKind {
    Unknown,
    /// hyprlang: `keyword monitor ...`.
    Legacy,
    /// Lua config (0.55+): `keyword` is refused, use `eval hl.monitor{...}`.
    Lua,
}

pub struct Hyprland {
    socket: PathBuf,
    config: ConfigKind,
}

impl Hyprland {
    pub fn from_env() -> Result<Self> {
        let runtime = std::env::var_os("XDG_RUNTIME_DIR").context("XDG_RUNTIME_DIR not set")?;
        let hypr = Path::new(&runtime).join("hypr");
        let socket = match std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
            Ok(sig) if !sig.is_empty() => hypr.join(sig).join(".socket.sock"),
            _ => newest_live_socket(&hypr)?,
        };
        Ok(Self::with_socket(socket))
    }

    pub fn with_socket(socket: PathBuf) -> Self {
        Self { socket, config: ConfigKind::Unknown }
    }

    pub fn request(&self, cmd: &str) -> Result<String> {
        let mut s = UnixStream::connect(&self.socket)
            .with_context(|| format!("connect {}", self.socket.display()))?;
        s.set_read_timeout(Some(Duration::from_secs(5)))?;
        s.write_all(cmd.as_bytes())?;
        let mut out = String::new();
        s.read_to_string(&mut out)?;
        debug!(cmd, reply = out.trim(), "hyprctl");
        Ok(out)
    }

    fn ok(&self, cmd: &str) -> Result<()> {
        let r = self.request(cmd)?;
        ensure!(r.trim() == "ok", "hyprland `{cmd}`: {}", r.trim());
        Ok(())
    }

    fn apply_rule(&mut self, output: &str, mode: &DisplayMode, position: &str) -> Result<()> {
        let m = format!("{}x{}@{}", mode.width, mode.height, fmt_num(mode.refresh_hz));
        let scale = fmt_num(mode.scale);
        if self.config != ConfigKind::Lua {
            let r = self.request(&format!("keyword monitor {output},{m},{position},{scale}"))?;
            if r.trim() == "ok" {
                self.config = ConfigKind::Legacy;
                return Ok(());
            }
            ensure!(r.contains("non-legacy"), "hyprland keyword monitor: {}", r.trim());
            self.config = ConfigKind::Lua;
        }
        self.ok(&lua_monitor_rule(output, &m, position, &scale))
    }
}

fn newest_live_socket(hypr: &Path) -> Result<PathBuf> {
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for e in std::fs::read_dir(hypr).with_context(|| format!("read {}", hypr.display()))? {
        let sock = e?.path().join(".socket.sock");
        let Ok(meta) = std::fs::metadata(&sock) else { continue };
        if UnixStream::connect(&sock).is_err() {
            continue;
        }
        let t = meta.modified()?;
        if best.as_ref().is_none_or(|(bt, _)| t > *bt) {
            best = Some((t, sock));
        }
    }
    best.map(|(_, p)| p).ok_or_else(|| anyhow!("no running Hyprland instance under {}", hypr.display()))
}

fn check_name(name: &str) -> Result<()> {
    ensure!(
        !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')),
        "invalid output name {name:?}"
    );
    Ok(())
}

fn fmt_num(v: f64) -> String {
    let s = format!("{v:.5}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn lua_monitor_rule(output: &str, mode: &str, position: &str, scale: &str) -> String {
    format!("eval hl.monitor({{ output = \"{output}\", mode = \"{mode}\", position = \"{position}\", scale = {scale} }})")
}

pub fn parse_monitors(json: &str) -> Result<Vec<OutputInfo>> {
    let v: Value = serde_json::from_str(json).context("monitors JSON")?;
    let arr = v.as_array().ok_or_else(|| anyhow!("monitors: not an array"))?;
    arr.iter()
        .map(|m| {
            let u = |k: &str| m[k].as_u64().ok_or_else(|| anyhow!("monitors: missing {k}"));
            let i = |k: &str| m[k].as_i64().ok_or_else(|| anyhow!("monitors: missing {k}"));
            let f = |k: &str| m[k].as_f64().ok_or_else(|| anyhow!("monitors: missing {k}"));
            Ok(OutputInfo {
                name: m["name"].as_str().ok_or_else(|| anyhow!("monitors: missing name"))?.to_string(),
                width: u("width")? as u32,
                height: u("height")? as u32,
                refresh_hz: f("refreshRate")?,
                scale: f("scale")?,
                x: i("x")? as i32,
                y: i("y")? as i32,
                focused: m["focused"].as_bool().unwrap_or(false),
                disabled: m["disabled"].as_bool().unwrap_or(false),
            })
        })
        .collect()
}

impl DisplayManager for Hyprland {
    fn outputs(&mut self) -> Result<Vec<OutputInfo>> {
        parse_monitors(&self.request("j/monitors all")?)
    }

    fn set_mode(&mut self, output: &str, mode: &DisplayMode) -> Result<OutputInfo> {
        check_name(output)?;
        let cur = self
            .outputs()?
            .into_iter()
            .find(|o| o.name == output)
            .ok_or_else(|| anyhow!("no output {output}"))?;
        self.apply_rule(output, mode, &format!("{}x{}", cur.x, cur.y))?;
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let now = self.outputs()?.into_iter().find(|o| o.name == output);
            if let Some(o) = &now
                && o.width == mode.width
                && o.height == mode.height
            {
                if (o.scale - mode.scale).abs() > 1e-3 {
                    warn!(output, requested = mode.scale, actual = o.scale, "compositor changed scale");
                }
                return Ok(o.clone());
            }
            if Instant::now() > deadline {
                bail!("{output} did not switch to {}x{} (now {:?})", mode.width, mode.height, now.map(|o| o.mode()));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn create_headless(&mut self, name: &str) -> Result<()> {
        check_name(name)?;
        self.ok(&format!("output create headless {name}"))
    }

    fn remove_output(&mut self, name: &str) -> Result<()> {
        check_name(name)?;
        self.ok(&format!("output remove {name}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MONITORS: &str = r#"[{
        "id": 0, "name": "HEADLESS-1", "description": "", "width": 3456, "height": 2160,
        "refreshRate": 60.00000, "x": 0, "y": 0, "scale": 2, "focused": true, "disabled": false,
        "availableModes": ["1920x1080@60.00Hz"]
    },{
        "id": 1, "name": "ARDTEST-1", "width": 2001, "height": 1333, "refreshRate": 30.0,
        "x": 1728, "y": 0, "scale": 1, "focused": false, "disabled": false
    }]"#;

    #[test]
    fn parses_monitors() {
        let m = parse_monitors(MONITORS).unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].mode(), DisplayMode { width: 3456, height: 2160, refresh_hz: 60.0, scale: 2.0 });
        assert!(m[0].focused);
        assert_eq!((m[1].name.as_str(), m[1].x), ("ARDTEST-1", 1728));
        assert!(parse_monitors("{}").is_err());
    }

    #[test]
    fn commands() {
        assert_eq!(fmt_num(60.0), "60");
        assert_eq!(fmt_num(59.94), "59.94");
        assert_eq!(fmt_num(1.25), "1.25");
        assert_eq!(
            lua_monitor_rule("ARDTEST-1", "2940x1912@60", "0x0", "2"),
            r#"eval hl.monitor({ output = "ARDTEST-1", mode = "2940x1912@60", position = "0x0", scale = 2 })"#
        );
        assert!(check_name("HEADLESS-1").is_ok());
        assert!(check_name("x\" }) os.execute(\"").is_err());
    }

    #[test]
    fn talks_to_a_fake_socket() {
        use std::os::unix::net::UnixListener;
        let dir = std::env::temp_dir().join(format!("ardd-hypr-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(".socket.sock");
        let _ = std::fs::remove_file(&path);
        let l = UnixListener::bind(&path).unwrap();
        let t = std::thread::spawn(move || {
            let (mut c, _) = l.accept().unwrap();
            let mut buf = [0u8; 64];
            let n = c.read(&mut buf).unwrap();
            assert_eq!(&buf[..n], b"j/monitors all");
            c.write_all(MONITORS.as_bytes()).unwrap();
        });
        let mut h = Hyprland::with_socket(path.clone());
        assert_eq!(h.outputs().unwrap().len(), 2);
        t.join().unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
