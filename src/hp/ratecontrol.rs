//! Sender-side rate control driven by the receiver's RTCP feedback.
//!
//! The viewer runs its own bandwidth estimator and reports it in RTCP APP `RCTL` packets
//! about every 50 ms (layout in `notes/hp-impl.md`). This controller follows that estimate,
//! backs off on queueing delay (OWRD) growth and keyframe-request bursts, and ramps up
//! slowly otherwise.

use std::collections::VecDeque;
use std::time::Duration;

use super::rtcp::RtcpEvent;

/// One decoded `RCTL` payload (`VCMediaControlInfo` version 2, 20 bytes).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateFeedback {
    /// Second header byte. Loss-related (receiver stat * 1000 / 20); always 0 in our runs.
    pub loss_raw: u8,
    /// Our RTP timestamp echoed back, low precision (>> 8).
    pub echo_ts: u16,
    pub ms_since_last_rx: u16,
    /// Receiver clock in 1/1024 s, wrapping.
    pub rx_time_1024: u16,
    /// One-way relative delay in seconds (13 fractional bits on the wire, saturates at 8 s).
    pub owrd: f64,
    pub bursty_loss: u8,
    /// Received video packets, 12-bit wrapping counter.
    pub rx_packets: u16,
    /// Receiver bandwidth estimate. 0 for the first ~2.5 s (no estimate yet).
    pub bwe_bps: u32,
}

