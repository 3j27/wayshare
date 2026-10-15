# Configuration

wayshare reads `~/.config/wayshare/config.toml` (or `$XDG_CONFIG_HOME/wayshare/config.toml`).
Every key is optional. Command-line flags override the file; `wayshare --help` lists them.

A full example with every key lives in
[`contrib/config.example.toml`](../contrib/config.example.toml), and packages install it to
`/usr/share/doc/wayshare/`.

After changing anything, restart: `systemctl --user restart wayshare`.

## The basics

```toml
output = "DP-1"          # the monitor to share; `hyprctl monitors` lists them
name = "studio"          # how the machine appears on the Mac; default: the host name
listen = "0.0.0.0:5900"
resize = true            # match the Mac's window size, restore the monitor afterwards
```

Without `output`, wayshare serves a test pattern instead of your screen.

With `resize` on, wayshare switches the shared monitor to the mode the Mac asks for while
you're connected, and puts it back when you disconnect.

## Login

`wayshare set-password --user NAME` writes `~/.config/wayshare/srp-NAME`. Run it again to
change the password. To keep the file somewhere else, set `srp_file`.

## Keyboard

```toml
[input]
enabled = true
command_key = "super"    # super | ctrl | alt | altgr
option_key = "alt"
layout = "us"
variant = ""
```

`command_key` decides what the Mac's Command key sends. `super` suits Omarchy, whose
shortcuts live on Super.

## High Performance

```toml
[hp]
enabled = true
port = 5960              # video; audio uses port + 2
encoder = "auto"         # auto | vaapi | x265
chroma = "420"           # "444" needs encoder = "x265"
audio = true
# bitrate = 40000000
```

See [High Performance mode](high-performance.md).

## Finder sidebar

```toml
[mdns]
enabled = true
# device_model = "MacPro7,1"   # picks the icon Finder shows
```
