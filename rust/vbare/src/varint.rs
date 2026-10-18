//! BARE `uint` (ULEB128) and `int` (zig-zag) helpers.

use crate::error::DecodeErrorKind;
use bytes::BufMut;

/// The longest valid varint: 64 bits in 7-bit groups.
pub const MAX_LEN: usize = 10;

/// Maps a signed integer to an unsigned one so small magnitudes stay short.
#[inline]
pub const fn zigzag(value: i64) -> u64 {
    ((value << 1) ^ (value >> 63)) as u64
}

/// Inverse of [`zigzag`].
#[inline]
pub const fn unzigzag(value: u64) -> i64 {
    ((value >> 1) as i64) ^ -((value & 1) as i64)
}

/// The number of bytes `value` takes as a `uint`.
#[inline]
pub const fn uint_len(value: u64) -> usize {
    let bits = 64 - (value | 1).leading_zeros() as usize;
    bits.div_ceil(7)
}

/// The number of bytes `value` takes as an `int`.
#[inline]
pub const fn int_len(value: i64) -> usize {
    uint_len(zigzag(value))
}

#[inline]
pub fn put_uint<B: BufMut + ?Sized>(out: &mut B, mut value: u64) {
    let mut buf = [0u8; MAX_LEN];
    let mut len = 0;
    while value >= 0x80 {
        buf[len] = (value as u8) | 0x80;
        value >>= 7;
        len += 1;
    }
    buf[len] = value as u8;
    out.put_slice(&buf[..=len]);
}

#[inline]
pub fn put_int<B: BufMut + ?Sized>(out: &mut B, value: i64) {
    put_uint(out, zigzag(value));
}

/// Strictly decodes a `uint` from the start of `input`, returning the value and its length.
///
/// Rejects encodings longer than 10 bytes, a 10th byte above 1 (bits beyond 64), and non-minimal
/// encodings, which end in a zero group after the first byte.
#[inline]
pub fn read_uint(input: &[u8]) -> Result<(u64, usize), DecodeErrorKind> {
    let mut value = 0u64;
    for (index, &byte) in input.iter().enumerate() {
        if index == MAX_LEN - 1 && byte > 1 {
            return Err(DecodeErrorKind::VarintOverflow);
        }
        value |= u64::from(byte & 0x7f) << (7 * index);
        if byte & 0x80 == 0 {
            if byte == 0 && index > 0 {
                return Err(DecodeErrorKind::NonCanonicalVarint);
            }
            return Ok((value, index + 1));
        }
    }
    Err(DecodeErrorKind::Truncated)
}

/// Decodes a `uint` already accepted by [`read_uint`]. Never fails: malformed or truncated input
/// yields an arbitrary value and consumes at most 10 bytes.
#[inline]
pub fn read_uint_lenient(input: &[u8]) -> (u64, usize) {
    let mut value = 0u64;
    for (index, &byte) in input.iter().take(MAX_LEN).enumerate() {
        value |= u64::from(byte & 0x7f).wrapping_shl(7 * index as u32);
        if byte & 0x80 == 0 {
            return (value, index + 1);
        }
    }
    (value, input.len().min(MAX_LEN))
}
