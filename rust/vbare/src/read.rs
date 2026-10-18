//! Cursors over input: [`Verifier`] checks untrusted bytes, [`Reader`] walks verified bytes.
//!
//! Neither type can be constructed outside this crate. Generated code only receives them inside
//! [`Wire`](crate::Wire) implementations, which is what lets `Reader` hand out `&str` without
//! validating UTF-8 again.

use crate::{
    error::{DecodeError, DecodeErrorKind},
    str::Str,
    varint,
};
use bytes::Bytes;

/// A cursor over bytes that a [`Verifier`] has accepted.
///
/// Every read is total: if the bytes were not verified for the type being read, reads return
/// arbitrary values instead of panicking. Positions are absolute offsets into the source buffer.
#[derive(Clone, Copy)]
pub struct Reader<'a> {
    base: &'a Bytes,
    start: usize,
    end: usize,
    pos: usize,
}

impl<'a> Reader<'a> {
    #[inline]
    pub(crate) fn new(base: &'a Bytes) -> Self {
        Reader {
            base,
            start: 0,
            end: base.len(),
            pos: 0,
        }
    }

    /// The position relative to the start of this reader's span.
    #[inline]
    pub fn offset(&self) -> usize {
        self.pos - self.start
    }

    /// The length of this reader's span.
    #[inline]
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.end == self.start
    }

    /// The bytes not yet read.
    #[inline]
    pub fn remaining(&self) -> &'a [u8] {
        self.base.get(self.pos..self.end).unwrap_or(&[])
    }

    /// The whole span this reader covers, read or not.
    #[inline]
    pub fn as_slice(&self) -> &'a [u8] {
        self.base.get(self.start..self.end).unwrap_or(&[])
    }

    /// The span as shared [`Bytes`], without copying.
    #[inline]
    pub fn to_bytes(&self) -> Bytes {
        self.base.slice(self.start..self.end)
    }

    /// A reader over `[from, to)` of this span, positioned at `from`. Offsets are relative to the
    /// start of this span and are clamped to it.
    #[inline]
    pub fn slice(&self, from: usize, to: usize) -> Reader<'a> {
        let to = self.start.saturating_add(to).min(self.end);
        let from = self.start.saturating_add(from).min(to);
        Reader {
            base: self.base,
            start: from,
            end: to,
            pos: from,
        }
    }

    /// A reader over the bytes from relative offset `from` to the current position.
    #[inline]
    pub fn since(&self, from: usize) -> Reader<'a> {
        self.slice(from, self.offset())
    }

    /// A reader over the next `len` bytes, advancing past them.
    #[inline]
    pub fn split(&mut self, len: usize) -> Reader<'a> {
        let from = self.pos;
        let to = from.saturating_add(len).min(self.end);
        self.pos = to;
        Reader {
            base: self.base,
            start: from,
            end: to,
            pos: from,
        }
    }

    /// A reader over the rest of the span, advancing to its end.
    #[inline]
    pub fn rest(&mut self) -> Reader<'a> {
        self.split(self.end - self.pos)
    }

    #[inline]
    pub fn skip(&mut self, len: usize) {
        self.pos = self.pos.saturating_add(len).min(self.end);
    }

    #[inline]
    pub fn take(&mut self, len: usize) -> &'a [u8] {
        let from = self.pos;
        self.skip(len);
        self.base.get(from..self.pos).unwrap_or(&[])
    }

    /// Reads `len` bytes as shared [`Bytes`].
    #[inline]
    pub fn take_bytes(&mut self, len: usize) -> Bytes {
        let from = self.pos;
        self.skip(len);
        self.base.slice(from..self.pos)
    }

    /// Reads `N` bytes, zero-padding if the span ends first.
    #[inline]
    pub fn array<const N: usize>(&mut self) -> [u8; N] {
        let mut out = [0u8; N];
        let bytes = self.take(N);
        out[..bytes.len()].copy_from_slice(bytes);
        out
    }

    #[inline]
    pub fn array_ref<const N: usize>(&mut self) -> &'a [u8; N] {
        <&[u8; N]>::try_from(self.take(N)).unwrap_or(&crate::fixed::Zeros::<N>::ARRAY)
    }

    #[inline]
    pub fn u8(&mut self) -> u8 {
        self.array::<1>()[0]
    }

    #[inline]
    pub fn uint(&mut self) -> u64 {
        let (value, len) = varint::read_uint_lenient(self.remaining());
        self.skip(len);
        value
    }

    #[inline]
    pub fn int(&mut self) -> i64 {
        varint::unzigzag(self.uint())
    }

    /// Reads a length-prefixed `str` that the verifier accepted.
    #[inline]
    pub fn str(&mut self) -> &'a str {
        let len = self.uint();
        let bytes = self.take(usize::try_from(len).unwrap_or(usize::MAX));
        // SAFETY: `Reader` is only constructed over verified input, and `Wire` implementations
        // (an unsafe trait) promise to read exactly the layout their verifier accepted, so these
        // bytes are a `str` the verifier validated as UTF-8.
        unsafe { std::str::from_utf8_unchecked(bytes) }
    }

    /// Reads a length-prefixed `str` as a shared [`Str`].
    #[inline]
    pub fn str_owned(&mut self) -> Str {
        let len = self.uint();
        Str::from_verified(self.take_bytes(usize::try_from(len).unwrap_or(usize::MAX)))
    }

    /// Reads a length-prefixed `data`.
    #[inline]
    pub fn data(&mut self) -> &'a [u8] {
        let len = self.uint();
        self.take(usize::try_from(len).unwrap_or(usize::MAX))
    }

    #[inline]
    pub fn data_owned(&mut self) -> Bytes {
        let len = self.uint();
        self.take_bytes(usize::try_from(len).unwrap_or(usize::MAX))
    }
}

