use anyhow::Result;
use flate2::{Compress, Compression, FlushCompress};

use super::pixel::PixelFormat;
use crate::source::{CursorImage, Frame};

pub const ENC_DESKTOP_SIZE: i32 = -223;
pub const ENC_RICH_CURSOR: i32 = -239;
pub const ENC_CURSOR_POSITION: i32 = 1100;
pub const ENC_CURSOR_ALPHA: i32 = 1104;
pub const ENC_DEVICE_INFO: i32 = 1110;
pub const ENC_DISPLAY_INFO: i32 = 1101;
pub const ENC_DISPLAY_INFO2: i32 = 1105;

pub const FIRST_CURSOR_ID: u32 = 1000;

fn rect_header(out: &mut Vec<u8>, x: u16, y: u16, w: u16, h: u16, encoding: i32) {
    for v in [x, y, w, h] {
        out.extend_from_slice(&v.to_be_bytes());
    }
    out.extend_from_slice(&encoding.to_be_bytes());
}

/// A FramebufferUpdate with a single pseudo-encoding rect.
pub fn single_rect(x: u16, y: u16, w: u16, h: u16, encoding: i32, payload: &[u8]) -> Vec<u8> {
    let mut out = vec![0, 0, 0, 1];
    rect_header(&mut out, x, y, w, h, encoding);
    out.extend_from_slice(payload);
    out
}

pub fn desktop_size(w: u16, h: u16) -> Vec<u8> {
    single_rect(0, 0, w, h, ENC_DESKTOP_SIZE, &[])
}

pub fn cursor_position(x: u16, y: u16) -> Vec<u8> {
    single_rect(x, y, 0, 0, ENC_CURSOR_POSITION, &[])
}

fn cursor_as_frame(c: &CursorImage) -> Frame {
    Frame {
        width: c.width,
        height: c.height,
        pixels: c.pixels.iter().map(|p| p & 0x00ff_ffff).collect(),
    }
}

/// Encoding 1104. `cached` sends just the ID, for an image the client already holds.
pub fn cursor_alpha(c: &CursorImage, id: u32, pf: &PixelFormat, cached: bool) -> Result<Vec<u8>> {
    let mut payload = id.to_be_bytes().to_vec();
    if cached {
        payload.extend_from_slice(&0u32.to_be_bytes());
    } else {
        let mut raw = Vec::new();
        pf.convert_rect(&cursor_as_frame(c), 0, 0, c.width, c.height, &mut raw);
        raw.extend(c.pixels.iter().map(|p| (p >> 24) as u8));
        let z = zlib_sync_flushed(&raw)?;
        payload.extend_from_slice(&(z.len() as u32).to_be_bytes());
        payload.extend_from_slice(&z);
    }
    Ok(single_rect(
        c.hotspot.0,
        c.hotspot.1,
        c.width,
        c.height,
        ENC_CURSOR_ALPHA,
        &payload,
    ))
}

/// A zlib stream ended with a sync flush but no final block: the client inflates with
/// Z_SYNC_FLUSH and treats Z_STREAM_END as an error.
fn zlib_sync_flushed(raw: &[u8]) -> Result<Vec<u8>> {
    let mut z = Compress::new(Compression::default(), true);
    let mut out = Vec::with_capacity(raw.len() / 2 + 64);
    let mut input = raw;
    loop {
        out.reserve(input.len() + 64);
        let before = z.total_in();
        z.compress_vec(input, &mut out, FlushCompress::Sync)?;
        input = &input[(z.total_in() - before) as usize..];
        if input.is_empty() && out.len() < out.capacity() {
            return Ok(out);
        }
    }
}

/// Encoding -239 fallback: pixels in the client format plus a 1-bit mask (alpha >= 128).
pub fn rich_cursor(c: &CursorImage, pf: &PixelFormat) -> Vec<u8> {
    let mut payload = Vec::new();
    pf.convert_rect(&cursor_as_frame(c), 0, 0, c.width, c.height, &mut payload);
    let row_bytes = (c.width as usize).div_ceil(8);
    for row in c.pixels.chunks(c.width.max(1) as usize) {
        let mut mask = vec![0u8; row_bytes];
        for (i, p) in row.iter().enumerate() {
            if p >> 24 >= 0x80 {
                mask[i / 8] |= 0x80 >> (i % 8);
            }
        }
        payload.extend_from_slice(&mask);
    }
    single_rect(
        c.hotspot.0,
        c.hotspot.1,
        c.width,
        c.height,
        ENC_RICH_CURSOR,
        &payload,
    )
}

