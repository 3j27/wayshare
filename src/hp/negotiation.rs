//! AVCMediaStreamNegotiator offer parsing and answer generation (server side).
//!
//! Offer/answer = binary plist; the media blob is a zlib-compressed `VCMediaNegotiationBlob`
//! protobuf.

use std::io::{Read, Write};

use anyhow::{Context, Result, bail};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use plist::{Dictionary, Value as Plist};

use super::proto::{Writer, fields};

pub const MODE_VIDEO: i64 = 7;
pub const MODE_AUDIO: i64 = 8;
/// RTP payload type Apple's negotiator maps to HEVC (codec type 102).
pub const PAYLOAD_HEVC: u32 = 100;
/// Audio payload type Apple's negotiator selects for our answer's payload flags (codec type 6).
pub const PAYLOAD_AUDIO: u32 = 101;

const KEY_CALL_ID: &str = "avcMediaStreamOptionCallID";
const KEY_ENDPOINT: &str = "avcMediaStreamOptionRemoteEndpointInfo";
const KEY_MODE: &str = "avcMediaStreamNegotiatorMode";
const KEY_BLOB: &str = "avcMediaStreamNegotiatorMediaBlob";
const USER_AGENT: &str = "Viceroy 1.7.0";

/// `VCCallInfoBlob`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CallInfo {
    pub call_id: u32,
    pub client_version: u32,
    pub device_type: String,
    pub framework_version: String,
    pub os_version: String,
}

impl CallInfo {
    pub fn parse(buf: &[u8]) -> Result<Self> {
        let mut c = CallInfo::default();
        for (f, v) in fields(buf)? {
            match f {
                1 => c.call_id = v.u32()?,
                2 => c.client_version = v.u32()?,
                3 => c.device_type = v.string()?,
                4 => c.framework_version = v.string()?,
                5 => c.os_version = v.string()?,
                _ => {}
            }
        }
        Ok(c)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.uint(1, self.call_id)
            .uint(2, self.client_version)
            .bytes(3, self.device_type.as_bytes())
            .bytes(4, self.framework_version.as_bytes())
            .bytes(5, self.os_version.as_bytes());
        w.0
    }
}

/// `VCMediaNegotiationBlobVideoRuleCollection`.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleCollection {
    pub transport: u32,
    pub operation: u32,
    pub formats: u32,
    pub preferred_format: Option<u32>,
}

/// `VCMediaNegotiationBlobVideoPayloadSettings`.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoPayload {
    pub payload: u32,
    pub rules: Vec<RuleCollection>,
    pub feature_string: String,
    pub parameter_set: u32,
}

/// `VCMediaNegotiationBlobVideoSettings` (used for both `videoSettings` and `screenSettings`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VideoSettings {
    pub rtp_ssrc: u32,
    pub allow_rtcp_fb: bool,
    pub payloads: Vec<VideoPayload>,
    pub custom_width: Option<u32>,
    pub custom_height: Option<u32>,
    pub tiles_per_frame: Option<u32>,
    pub ltrp_enabled: Option<bool>,
    pub pixel_formats: Option<u32>,
    pub hdr_modes_supported: Option<u32>,
    pub fec_enabled: Option<bool>,
    pub rtx_enabled: Option<bool>,
    pub black_frame_on_clear_screen: Option<bool>,
}

/// `VCMediaNegotiationBlobAudioSettings`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AudioSettings {
    pub rtp_ssrc: u32,
    pub audio_unit_model: Option<u32>,
    pub support_flags: u32,
    pub payload_flags: u32,
    pub secondary_flags: u32,
    pub use_sbr: bool,
}

/// `VCMediaNegotiationBlobBandwidthSettings`.
#[derive(Debug, Clone, PartialEq)]
pub struct Bandwidth {
    pub configuration: u32,
    pub max_bandwidth: u32,
    pub extension: Option<u32>,
}

