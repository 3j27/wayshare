use anyhow::{Result, bail};

use crate::source::Frame;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixelFormat {
    pub bits_per_pixel: u8,
    pub depth: u8,
    pub big_endian: bool,
    pub true_colour: bool,
    pub red_max: u16,
    pub green_max: u16,
    pub blue_max: u16,
    pub red_shift: u8,
    pub green_shift: u8,
    pub blue_shift: u8,
}

impl PixelFormat {
    /// Matches the in-memory layout of `Frame` pixels when written little-endian.
    pub const NATIVE: Self = Self {
        bits_per_pixel: 32,
        depth: 24,
        big_endian: false,
        true_colour: true,
        red_max: 255,
        green_max: 255,
        blue_max: 255,
        red_shift: 16,
        green_shift: 8,
        blue_shift: 0,
    };

    pub fn to_bytes(&self) -> [u8; 16] {
        let mut b = [0u8; 16];
        b[0] = self.bits_per_pixel;
        b[1] = self.depth;
        b[2] = self.big_endian as u8;
        b[3] = self.true_colour as u8;
        b[4..6].copy_from_slice(&self.red_max.to_be_bytes());
        b[6..8].copy_from_slice(&self.green_max.to_be_bytes());
        b[8..10].copy_from_slice(&self.blue_max.to_be_bytes());
        b[10] = self.red_shift;
        b[11] = self.green_shift;
        b[12] = self.blue_shift;
        b
    }

    pub fn from_bytes(b: &[u8; 16]) -> Self {
        Self {
            bits_per_pixel: b[0],
            depth: b[1],
            big_endian: b[2] != 0,
            true_colour: b[3] != 0,
            red_max: u16::from_be_bytes([b[4], b[5]]),
            green_max: u16::from_be_bytes([b[6], b[7]]),
            blue_max: u16::from_be_bytes([b[8], b[9]]),
            red_shift: b[10],
            green_shift: b[11],
            blue_shift: b[12],
        }
    }

    pub fn validate(&self) -> Result<()> {
        if !self.true_colour {
            bail!("colour-map pixel formats are unsupported: {self:?}");
        }
        if !matches!(self.bits_per_pixel, 8 | 16 | 32) {
            bail!("unsupported bits-per-pixel {}", self.bits_per_pixel);
        }
        let bpp = self.bits_per_pixel as u32;
        for (shift, max) in [
            (self.red_shift, self.red_max),
            (self.green_shift, self.green_max),
            (self.blue_shift, self.blue_max),
        ] {
            if shift as u32 + (16 - max.leading_zeros()) > bpp {
                bail!("channel (shift {shift}, max {max}) does not fit in {bpp} bpp");
            }
        }
        Ok(())
    }

    pub fn bytes_per_pixel(&self) -> usize {
        self.bits_per_pixel as usize / 8
    }

    pub fn pack(&self, rgb: u32) -> u32 {
        let scale = |v: u32, max: u16| (v * max as u32 + 127) / 255;
        (scale((rgb >> 16) & 0xff, self.red_max) << self.red_shift)
            | (scale((rgb >> 8) & 0xff, self.green_max) << self.green_shift)
            | (scale(rgb & 0xff, self.blue_max) << self.blue_shift)
    }

    pub fn convert_rect(&self, frame: &Frame, x: u16, y: u16, w: u16, h: u16, out: &mut Vec<u8>) {
        let stride = frame.width as usize;
        out.reserve(w as usize * h as usize * self.bytes_per_pixel());
        for row in y as usize..(y + h) as usize {
            let start = row * stride + x as usize;
            let src = &frame.pixels[start..start + w as usize];
            if *self == Self::NATIVE {
                for &p in src {
                    out.extend_from_slice(&p.to_le_bytes());
                }
                continue;
            }
            for &p in src {
                let v = self.pack(p);
                match (self.bits_per_pixel, self.big_endian) {
                    (8, _) => out.push(v as u8),
                    (16, false) => out.extend_from_slice(&(v as u16).to_le_bytes()),
                    (16, true) => out.extend_from_slice(&(v as u16).to_be_bytes()),
                    (_, false) => out.extend_from_slice(&v.to_le_bytes()),
                    (_, true) => out.extend_from_slice(&v.to_be_bytes()),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(pixels: Vec<u32>, width: u16) -> Frame {
        let height = (pixels.len() / width as usize) as u16;
        Frame {
            width,
            height,
            pixels,
        }
    }

    #[test]
    fn bytes_round_trip() {
        let pf = PixelFormat::NATIVE;
        assert_eq!(
            pf.to_bytes(),
            [32, 24, 0, 1, 0, 255, 0, 255, 0, 255, 16, 8, 0, 0, 0, 0]
        );
        assert_eq!(PixelFormat::from_bytes(&pf.to_bytes()), pf);
    }

    #[test]
    fn native_is_bgrx_little_endian() {
        let f = frame(vec![0x112233, 0xaabbcc], 2);
        let mut out = Vec::new();
        PixelFormat::NATIVE.convert_rect(&f, 1, 0, 1, 1, &mut out);
        assert_eq!(out, [0xcc, 0xbb, 0xaa, 0x00]);
    }

    #[test]
    fn rgb565_both_endians() {
        let mut pf = PixelFormat {
            bits_per_pixel: 16,
            depth: 16,
            big_endian: false,
            true_colour: true,
            red_max: 31,
            green_max: 63,
            blue_max: 31,
            red_shift: 11,
            green_shift: 5,
            blue_shift: 0,
        };
        pf.validate().unwrap();
        let f = frame(vec![0xff0000, 0x00ff00, 0x0000ff, 0xffffff], 4);
        let mut out = Vec::new();
        pf.convert_rect(&f, 0, 0, 4, 1, &mut out);
        assert_eq!(out, [0x00, 0xf8, 0xe0, 0x07, 0x1f, 0x00, 0xff, 0xff]);
        pf.big_endian = true;
        out.clear();
        pf.convert_rect(&f, 0, 0, 1, 1, &mut out);
        assert_eq!(out, [0xf8, 0x00]);
    }

    #[test]
    fn rgb332_and_32bpp_big_endian_rgbx() {
        let pf = PixelFormat {
            bits_per_pixel: 8,
            depth: 8,
            big_endian: false,
            true_colour: true,
            red_max: 7,
            green_max: 7,
            blue_max: 3,
            red_shift: 0,
            green_shift: 3,
            blue_shift: 6,
        };
        pf.validate().unwrap();
        let f = frame(vec![0xff8000], 1);
        let mut out = Vec::new();
        pf.convert_rect(&f, 0, 0, 1, 1, &mut out);
        assert_eq!(out, [7 | (4 << 3)]);

        let pf = PixelFormat {
            bits_per_pixel: 32,
            big_endian: true,
            red_shift: 24,
            green_shift: 16,
            blue_shift: 8,
            ..PixelFormat::NATIVE
        };
        pf.validate().unwrap();
        out.clear();
        pf.convert_rect(&f, 0, 0, 1, 1, &mut out);
        assert_eq!(out, [0xff, 0x80, 0x00, 0x00]);
    }

    #[test]
    fn rejects_bad_formats() {
        let cmap = PixelFormat {
            true_colour: false,
            ..PixelFormat::NATIVE
        };
        assert!(cmap.validate().is_err());
        let overflow = PixelFormat {
            bits_per_pixel: 16,
            ..PixelFormat::NATIVE
        };
        assert!(overflow.validate().is_err());
        let odd = PixelFormat {
            bits_per_pixel: 24,
            ..PixelFormat::NATIVE
        };
        assert!(odd.validate().is_err());
    }
}