/// Encoding 1110, message version 1. Strings are sent NUL-terminated.
pub fn device_info(identifier: &str, color: &str, enclosure_color: &str) -> Vec<u8> {
    let strings: Vec<Vec<u8>> = [identifier, color, enclosure_color]
        .iter()
        .map(|s| {
            let mut v = s.as_bytes().to_vec();
            v.push(0);
            v
        })
        .collect();
    let mut body = Vec::new();
    body.extend_from_slice(&1u16.to_be_bytes());
    body.extend_from_slice(&0u32.to_be_bytes());
    body.extend_from_slice(&[0; 4]);
    for s in &strings {
        body.extend_from_slice(&(s.len() as u16).to_be_bytes());
    }
    for s in &strings {
        body.extend_from_slice(s);
    }
    let mut payload = (body.len() as u16).to_be_bytes().to_vec();
    payload.extend_from_slice(&body);
    single_rect(0, 0, 0, 0, ENC_DEVICE_INFO, &payload)
}

/// Server message 3.
pub fn server_cut_text(text: &[u8]) -> Vec<u8> {
    let mut out = vec![3, 0, 0, 0];
    out.extend_from_slice(&(text.len() as u32).to_be_bytes());
    out.extend_from_slice(text);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cursor() -> CursorImage {
        CursorImage {
            serial: 1,
            width: 3,
            height: 2,
            hotspot: (1, 0),
            pixels: vec![
                0xff112233, 0x00000000, 0x80aabbcc, 0xff000000, 0x7fffffff, 0xffffffff,
            ],
        }
    }

    #[test]
    fn cursor_alpha_layout() {
        let msg = cursor_alpha(&cursor(), 1000, &PixelFormat::NATIVE, false).unwrap();
        assert_eq!(&msg[..4], &[0, 0, 0, 1]);
        assert_eq!(&msg[4..12], &[0, 1, 0, 0, 0, 3, 0, 2]);
        assert_eq!(i32::from_be_bytes(msg[12..16].try_into().unwrap()), 1104);
        assert_eq!(&msg[16..20], &1000u32.to_be_bytes());
        let zlen = u32::from_be_bytes(msg[20..24].try_into().unwrap()) as usize;
        assert_eq!(msg.len(), 24 + zlen);
        let mut inflate = flate2::Decompress::new(true);
        let mut raw = Vec::with_capacity(64);
        let status = inflate
            .decompress_vec(&msg[24..], &mut raw, flate2::FlushDecompress::Sync)
            .unwrap();
        assert_eq!(status, flate2::Status::Ok, "must not be a finished stream");
        assert_eq!(raw.len(), 6 * 4 + 6);
        assert_eq!(&raw[..4], &[0x33, 0x22, 0x11, 0x00]);
        assert_eq!(&raw[24..], &[0xff, 0x00, 0x80, 0xff, 0x7f, 0xff]);

        let cached = cursor_alpha(&cursor(), 1000, &PixelFormat::NATIVE, true).unwrap();
        assert_eq!(&cached[16..], &[0, 0, 0x03, 0xe8, 0, 0, 0, 0]);
    }

    #[test]
    fn rich_cursor_layout() {
        let msg = rich_cursor(&cursor(), &PixelFormat::NATIVE);
        assert_eq!(i32::from_be_bytes(msg[12..16].try_into().unwrap()), -239);
        assert_eq!(msg.len(), 16 + 6 * 4 + 2);
        assert_eq!(&msg[40..], &[0b1010_0000, 0b1010_0000]);
    }

    #[test]
    fn position_and_size() {
        assert_eq!(
            cursor_position(5, 6),
            [0, 0, 0, 1, 0, 5, 0, 6, 0, 0, 0, 0, 0, 0, 4, 0x4c]
        );
        assert_eq!(
            desktop_size(800, 600),
            [
                0, 0, 0, 1, 0, 0, 0, 0, 3, 0x20, 2, 0x58, 0xff, 0xff, 0xff, 0x21
            ]
        );
    }

    #[test]
    fn device_info_layout() {
        let msg = device_info("Mac14,3", "", "");
        let size = u16::from_be_bytes([msg[16], msg[17]]) as usize;
        let body = &msg[18..];
        assert_eq!(size, body.len());
        assert!(size > 17);
        assert_eq!(&body[0..2], &[0, 1]);
        assert_eq!(&body[10..16], &[0, 8, 0, 1, 0, 1]);
        assert_eq!(&body[16..24], b"Mac14,3\0");
        assert_eq!(&body[24..], &[0, 0]);
    }

    #[test]
    fn cut_text() {
        assert_eq!(server_cut_text(b"hi"), [3, 0, 0, 0, 0, 0, 0, 2, b'h', b'i']);
    }
}