/// The subset of `VCMediaNegotiationBlob` used by Screen Sharing.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MediaBlob {
    pub allow_dynamic_max_bitrate: Option<bool>,
    pub allows_contents_change: Option<bool>,
    pub audio: Option<AudioSettings>,
    pub video: Option<VideoSettings>,
    pub screen: Option<VideoSettings>,
    pub user_agent: Option<String>,
    pub baseband_codec_sample_rate: Option<u32>,
    pub bandwidth: Vec<Bandwidth>,
    /// `momentsSettings` (capabilities, multiwayCapabilities).
    pub moments: Option<(u32, Option<u32>)>,
    pub ntp_time: Option<u64>,
    pub blob_version: Option<u32>,
    pub media_control_info_version: Option<u32>,
}

impl RuleCollection {
    fn parse(buf: &[u8]) -> Result<Self> {
        let mut r = RuleCollection {
            transport: 0,
            operation: 0,
            formats: 0,
            preferred_format: None,
        };
        for (f, v) in fields(buf)? {
            match f {
                1 => r.transport = v.u32()?,
                2 => r.operation = v.u32()?,
                3 => r.formats = v.u32()?,
                4 => r.preferred_format = Some(v.u32()?),
                _ => {}
            }
        }
        Ok(r)
    }

    fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.uint(1, self.transport)
            .uint(2, self.operation)
            .uint(3, self.formats);
        w.opt_uint(4, self.preferred_format);
        w.0
    }
}

impl VideoPayload {
    fn parse(buf: &[u8]) -> Result<Self> {
        let mut p = VideoPayload {
            payload: 0,
            rules: vec![],
            feature_string: String::new(),
            parameter_set: 0,
        };
        for (f, v) in fields(buf)? {
            match f {
                1 => p.payload = v.u32()?,
                2 => p.rules.push(RuleCollection::parse(v.bytes()?)?),
                3 => p.feature_string = v.string()?,
                4 => p.parameter_set = v.u32()?,
                _ => {}
            }
        }
        Ok(p)
    }

    fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.uint(1, self.payload);
        for r in &self.rules {
            w.bytes(2, &r.encode());
        }
        w.bytes(3, self.feature_string.as_bytes())
            .uint(4, self.parameter_set);
        w.0
    }
}

impl VideoSettings {
    fn parse(buf: &[u8]) -> Result<Self> {
        let mut s = VideoSettings::default();
        for (f, v) in fields(buf)? {
            match f {
                1 => s.rtp_ssrc = v.u32()?,
                2 => s.allow_rtcp_fb = v.bool()?,
                3 => s.payloads.push(VideoPayload::parse(v.bytes()?)?),
                4 => s.custom_width = Some(v.u32()?),
                5 => s.custom_height = Some(v.u32()?),
                6 => s.tiles_per_frame = Some(v.u32()?),
                7 => s.ltrp_enabled = Some(v.bool()?),
                8 => s.pixel_formats = Some(v.u32()?),
                9 => s.hdr_modes_supported = Some(v.u32()?),
                10 => s.fec_enabled = Some(v.bool()?),
                11 => s.rtx_enabled = Some(v.bool()?),
                12 => s.black_frame_on_clear_screen = Some(v.bool()?),
                _ => {}
            }
        }
        Ok(s)
    }

    fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.uint(1, self.rtp_ssrc).bool(2, self.allow_rtcp_fb);
        for p in &self.payloads {
            w.bytes(3, &p.encode());
        }
        w.opt_uint(4, self.custom_width)
            .opt_uint(5, self.custom_height)
            .opt_uint(6, self.tiles_per_frame)
            .opt_bool(7, self.ltrp_enabled)
            .opt_uint(8, self.pixel_formats)
            .opt_uint(9, self.hdr_modes_supported)
            .opt_bool(10, self.fec_enabled)
            .opt_bool(11, self.rtx_enabled)
            .opt_bool(12, self.black_frame_on_clear_screen);
        w.0
    }
}

impl AudioSettings {
    fn parse(buf: &[u8]) -> Result<Self> {
        let mut a = AudioSettings::default();
        for (f, v) in fields(buf)? {
            match f {
                1 => a.rtp_ssrc = v.u32()?,
                2 => a.audio_unit_model = Some(v.u32()?),
                3 => a.support_flags = v.u32()?,
                4 => a.payload_flags = v.u32()?,
                5 => a.secondary_flags = v.u32()?,
                6 => a.use_sbr = v.bool()?,
                _ => {}
            }
        }
        Ok(a)
    }

    fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.uint(1, self.rtp_ssrc).opt_uint(2, self.audio_unit_model);
        w.uint(3, self.support_flags)
            .uint(4, self.payload_flags)
            .uint(5, self.secondary_flags)
            .bool(6, self.use_sbr);
        w.0
    }
}

impl MediaBlob {
    pub fn parse(buf: &[u8]) -> Result<Self> {
        let mut b = MediaBlob::default();
        for (f, v) in fields(buf)? {
            match f {
                1 => b.allow_dynamic_max_bitrate = Some(v.bool()?),
                2 => b.allows_contents_change = Some(v.bool()?),
                3 => b.audio = Some(AudioSettings::parse(v.bytes()?)?),
                4 => b.video = Some(VideoSettings::parse(v.bytes()?)?),
                5 => b.screen = Some(VideoSettings::parse(v.bytes()?)?),
                6 => b.user_agent = Some(v.string()?),
                8 => b.baseband_codec_sample_rate = Some(v.u32()?),
                9 => {
                    let mut bw = Bandwidth {
                        configuration: 0,
                        max_bandwidth: 0,
                        extension: None,
                    };
                    for (f, v) in fields(v.bytes()?)? {
                        match f {
                            1 => bw.configuration = v.u32()?,
                            2 => bw.max_bandwidth = v.u32()?,
                            3 => bw.extension = Some(v.u32()?),
                            _ => {}
                        }
                    }
                    b.bandwidth.push(bw);
                }
                12 => {
                    let (mut caps, mut multiway) = (0, None);
                    for (f, v) in fields(v.bytes()?)? {
                        match f {
                            1 => caps = v.u32()?,
                            4 => multiway = Some(v.u32()?),
                            _ => {}
                        }
                    }
                    b.moments = Some((caps, multiway));
                }
                13 => b.ntp_time = Some(v.u64()?),
                14 => b.blob_version = Some(v.u32()?),
                16 => b.media_control_info_version = Some(v.u32()?),
                _ => {}
            }
        }
        Ok(b)
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::default();
        w.opt_bool(1, self.allow_dynamic_max_bitrate)
            .opt_bool(2, self.allows_contents_change);
        if let Some(a) = &self.audio {
            w.bytes(3, &a.encode());
        }
        if let Some(v) = &self.video {
            w.bytes(4, &v.encode());
        }
        if let Some(s) = &self.screen {
            w.bytes(5, &s.encode());
        }
        if let Some(ua) = &self.user_agent {
            w.bytes(6, ua.as_bytes());
        }
        w.opt_uint(8, self.baseband_codec_sample_rate);
        for bw in &self.bandwidth {
            let mut m = Writer::default();
            m.uint(1, bw.configuration)
                .uint(2, bw.max_bandwidth)
                .opt_uint(3, bw.extension);
            w.bytes(9, &m.0);
        }
        if let Some((caps, multiway)) = self.moments {
            let mut m = Writer::default();
            m.uint(1, caps).opt_uint(4, multiway);
            w.bytes(12, &m.0);
        }
        if let Some(t) = self.ntp_time {
            w.uint(13, t);
        }
        w.opt_uint(14, self.blob_version)
            .opt_uint(16, self.media_control_info_version);
        w.0
    }
}

/// A client offer (one per stream: audio mode 8, video modes 7).
#[derive(Debug, Clone, PartialEq)]
pub struct Offer {
    pub call_id: Option<String>,
    pub mode: i64,
    pub remote: CallInfo,
    pub blob: MediaBlob,
}

