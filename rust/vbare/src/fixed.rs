//! `FixedBytes<N>`, the owned form of `data[N]` for N above 32.

use bytes::Bytes;
use std::{fmt, ops::Deref};

/// Exactly `N` bytes backed by [`Bytes`], so decoding shares the input buffer instead of copying.
///
/// Small fixed-length data (`data[N]` with N at most 32) decodes to `[u8; N]` instead.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FixedBytes<const N: usize>(Bytes);

/// Holds an all-zero array so a `&'static [u8; N]` exists for every `N`.
pub(crate) struct Zeros<const N: usize>;

impl<const N: usize> Zeros<N> {
    pub(crate) const ARRAY: [u8; N] = [0; N];
}

impl<const N: usize> FixedBytes<N> {
    /// Wraps `bytes`, returning them unchanged if their length is not `N`.
    #[inline]
    pub fn new(bytes: Bytes) -> Result<Self, Bytes> {
        if bytes.len() == N {
            Ok(FixedBytes(bytes))
        } else {
            Err(bytes)
        }
    }

    #[inline]
    pub fn as_array(&self) -> &[u8; N] {
        // The constructors guarantee the length, so the fallback is never taken.
        <&[u8; N]>::try_from(self.0.as_ref()).unwrap_or(&Zeros::<N>::ARRAY)
    }

    #[inline]
    pub fn as_bytes(&self) -> &Bytes {
        &self.0
    }

    #[inline]
    pub fn into_bytes(self) -> Bytes {
        self.0
    }
}

impl<const N: usize> Default for FixedBytes<N> {
    #[inline]
    fn default() -> Self {
        FixedBytes(Bytes::from_static(&Zeros::<N>::ARRAY))
    }
}

impl<const N: usize> From<[u8; N]> for FixedBytes<N> {
    #[inline]
    fn from(value: [u8; N]) -> Self {
        FixedBytes(Bytes::copy_from_slice(&value))
    }
}

impl<const N: usize> From<&[u8; N]> for FixedBytes<N> {
    #[inline]
    fn from(value: &[u8; N]) -> Self {
        FixedBytes(Bytes::copy_from_slice(value))
    }
}

impl<const N: usize> TryFrom<Bytes> for FixedBytes<N> {
    type Error = Bytes;

    #[inline]
    fn try_from(value: Bytes) -> Result<Self, Bytes> {
        FixedBytes::new(value)
    }
}

impl<const N: usize> Deref for FixedBytes<N> {
    type Target = [u8; N];

    #[inline]
    fn deref(&self) -> &[u8; N] {
        self.as_array()
    }
}

impl<const N: usize> AsRef<[u8]> for FixedBytes<N> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl<const N: usize> fmt::Debug for FixedBytes<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}
