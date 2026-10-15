//! HP audio: AAC-ELD (48 kHz stereo, 480-sample frames) in raw RTP.

#[cfg(target_os = "linux")]
pub mod capture;
#[cfg(target_os = "linux")]
pub mod fdk;
pub mod rtp;
#[cfg(target_os = "linux")]
pub mod sender;
