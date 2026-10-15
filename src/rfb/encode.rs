use anyhow::{Result, ensure};
use flate2::{Compress, Compression, FlushCompress};

use super::pixel::PixelFormat;
use crate::source::Frame;

pub const ENC_RAW: i32 = 0;
pub const ENC_ZLIB: i32 = 6;

pub struct Encoder {
    zlib: Compress,
    scratch: Vec<u8>,
}

impl Default for Encoder {
    fn default() -> Self {
        Self {
            zlib: Compress::new(Compression::fast(), true),
            scratch: Vec::new(),
        }
    }
}

impl Encoder {
    /// One FramebufferUpdate with a single rect, including the message header.
    pub fn update(
        &mut self,
        frame: &Frame,
        pf: &PixelFormat,
        encoding: i32,
        rect: (u16, u16, u16, u16),
    ) -> Result<Vec<u8>> {
        self.update_rects(frame, pf, encoding, &[rect])
    }

    /// One FramebufferUpdate carrying every rect in `rects`.
    pub fn update_rects(
        &mut self,
        frame: &Frame,
        pf: &PixelFormat,
        encoding: i32,
        rects: &[(u16, u16, u16, u16)],
    ) -> Result<Vec<u8>> {
        ensure!(rects.len() <= u16::MAX as usize, "too many rects");
        let pixels: usize = rects.iter().map(|r| r.2 as usize * r.3 as usize).sum();
        let mut out = Vec::with_capacity(4 + rects.len() * 16 + pixels * pf.bytes_per_pixel());
        out.extend_from_slice(&[0, 0]);
        out.extend_from_slice(&(rects.len() as u16).to_be_bytes());
        for &(x, y, w, h) in rects {
            for v in [x, y, w, h] {
                out.extend_from_slice(&v.to_be_bytes());
            }
            out.extend_from_slice(&encoding.to_be_bytes());
            match encoding {
                ENC_ZLIB => {
                    self.scratch.clear();
                    pf.convert_rect(frame, x, y, w, h, &mut self.scratch);
                    let len_at = out.len();
                    out.extend_from_slice(&[0; 4]);
                    let start = out.len();
                    self.deflate(&mut out)?;
                    let n = (out.len() - start) as u32;
                    out[len_at..len_at + 4].copy_from_slice(&n.to_be_bytes());
                }
                _ => pf.convert_rect(frame, x, y, w, h, &mut out),
            }
        }
        Ok(out)
    }

    fn deflate(&mut self, out: &mut Vec<u8>) -> Result<()> {
        let mut input = &self.scratch[..];
        loop {
            out.reserve(input.len() / 2 + 1024);
            let before = self.zlib.total_in();
            self.zlib.compress_vec(input, out, FlushCompress::Sync)?;
            let consumed = (self.zlib.total_in() - before) as usize;
            input = &input[consumed..];
            if input.is_empty() && out.len() < out.capacity() {
                return Ok(());
            }
            ensure!(
                consumed > 0 || out.len() == out.capacity(),
                "zlib made no progress"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Decompress, FlushDecompress};

    #[test]
    fn zlib_stream_persists_across_updates() {
        let frame = Frame {
            width: 64,
            height: 32,
            pixels: (0..64 * 32).collect(),
        };
        let pf = PixelFormat::NATIVE;
        let mut enc = Encoder::default();
        let mut inflate = Decompress::new(true);
        for _ in 0..3 {
            let msg = enc.update(&frame, &pf, ENC_ZLIB, (0, 0, 64, 32)).unwrap();
            assert_eq!(
                i32::from_be_bytes(msg[12..16].try_into().unwrap()),
                ENC_ZLIB
            );
            let len = u32::from_be_bytes(msg[16..20].try_into().unwrap()) as usize;
            assert_eq!(msg.len(), 20 + len);
            let mut plain = Vec::with_capacity(64 * 32 * 4 + 64);
            inflate
                .decompress_vec(&msg[20..], &mut plain, FlushDecompress::Sync)
                .unwrap();
            let mut expect = Vec::new();
            pf.convert_rect(&frame, 0, 0, 64, 32, &mut expect);
            assert_eq!(plain, expect);
        }
    }

    #[test]
    fn raw_layout() {
        let frame = Frame {
            width: 2,
            height: 1,
            pixels: vec![0x010203, 0x040506],
        };
        let msg = Encoder::default()
            .update(&frame, &PixelFormat::NATIVE, ENC_RAW, (1, 0, 1, 1))
            .unwrap();
        assert_eq!(
            msg,
            [0, 0, 0, 1, 0, 1, 0, 0, 0, 1, 0, 1, 0, 0, 0, 0, 6, 5, 4, 0]
        );
    }
}

#[cfg(test)]
mod multi_rect_tests {
    use super::*;

    #[test]
    fn two_raw_rects() {
        let frame = Frame {
            width: 2,
            height: 2,
            pixels: vec![1, 2, 3, 4],
        };
        let msg = Encoder::default()
            .update_rects(
                &frame,
                &PixelFormat::NATIVE,
                ENC_RAW,
                &[(0, 0, 1, 1), (1, 1, 1, 1)],
            )
            .unwrap();
        assert_eq!(&msg[..4], &[0, 0, 0, 2]);
        assert_eq!(msg.len(), 4 + 2 * (12 + 4));
        assert_eq!(&msg[16..20], &[1, 0, 0, 0]);
        assert_eq!(&msg[20..28], &[0, 1, 0, 1, 0, 1, 0, 1]);
        assert_eq!(&msg[32..36], &[4, 0, 0, 0]);
    }
}
