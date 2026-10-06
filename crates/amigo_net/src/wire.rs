//! Byte-level encoding for packets: little-endian integers, written and read
//! with explicit bounds.
//!
//! Every read checks the remaining length and fails with [`WireError`]
//! instead of panicking, because every byte here comes from an untrusted
//! peer.

use std::fmt;

/// Why a byte buffer could not be decoded or encoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WireError {
    /// The buffer ended before the value did.
    Truncated,
    /// Bytes were left over after the last field.
    TrailingBytes(usize),
    /// A field held a value it may not have.
    Invalid(&'static str),
    /// The encoded packet would exceed the maximum size.
    TooLarge { size: usize, max: usize },
}

impl fmt::Display for WireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WireError::Truncated => write!(f, "packet truncated"),
            WireError::TrailingBytes(n) => write!(f, "{n} unexpected bytes at the end"),
            WireError::Invalid(what) => write!(f, "invalid {what}"),
            WireError::TooLarge { size, max } => {
                write!(f, "packet of {size} bytes exceeds the maximum of {max}")
            }
        }
    }
}

impl std::error::Error for WireError {}

/// Appends little-endian values to a buffer.
#[derive(Debug, Default)]
pub struct ByteWriter {
    buf: Vec<u8>,
}

impl ByteWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            buf: Vec::with_capacity(capacity),
        }
    }

    pub fn u8(&mut self, v: u8) -> &mut Self {
        self.buf.push(v);
        self
    }

    pub fn u16(&mut self, v: u16) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn u32(&mut self, v: u32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn u64(&mut self, v: u64) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn i32(&mut self, v: i32) -> &mut Self {
        self.buf.extend_from_slice(&v.to_le_bytes());
        self
    }

    pub fn bytes(&mut self, v: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(v);
        self
    }

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    pub fn finish(self) -> Vec<u8> {
        self.buf
    }
}

/// Reads little-endian values from a buffer, failing on short input.
#[derive(Debug)]
pub struct ByteReader<'a> {
    buf: &'a [u8],
}

impl<'a> ByteReader<'a> {
    pub fn new(buf: &'a [u8]) -> Self {
        Self { buf }
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        if self.buf.len() < n {
            return Err(WireError::Truncated);
        }
        let (head, rest) = self.buf.split_at(n);
        self.buf = rest;
        Ok(head)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], WireError> {
        let mut out = [0u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }

    pub fn u8(&mut self) -> Result<u8, WireError> {
        Ok(self.take(1)?[0])
    }

    pub fn u16(&mut self) -> Result<u16, WireError> {
        self.array().map(u16::from_le_bytes)
    }

    pub fn u32(&mut self) -> Result<u32, WireError> {
        self.array().map(u32::from_le_bytes)
    }

    pub fn u64(&mut self) -> Result<u64, WireError> {
        self.array().map(u64::from_le_bytes)
    }

    pub fn i32(&mut self) -> Result<i32, WireError> {
        self.array().map(i32::from_le_bytes)
    }

    /// The next `n` bytes.
    pub fn bytes(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        self.take(n)
    }

    /// Everything not read yet.
    pub fn rest(&mut self) -> &'a [u8] {
        std::mem::take(&mut self.buf)
    }

    pub fn remaining(&self) -> usize {
        self.buf.len()
    }

    /// Fail unless every byte was read.
    pub fn finish(self) -> Result<(), WireError> {
        match self.buf.len() {
            0 => Ok(()),
            n => Err(WireError::TrailingBytes(n)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_round_trip() {
        let mut w = ByteWriter::new();
        w.u8(7)
            .u16(0xbeef)
            .u32(0xdead_beef)
            .u64(u64::MAX - 3)
            .i32(-42)
            .bytes(b"xy");
        let buf = w.finish();
        assert_eq!(buf.len(), 1 + 2 + 4 + 8 + 4 + 2);

        let mut r = ByteReader::new(&buf);
        assert_eq!(r.u8(), Ok(7));
        assert_eq!(r.u16(), Ok(0xbeef));
        assert_eq!(r.u32(), Ok(0xdead_beef));
        assert_eq!(r.u64(), Ok(u64::MAX - 3));
        assert_eq!(r.i32(), Ok(-42));
        assert_eq!(r.bytes(2), Ok(&b"xy"[..]));
        assert_eq!(r.finish(), Ok(()));
    }

    #[test]
    fn short_input_is_an_error_not_a_panic() {
        let mut r = ByteReader::new(&[1, 2, 3]);
        assert_eq!(r.u32(), Err(WireError::Truncated));
        // A failed read consumes nothing.
        assert_eq!(r.u16(), Ok(0x0201));
        assert_eq!(r.bytes(5), Err(WireError::Truncated));
        assert_eq!(
            ByteReader::new(&[0; 3]).finish(),
            Err(WireError::TrailingBytes(3))
        );
    }
}