impl std::fmt::Debug for Reader<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reader")
            .field("start", &self.start)
            .field("end", &self.end)
            .field("pos", &self.pos)
            .finish()
    }
}

/// A cursor that checks untrusted input and reports the first violation.
pub struct Verifier<'a> {
    base: &'a Bytes,
    pos: usize,
}

impl<'a> Verifier<'a> {
    #[inline]
    pub(crate) fn new(base: &'a Bytes) -> Result<Self, DecodeError> {
        if u32::try_from(base.len()).is_err() {
            return Err(DecodeError::new(DecodeErrorKind::MessageTooLarge, 0));
        }
        Ok(Verifier { base, pos: 0 })
    }

    /// The absolute position in the input.
    #[inline]
    pub fn position(&self) -> usize {
        self.pos
    }

    #[inline]
    pub fn remaining_len(&self) -> usize {
        self.base.len() - self.pos
    }

    /// Creates an error at the current position.
    #[cold]
    pub fn error(&self, kind: DecodeErrorKind) -> DecodeError {
        DecodeError::new(kind, self.pos)
    }

    /// A reader over the verified bytes from absolute position `from` to the current position.
    #[inline]
    pub fn reader_since(&self, from: usize) -> Reader<'a> {
        let from = from.min(self.pos);
        Reader {
            base: self.base,
            start: from,
            end: self.pos,
            pos: from,
        }
    }

    /// The offset of the current position from `from`, for offset tables. Messages are at most
    /// 4 GiB, so it always fits.
    #[inline]
    pub fn offset_from(&self, from: usize) -> u32 {
        u32::try_from(self.pos.saturating_sub(from)).unwrap_or(u32::MAX)
    }

    #[inline]
    pub(crate) fn finish(&self) -> Result<(), DecodeError> {
        if self.pos == self.base.len() {
            Ok(())
        } else {
            Err(self.error(DecodeErrorKind::TrailingBytes))
        }
    }

    #[inline]
    pub fn take(&mut self, len: usize) -> Result<&'a [u8], DecodeError> {
        let end = self
            .pos
            .checked_add(len)
            .filter(|end| *end <= self.base.len())
            .ok_or_else(|| self.error(DecodeErrorKind::Truncated))?;
        let bytes = self.base.get(self.pos..end).unwrap_or(&[]);
        self.pos = end;
        Ok(bytes)
    }

    #[inline]
    pub fn array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let bytes = self.take(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(bytes);
        Ok(out)
    }

    #[inline]
    pub fn array_ref<const N: usize>(&mut self) -> Result<&'a [u8; N], DecodeError> {
        let bytes = self.take(N)?;
        Ok(<&[u8; N]>::try_from(bytes).unwrap_or(&crate::fixed::Zeros::<N>::ARRAY))
    }

    #[inline]
    pub fn uint(&mut self) -> Result<u64, DecodeError> {
        let input = self.base.get(self.pos..).unwrap_or(&[]);
        match varint::read_uint(input) {
            Ok((value, len)) => {
                self.pos += len;
                Ok(value)
            }
            Err(kind) => Err(self.error(kind)),
        }
    }

    #[inline]
    pub fn int(&mut self) -> Result<i64, DecodeError> {
        self.uint().map(varint::unzigzag)
    }

    #[inline]
    pub fn bool(&mut self) -> Result<bool, DecodeError> {
        match self.array::<1>()? {
            [0] => Ok(false),
            [1] => Ok(true),
            _ => Err(DecodeError::new(DecodeErrorKind::InvalidBool, self.pos - 1)),
        }
    }

    /// Reads a length or count prefix and checks that `len * min_size` bytes remain. Every
    /// element of a valid schema takes at least one byte, so callers pass `min_size >= 1`.
    #[inline]
    pub fn length(&mut self, min_size: usize) -> Result<u64, DecodeError> {
        let start = self.pos;
        let len = self.uint()?;
        let needed = usize::try_from(len)
            .ok()
            .and_then(|len| len.checked_mul(min_size.max(1)));
        match needed {
            Some(needed) if needed <= self.remaining_len() => Ok(len),
            _ => Err(DecodeError::new(DecodeErrorKind::LengthExceedsInput, start)),
        }
    }

    #[inline]
    pub fn data(&mut self) -> Result<&'a [u8], DecodeError> {
        let len = self.length(1)?;
        self.take(usize::try_from(len).unwrap_or(usize::MAX))
    }

    #[inline]
    pub fn str(&mut self) -> Result<&'a str, DecodeError> {
        let bytes = self.data()?;
        let bytes_start = self.pos - bytes.len();
        std::str::from_utf8(bytes).map_err(|error| {
            DecodeError::new(
                DecodeErrorKind::InvalidUtf8,
                bytes_start + error.valid_up_to(),
            )
        })
    }
}
