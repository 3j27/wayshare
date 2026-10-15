//! Minimal protobuf wire format: enough for Apple's PBCodable negotiation blobs.

use anyhow::{Result, bail};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value<'a> {
    Varint(u64),
    Fixed64(u64),
    Bytes(&'a [u8]),
    Fixed32(u32),
}

impl<'a> Value<'a> {
    pub fn u64(self) -> Result<u64> {
        match self {
            Value::Varint(v) | Value::Fixed64(v) => Ok(v),
            Value::Fixed32(v) => Ok(v.into()),
            Value::Bytes(_) => bail!("expected scalar, got bytes"),
        }
    }

    pub fn u32(self) -> Result<u32> {
        Ok(self.u64()? as u32)
    }

    pub fn bool(self) -> Result<bool> {
        Ok(self.u64()? != 0)
    }

    pub fn bytes(self) -> Result<&'a [u8]> {
        match self {
            Value::Bytes(b) => Ok(b),
            _ => bail!("expected length-delimited field"),
        }
    }

    pub fn string(self) -> Result<String> {
        Ok(String::from_utf8(self.bytes()?.to_vec())?)
    }
}

fn read_varint(buf: &[u8], pos: &mut usize) -> Result<u64> {
    let mut v = 0u64;
    for shift in (0..64).step_by(7) {
        let Some(&b) = buf.get(*pos) else {
            bail!("truncated varint")
        };
        *pos += 1;
        v |= u64::from(b & 0x7f) << shift;
        if b & 0x80 == 0 {
            return Ok(v);
        }
    }
    bail!("varint too long")
}

fn take<'a>(buf: &'a [u8], pos: &mut usize, n: usize) -> Result<&'a [u8]> {
    let end = pos.checked_add(n).filter(|&e| e <= buf.len());
    let Some(end) = end else {
        bail!("truncated field")
    };
    let s = &buf[*pos..end];
    *pos = end;
    Ok(s)
}

/// Decodes every top-level field of `buf` in wire order.
pub fn fields(buf: &[u8]) -> Result<Vec<(u32, Value<'_>)>> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < buf.len() {
        let key = read_varint(buf, &mut pos)?;
        let field = (key >> 3) as u32;
        let value = match key & 7 {
            0 => Value::Varint(read_varint(buf, &mut pos)?),
            1 => Value::Fixed64(u64::from_le_bytes(take(buf, &mut pos, 8)?.try_into()?)),
            2 => {
                let n = read_varint(buf, &mut pos)? as usize;
                Value::Bytes(take(buf, &mut pos, n)?)
            }
            5 => Value::Fixed32(u32::from_le_bytes(take(buf, &mut pos, 4)?.try_into()?)),
            wt => bail!("unsupported wire type {wt}"),
        };
        out.push((field, value));
    }
    Ok(out)
}

#[derive(Default)]
pub struct Writer(pub Vec<u8>);

impl Writer {
    fn varint(&mut self, mut v: u64) {
        while v >= 0x80 {
            self.0.push(v as u8 | 0x80);
            v >>= 7;
        }
        self.0.push(v as u8);
    }

    pub fn uint(&mut self, field: u32, v: impl Into<u64>) -> &mut Self {
        self.varint(u64::from(field) << 3);
        self.varint(v.into());
        self
    }

    pub fn bool(&mut self, field: u32, v: bool) -> &mut Self {
        self.uint(field, v as u64)
    }

    pub fn bytes(&mut self, field: u32, v: &[u8]) -> &mut Self {
        self.varint(u64::from(field) << 3 | 2);
        self.varint(v.len() as u64);
        self.0.extend_from_slice(v);
        self
    }

    pub fn opt_uint(&mut self, field: u32, v: Option<u32>) -> &mut Self {
        if let Some(v) = v {
            self.uint(field, v);
        }
        self
    }

    pub fn opt_bool(&mut self, field: u32, v: Option<bool>) -> &mut Self {
        if let Some(v) = v {
            self.bool(field, v);
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut w = Writer::default();
        w.uint(1, 300u32)
            .bytes(3, b"Mac")
            .uint(4, u64::MAX)
            .bool(5, true);
        let f = fields(&w.0).unwrap();
        assert_eq!(f[0], (1, Value::Varint(300)));
        assert_eq!(f[1], (3, Value::Bytes(b"Mac")));
        assert_eq!(f[2].1.u64().unwrap(), u64::MAX);
        assert!(f[3].1.bool().unwrap());
    }

    #[test]
    fn truncated() {
        assert!(fields(&[0x0a, 0x05, 1, 2]).is_err());
        assert!(fields(&[0x08, 0x80]).is_err());
    }
}