impl RateFeedback {
    /// Parses the APP data after the `RCTL` name.
    pub fn parse(data: &[u8]) -> Option<Self> {
        let d: &[u8; 20] = data.get(..20)?.try_into().ok()?;
        if d[0] & 0xc0 != 0x80 || u16::from_be_bytes([d[2], d[3]]) != 4 {
            return None;
        }
        let w = |i: usize| u16::from_be_bytes([d[2 * i], d[2 * i + 1]]);
        Some(RateFeedback {
            loss_raw: d[1],
            echo_ts: w(2),
            ms_since_last_rx: w(5),
            rx_time_1024: w(6),
            owrd: f64::from(w(7)) / 8192.0,
            bursty_loss: (w(8) >> 12) as u8,
            rx_packets: w(8) & 0x0fff,
            bwe_bps: u32::from(w(9)) * 1000,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RateConfig {
    pub min_bps: u32,
    pub start_bps: u32,
    /// Negotiated maximum (the receiver's estimate never exceeds it either).
    pub max_bps: u32,
}

impl Default for RateConfig {
    fn default() -> Self {
        RateConfig {
            min_bps: 1_000_000,
            start_bps: 20_000_000,
            max_bps: 60_000_000,
        }
    }
}

const QUEUE_HIGH: f64 = 0.100;
const QUEUE_LOW: f64 = 0.030;
const DECREASE_FACTOR: f64 = 0.85;
const BWE_HEADROOM: f64 = 0.9;
const RAMP_PER_SEC: f64 = 0.08;
const MIN_DECREASE_GAP: Duration = Duration::from_millis(300);
const FIR_WINDOW: Duration = Duration::from_secs(2);
const FIR_BURST: usize = 3;
const FIR_FACTOR: f64 = 0.7;
const FIR_SAME_EVENT: Duration = Duration::from_millis(100);
const OWRD_FLOOR_WINDOW: Duration = Duration::from_secs(10);
/// Smaller target changes are not reported, so the encoder is not reconfigured constantly.
const REPORT_STEP: f64 = 0.05;

pub struct RateController {
    cfg: RateConfig,
    target: f64,
    reported: f64,
    last_feedback: Option<Duration>,
    last_decrease: Option<Duration>,
    firs: VecDeque<Duration>,
    owrd_samples: VecDeque<(Duration, f64)>,
}

impl RateController {
    pub fn new(cfg: RateConfig) -> Self {
        let start = f64::from(cfg.start_bps.clamp(cfg.min_bps, cfg.max_bps));
        RateController {
            cfg,
            target: start,
            reported: start,
            last_feedback: None,
            last_decrease: None,
            firs: VecDeque::new(),
            owrd_samples: VecDeque::new(),
        }
    }

    pub fn target_bps(&self) -> u32 {
        self.reported as u32
    }

    /// Feeds one RTCP event received at `now` (any monotonic clock). Returns the new encoder
    /// bitrate when it changed enough to be worth applying.
    pub fn on_event(&mut self, ev: &RtcpEvent, now: Duration) -> Option<u32> {
        match ev {
            RtcpEvent::RateControl { data, .. } => {
                let fb = RateFeedback::parse(data)?;
                self.on_feedback(&fb, now);
            }
            RtcpEvent::KeyframeRequest { .. } => self.on_keyframe_request(now),
            _ => return None,
        }
        self.report()
    }

    fn on_feedback(&mut self, fb: &RateFeedback, now: Duration) {
        let dt = self
            .last_feedback
            .map_or(0.0, |t| now.saturating_sub(t).as_secs_f64().min(1.0));
        self.last_feedback = Some(now);

        while self
            .owrd_samples
            .front()
            .is_some_and(|&(t, _)| now.saturating_sub(t) > OWRD_FLOOR_WINDOW)
        {
            self.owrd_samples.pop_front();
        }
        self.owrd_samples.push_back((now, fb.owrd));
        let floor = self
            .owrd_samples
            .iter()
            .map(|&(_, d)| d)
            .fold(f64::INFINITY, f64::min);
        let queueing = fb.owrd - floor;

        let bwe = match fb.bwe_bps {
            0 => f64::from(self.cfg.max_bps),
            b => f64::from(b).clamp(f64::from(self.cfg.min_bps), f64::from(self.cfg.max_bps)),
        };
        let before = self.target;
        self.adjust(bwe, queueing, dt, now);
        tracing::debug!(
            bwe_kbps = fb.bwe_bps / 1000,
            owrd_ms = (fb.owrd * 1e3).round(),
            floor_ms = (floor * 1e3).round(),
            queueing_ms = (queueing * 1e3).round(),
            before_kbps = (before / 1e3).round(),
            target_kbps = (self.target / 1e3).round(),
            "RCTL"
        );
    }

    fn adjust(&mut self, bwe: f64, queueing: f64, dt: f64, now: Duration) {
        if queueing > QUEUE_HIGH || bwe * BWE_HEADROOM < self.target {
            if self
                .last_decrease
                .is_none_or(|t| now.saturating_sub(t) >= MIN_DECREASE_GAP)
            {
                self.decrease_to((self.target * DECREASE_FACTOR).min(bwe * BWE_HEADROOM), now);
            }
        } else if queueing < QUEUE_LOW {
            let ceiling = bwe * BWE_HEADROOM;
            self.target = (self.target * (1.0 + RAMP_PER_SEC * dt))
                .min(ceiling)
                .max(self.target);
        }
        self.clamp();
    }

    fn on_keyframe_request(&mut self, now: Duration) {
        // The receiver sends one FIR per tile for the same failure.
        if self
            .firs
            .back()
            .is_some_and(|&t| now.saturating_sub(t) < FIR_SAME_EVENT)
        {
            return;
        }
        while self
            .firs
            .front()
            .is_some_and(|&t| now.saturating_sub(t) > FIR_WINDOW)
        {
            self.firs.pop_front();
        }
        self.firs.push_back(now);
        if self.firs.len() >= FIR_BURST {
            self.firs.clear();
            self.decrease_to(self.target * FIR_FACTOR, now);
            self.clamp();
            tracing::debug!(target_kbps = (self.target / 1e3).round(), "FIR burst cut");
        }
    }

    fn decrease_to(&mut self, bps: f64, now: Duration) {
        if bps < self.target {
            self.target = bps;
            self.last_decrease = Some(now);
        }
    }

    fn clamp(&mut self) {
        self.target = self
            .target
            .clamp(f64::from(self.cfg.min_bps), f64::from(self.cfg.max_bps));
    }

    fn report(&mut self) -> Option<u32> {
        if (self.target - self.reported).abs() >= self.reported * REPORT_STEP {
            self.reported = self.target;
            Some(self.reported as u32)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hp::rtcp::parse;

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }

    /// Every 40th RCTL from an unconstrained 20 s run (~5.5 Mbit/s stream).
    const FREE: &[(u64, &str)] = &[
        (
            111,
            "80cc000723d17ae65243544c850000040009000000000006931600030046ea60",
        ),
        (
            2157,
            "80cc000723d17ae65243544c8500000400c90000000000069b4500120529ea60",
        ),
        (
            4212,
            "80cc000723d17ae65243544c85000004018900000000000aa37e00140a73ea60",
        ),
        (
            6313,
            "80cc000723d17ae65243544c85000004024e000000000009abe400120fe8ea61",
        ),
        (
            8305,
            "80cc000723d17ae65243544c85000004030a000000000001b3dc00130532ea60",
        ),
        (
            10308,
            "80cc000723d17ae65243544c8500000403c5000000000004bbe000140a02ea60",
        ),
        (
            12358,
            "80cc000723d17ae65243544c850000040485000000000006c413000f0ee4abc0",
        ),
        (
            14404,
            "80cc000723d17ae65243544c850000040544000000000012cc42001103c4abc0",
        ),
        (
            16454,
            "80cc000723d17ae65243544c850000040606000000000002d47600100933abc0",
        ),
        (
            18454,
            "80cc000723d17ae65243544c8500000406c1000000000001dc7500110e6babc0",
        ),
    ];

    /// Every 40th RCTL with the sender capped at 3 Mbit/s (queue builds, BWE ~3.07 Mbit/s).
    const CAPPED: &[(u64, &str)] = &[
        (
            150,
            "80cc00071a1f48d55243544c85000004000600000000000cf98a001400320c00",
        ),
        (
            2197,
            "80cc00071a1f48d55243544c85000004007500000000001501b9188a02fc0c01",
        ),
        (
            4245,
            "80cc00071a1f48d55243544c8500000400df00000000001809eb34a305bb0c09",
        ),
        (
            6242,
            "80cc00071a1f48d55243544c85000004014000000000001411e852b8086a0c10",
        ),
        (
            8250,
            "80cc00071a1f48d55243544c8500000401a200000000000019f071ad0b1c0c21",
        ),
        (
            10247,
            "80cc00071a1f48d55243544c85000004020500000000000b21ed8edc0dd70c39",
        ),
    ];

    fn run(seq: &[(u64, &str)], rc: &mut RateController) -> Vec<u32> {
        let mut changes = Vec::new();
        for &(ms, pkt) in seq {
            for ev in parse(&hex(pkt)).unwrap() {
                // Recorded every ~2 s; replay each sample 40 times at 50 ms like the receiver.
                for k in 0..40 {
                    let now = Duration::from_millis(ms + k * 50);
                    changes.extend(rc.on_event(&ev, now));
                }
            }
        }
        changes
    }

    #[test]
    fn parse_rctl() {
        let ev = parse(&hex(CAPPED[2].1)).unwrap();
        let RtcpEvent::RateControl { data, .. } = &ev[0] else {
            panic!("{ev:?}")
        };
        let fb = RateFeedback::parse(data).unwrap();
        assert_eq!(fb.bwe_bps, 3_081_000);
        assert_eq!(fb.echo_ts, 0xdf);
        assert!((fb.owrd - f64::from(0x34a3u16) / 8192.0).abs() < 1e-9);
        assert_eq!((fb.bursty_loss, fb.rx_packets), (0, 0x5bb));
        assert!(RateFeedback::parse(&data[..19]).is_none());
    }

    #[test]
    fn ramps_up_when_unconstrained_and_follows_bwe() {
        let mut rc = RateController::new(RateConfig::default());
        let changes = run(&FREE[..6], &mut rc);
        assert!(changes.windows(2).all(|w| w[1] > w[0]), "{changes:?}");
        let peak = rc.target_bps();
        assert!(peak > 30_000_000 && peak <= 54_000_000, "{peak}");
        run(&FREE[6..], &mut rc);
        assert!(
            rc.target_bps() <= 44_000_000 * 9 / 10 + 1,
            "follows BWE drop to 44 Mbit/s"
        );
    }

    #[test]
    fn backs_off_to_capacity() {
        let mut rc = RateController::new(RateConfig::default());
        let first = run(&CAPPED[..1], &mut rc);
        assert!(first[0] <= 3_000_000, "{first:?}");
        run(&CAPPED[1..], &mut rc);
        assert!(
            rc.target_bps() < 1_100_000,
            "the emulated link queue never drains, so OWRD keeps pushing towards the floor"
        );
    }

    #[test]
    fn zero_bwe_is_no_estimate() {
        let mut rc = RateController::new(RateConfig::default());
        let fb = RateFeedback {
            loss_raw: 0,
            echo_ts: 0,
            ms_since_last_rx: 0,
            rx_time_1024: 0,
            owrd: 0.0,
            bursty_loss: 0,
            rx_packets: 0,
            bwe_bps: 0,
        };
        for ms in (0..3000).step_by(50) {
            rc.on_feedback(&fb, Duration::from_millis(ms));
        }
        assert!(rc.target_bps() >= 20_000_000, "{}", rc.target_bps());
    }

    #[test]
    fn fir_burst_backs_off() {
        let mut rc = RateController::new(RateConfig::default());
        let fir = RtcpEvent::KeyframeRequest { ssrc: 1 };
        for ms in [0, 1, 2, 3] {
            assert_eq!(
                rc.on_event(&fir, Duration::from_millis(ms)),
                None,
                "one FIR per tile is one event"
            );
        }
        assert_eq!(rc.on_event(&fir, Duration::from_millis(500)), None);
        assert_eq!(
            rc.on_event(&fir, Duration::from_millis(900)),
            Some(14_000_000)
        );
        assert_eq!(rc.on_event(&fir, Duration::from_millis(5000)), None);
    }
}
