//! Annex B helpers for HEVC.

pub const NAL_IDR_W_RADL: u8 = 19;
pub const NAL_IDR_N_LP: u8 = 20;
pub const NAL_VPS: u8 = 32;
pub const NAL_SEI_PREFIX: u8 = 39;
pub const NAL_SEI_SUFFIX: u8 = 40;

pub fn nal_type(nal: &[u8]) -> u8 {
    nal.first().map_or(0, |b| (b >> 1) & 0x3f)
}

/// NAL units of an Annex B stream, without start codes.
pub fn split_annexb(data: &[u8]) -> impl Iterator<Item = &[u8]> {
    let mut pos = next_start(data, 0).map(|(_, end)| end);
    std::iter::from_fn(move || {
        let start = pos?;
        let (nal_end, next) = match next_start(data, start) {
            Some((s, e)) => (s, Some(e)),
            None => (data.len(), None),
        };
        pos = next;
        let mut nal = &data[start..nal_end];
        while let [rest @ .., 0] = nal {
            nal = rest;
        }
        Some(nal)
    })
}

/// (start of start code, first byte after it)
fn next_start(data: &[u8], from: usize) -> Option<(usize, usize)> {
    let i = data.get(from..)?.windows(3).position(|w| w == [0, 0, 1])? + from;
    Some((i, i + 3))
}

/// Rewrites `data` as Annex B with 4-byte start codes, dropping SEI NALs.
pub fn strip_sei(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    for nal in split_annexb(data) {
        if !nal.is_empty() && !matches!(nal_type(nal), NAL_SEI_PREFIX | NAL_SEI_SUFFIX) {
            out.extend_from_slice(&[0, 0, 0, 1]);
            out.extend_from_slice(nal);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_and_strip() {
        let vps = [NAL_VPS << 1, 1, 0xaa];
        let sei = [NAL_SEI_PREFIX << 1, 1, 0xbb];
        let idr = [NAL_IDR_W_RADL << 1, 1, 0xcc, 0x00, 0x00, 0x03, 0x01];
        let mut s = vec![0, 0, 0, 1];
        s.extend_from_slice(&vps);
        s.extend_from_slice(&[0, 0, 1]);
        s.extend_from_slice(&sei);
        s.extend_from_slice(&[0, 0, 0, 1]);
        s.extend_from_slice(&idr);
        let nals: Vec<_> = split_annexb(&s).collect();
        assert_eq!(nals, [&vps[..], &sei[..], &idr[..]]);

        let out = strip_sei(&s);
        let types: Vec<_> = split_annexb(&out).map(nal_type).collect();
        assert_eq!(types, [NAL_VPS, NAL_IDR_W_RADL]);
    }
}
