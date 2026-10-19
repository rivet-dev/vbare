//! `Str`, an owned UTF-8 string that shares its bytes with the buffer it was decoded from.

use bytes::Bytes;
use std::{
    borrow::Borrow,
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    ops::Deref,
    str::Utf8Error,
};

/// An immutable UTF-8 string backed by [`Bytes`].
///
/// Decoding a message produces `Str` values that point into the input buffer, so they cost a
/// reference count increment instead of a copy. A `Str` keeps its whole source buffer alive.
/// Comparison, ordering and hashing behave exactly like `str`.
#[derive(Clone, Default)]
pub struct Str(Bytes);

impl Str {
    #[inline]
    pub const fn new() -> Self {
        Str(Bytes::new())
    }

    #[inline]
    pub const fn from_static(value: &'static str) -> Self {
        Str(Bytes::from_static(value.as_bytes()))
    }

    /// Validates `bytes` as UTF-8 without copying.
    #[inline]
    pub fn from_utf8(bytes: Bytes) -> Result<Self, Utf8Error> {
        std::str::from_utf8(&bytes)?;
        Ok(Str(bytes))
    }

    /// Wraps bytes the verifier already validated as UTF-8.
    #[inline]
    pub(crate) fn from_verified(bytes: Bytes) -> Self {
        Str(bytes)
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        // SAFETY: every constructor either validates the bytes or receives bytes that the
        // verifier validated as UTF-8, and the bytes are immutable.
        unsafe { std::str::from_utf8_unchecked(&self.0) }
    }

    /// The underlying bytes, shared with the source buffer.
    #[inline]
    pub fn as_bytes(&self) -> &Bytes {
        &self.0
    }

    #[inline]
    pub fn into_bytes(self) -> Bytes {
        self.0
    }

    /// Copies the string into an allocation of its own, so it no longer keeps the buffer it was
    /// decoded from alive.
    ///
    /// A decoded `Str` shares its source buffer, so storing a short name taken from a 1 MiB
    /// message keeps all 1 MiB in memory. Call this before storing a small field past the life
    /// of a large input. `Bytes` fields detach with [`Bytes::copy_from_slice`].
    #[inline]
    pub fn detach(&self) -> Str {
        Str(Bytes::copy_from_slice(&self.0))
    }
}

impl Deref for Str {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<str> for Str {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl AsRef<[u8]> for Str {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Borrow<str> for Str {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl From<&'static str> for Str {
    #[inline]
    fn from(value: &'static str) -> Self {
        Str::from_static(value)
    }
}

impl From<String> for Str {
    #[inline]
    fn from(value: String) -> Self {
        Str(Bytes::from(value.into_bytes()))
    }
}

impl From<Box<str>> for Str {
    #[inline]
    fn from(value: Box<str>) -> Self {
        Str::from(String::from(value))
    }
}

impl From<Str> for Bytes {
    #[inline]
    fn from(value: Str) -> Self {
        value.0
    }
}

impl From<Str> for String {
    #[inline]
    fn from(value: Str) -> Self {
        value.as_str().to_owned()
    }
}

impl PartialEq for Str {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for Str {}

impl PartialEq<str> for Str {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for Str {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<String> for Str {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<Str> for str {
    #[inline]
    fn eq(&self, other: &Str) -> bool {
        self == other.as_str()
    }
}

impl PartialEq<Str> for &str {
    #[inline]
    fn eq(&self, other: &Str) -> bool {
        *self == other.as_str()
    }
}

impl PartialOrd for Str {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Str {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

/// Hashes like `str`, which `Borrow<str>` requires for map lookups by `&str`.
impl Hash for Str {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl fmt::Debug for Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for Str {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}