impl Offer {
    pub fn parse(bplist: &[u8]) -> Result<Self> {
        let dict = Plist::from_reader(std::io::Cursor::new(bplist))
            .context("offer is not a plist")?
            .into_dictionary()
            .context("offer plist is not a dictionary")?;
        let data = |k: &str| {
            dict.get(k)
                .and_then(Plist::as_data)
                .with_context(|| format!("offer missing {k}"))
        };
        let mut inflated = Vec::new();
        ZlibDecoder::new(data(KEY_BLOB)?)
            .read_to_end(&mut inflated)
            .context("media blob is not zlib")?;
        Ok(Offer {
            call_id: dict
                .get(KEY_CALL_ID)
                .and_then(Plist::as_string)
                .map(str::to_owned),
            mode: dict
                .get(KEY_MODE)
                .and_then(Plist::as_signed_integer)
                .with_context(|| format!("offer missing {KEY_MODE}"))?,
            remote: CallInfo::parse(data(KEY_ENDPOINT)?)?,
            blob: MediaBlob::parse(&inflated)?,
        })
    }

    /// SSRC the viewer sends from (its RTCP for video, its RTP for audio).
    pub fn remote_ssrc(&self) -> Option<u32> {
        match self.mode {
            MODE_AUDIO => self.blob.audio.as_ref().map(|a| a.rtp_ssrc),
            _ => self.blob.screen.as_ref().map(|s| s.rtp_ssrc),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AnswerParams {
    /// Our SSRC. For video, tile `i` is sent from `ssrc + i`.
    pub ssrc: u32,
    /// Endpoint info sent back. `None` echoes the viewer's own.
    pub local: Option<CallInfo>,
    /// Full video frame size (all tiles stacked). Ignored for audio.
    pub stream_size: (u32, u32),
}

#[derive(Debug, Clone)]
pub struct Answer {
    /// Binary plist to put in the 1010 type-2 message.
    pub bytes: Vec<u8>,
    pub ssrc: u32,
    pub remote_ssrc: u32,
    pub payload_type: u8,
    pub tiles: u32,
}

/// Builds an answer shaped like the one Apple's own negotiator produces for `offer`.
pub fn answer(offer: &Offer, params: &AnswerParams) -> Result<Answer> {
    let o = &offer.blob;
    let mut blob = MediaBlob {
        allow_dynamic_max_bitrate: Some(true),
        allows_contents_change: Some(true),
        user_agent: Some(USER_AGENT.into()),
        baseband_codec_sample_rate: Some(0),
        bandwidth: o.bandwidth.clone(),
        moments: Some((4, Some(4))),
        blob_version: Some(2),
        media_control_info_version: Some(0),
        ..Default::default()
    };
    let (payload_type, tiles) = match offer.mode {
        MODE_VIDEO => {
            let s = o
                .screen
                .as_ref()
                .context("video offer has no screenSettings")?;
            let hevc = s
                .payloads
                .iter()
                .find(|p| p.payload == PAYLOAD_HEVC)
                .context("offer has no HEVC payload")?;
            let rules = [1, 2].map(|operation| RuleCollection {
                transport: 1,
                operation,
                formats: 128,
                preferred_format: Some(0),
            });
            let tiles = s.tiles_per_frame.unwrap_or(1);
            blob.audio = Some(AudioSettings {
                audio_unit_model: Some(0),
                use_sbr: true,
                ..Default::default()
            });
            blob.screen = Some(VideoSettings {
                rtp_ssrc: params.ssrc,
                allow_rtcp_fb: false,
                payloads: vec![VideoPayload {
                    rules: rules.to_vec(),
                    ..hevc.clone()
                }],
                custom_width: Some(params.stream_size.0),
                custom_height: Some(params.stream_size.1),
                tiles_per_frame: Some(tiles),
                ltrp_enabled: s.ltrp_enabled,
                pixel_formats: s.pixel_formats,
                hdr_modes_supported: s.hdr_modes_supported,
                black_frame_on_clear_screen: s.black_frame_on_clear_screen,
                ..Default::default()
            });
            (PAYLOAD_HEVC as u8, tiles)
        }
        MODE_AUDIO => {
            let a = o
                .audio
                .as_ref()
                .context("audio offer has no audioSettings")?;
            blob.audio = Some(AudioSettings {
                rtp_ssrc: params.ssrc,
                audio_unit_model: Some(0),
                support_flags: 0,
                payload_flags: a.payload_flags & 0x1600,
                secondary_flags: 0x600,
                use_sbr: true,
            });
            (PAYLOAD_AUDIO as u8, 1)
        }
        m => bail!("unsupported negotiator mode {m}"),
    };

    let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
    z.write_all(&blob.encode())?;
    let local = params.local.as_ref().unwrap_or(&offer.remote);
    let mut dict = Dictionary::new();
    dict.insert(KEY_BLOB.into(), Plist::Data(z.finish()?));
    dict.insert(KEY_ENDPOINT.into(), Plist::Data(local.encode()));
    let mut bytes = Vec::new();
    Plist::Dictionary(dict).to_writer_binary(&mut bytes)?;
    Ok(Answer {
        bytes,
        ssrc: params.ssrc,
        remote_ssrc: offer.remote_ssrc().unwrap_or(0),
        payload_type,
        tiles,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OFFER7: &[u8] = include_bytes!("../../tests/fixtures/offer7.bin");
    const ANSWER7: &[u8] = include_bytes!("../../tests/fixtures/answer7.bin");

    fn sample_blob(bplist: &[u8]) -> Vec<u8> {
        let d = Plist::from_reader(std::io::Cursor::new(bplist))
            .unwrap()
            .into_dictionary()
            .unwrap();
        let mut out = Vec::new();
        ZlibDecoder::new(d[KEY_BLOB].as_data().unwrap())
            .read_to_end(&mut out)
            .unwrap();
        out
    }

    #[test]
    fn parse_offer7() {
        let o = Offer::parse(OFFER7).unwrap();
        assert_eq!(o.mode, MODE_VIDEO);
        assert_eq!(o.remote.device_type, "Mac14,10");
        assert_eq!(o.remote.os_version, "26A5425a");
        let s = o.blob.screen.as_ref().unwrap();
        assert_eq!(s.tiles_per_frame, Some(4));
        assert_eq!(s.pixel_formats, Some(255));
        assert_eq!(
            s.payloads.iter().map(|p| p.payload).collect::<Vec<_>>(),
            [123, 100]
        );
        assert_eq!(s.payloads[1].parameter_set, 14);
        assert_eq!(s.payloads[0].rules.len(), 4);
        assert_eq!(o.blob.bandwidth.len(), 10);
        assert_eq!(o.blob.blob_version, Some(2));
        assert_eq!(o.remote_ssrc(), Some(s.rtp_ssrc));
    }

    #[test]
    fn reencode_is_lossless() {
        for sample in [OFFER7, ANSWER7] {
            let raw = sample_blob(sample);
            assert_eq!(MediaBlob::parse(&raw).unwrap().encode(), raw);
        }
        let ci = CallInfo::parse(&Offer::parse(OFFER7).unwrap().remote.encode()).unwrap();
        assert_eq!(ci.framework_version, "2235.63.5.2");
    }

    #[test]
    fn answer_matches_apple_shape() {
        let o = Offer::parse(OFFER7).unwrap();
        let a = answer(
            &o,
            &AnswerParams {
                ssrc: 0x1234_5678,
                local: None,
                stream_size: (1920, 1080),
            },
        )
        .unwrap();
        assert_eq!((a.payload_type, a.tiles), (100, 4));
        let mine = MediaBlob::parse(&sample_blob(&a.bytes)).unwrap();
        let apple = MediaBlob::parse(&sample_blob(ANSWER7)).unwrap();
        let (ms, as_) = (
            mine.screen.as_ref().unwrap(),
            apple.screen.as_ref().unwrap(),
        );
        assert_eq!(ms.rtp_ssrc, 0x1234_5678);
        assert_eq!(ms.payloads[0].rules, as_.payloads[0].rules);
        assert_eq!(ms.payloads[0].parameter_set, as_.payloads[0].parameter_set);
        let strip = |m: &MediaBlob| MediaBlob {
            screen: None,
            bandwidth: vec![],
            ..m.clone()
        };
        assert_eq!(strip(&mine), strip(&apple));
    }
}
