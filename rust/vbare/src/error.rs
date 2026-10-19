//! Decode and encode errors.

use std::{borrow::Cow, error::Error as StdError, fmt};

/// Why a message failed to decode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DecodeErrorKind {
    /// The input ended inside a value.
    Truncated,
    /// Bytes remain after the top-level value.
    TrailingBytes,
    /// A `str` is not valid UTF-8.
    InvalidUtf8,
    /// An enum value, union tag or optional tag is not defined by the schema.
    InvalidTag,
    /// A `bool` byte is neither 0 nor 1.
    InvalidBool,
    /// A varint uses more bytes than its value needs.
    NonCanonicalVarint,
    /// A varint is longer than 10 bytes or does not fit in 64 bits.
    VarintOverflow,
    /// A length or element count claims more data than the input holds.
    LengthExceedsInput,
    /// A map contains the same key twice.
    DuplicateMapKey,
    /// The version header names a version the reader does not support.
    UnsupportedVersion,
    /// The input is larger than 4 GiB, which the offset tables cannot address.
    MessageTooLarge,
    /// A version converter rejected the decoded value.
    Conversion,
}

impl DecodeErrorKind {
    fn describe(self) -> &'static str {
        match self {
            DecodeErrorKind::Truncated => "input ended inside a value",
            DecodeErrorKind::TrailingBytes => "trailing bytes after the message",
            DecodeErrorKind::InvalidUtf8 => "invalid UTF-8 in str",
            DecodeErrorKind::InvalidTag => "unknown enum value, union tag or optional tag",
            DecodeErrorKind::InvalidBool => "bool byte is neither 0 nor 1",
            DecodeErrorKind::NonCanonicalVarint => "varint is not minimally encoded",
            DecodeErrorKind::VarintOverflow => "varint does not fit in 64 bits",
            DecodeErrorKind::LengthExceedsInput => "length exceeds the remaining input",
            DecodeErrorKind::DuplicateMapKey => "duplicate map key",
            DecodeErrorKind::UnsupportedVersion => "unsupported version",
            DecodeErrorKind::MessageTooLarge => "message is larger than 4 GiB",
            DecodeErrorKind::Conversion => "version conversion failed",
        }
    }
}

/// One step of the path from the message root to the failing value.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Segment {
    Field(&'static str),
    Index(u64),
}

#[derive(Debug)]
struct Inner {
    kind: DecodeErrorKind,
    offset: usize,
    /// Innermost segment first. Built only while an error propagates.
    path: Vec<Segment>,
    detail: Option<Cow<'static, str>>,
    /// The version converter failure this error wraps, kept as is so callers can downcast it.
    source: Option<anyhow::Error>,
}

/// A decode failure with the byte offset and the path to the value that failed, such as
/// `entries[3].body.payload`.
///
/// The error is boxed so `Result<T, DecodeError>` stays one pointer wide on the success path.
pub struct DecodeError(Box<Inner>);

impl DecodeError {
    /// Creates an error at byte `offset` of the input.
    #[cold]
    pub fn new(kind: DecodeErrorKind, offset: usize) -> Self {
        DecodeError(Box::new(Inner {
            kind,
            offset,
            path: Vec::new(),
            detail: None,
            source: None,
        }))
    }

    /// Creates an error carrying a message, for example an unsupported version number.
    #[cold]
    pub fn with_detail(
        kind: DecodeErrorKind,
        offset: usize,
        detail: impl Into<Cow<'static, str>>,
    ) -> Self {
        let mut error = DecodeError::new(kind, offset);
        error.0.detail = Some(detail.into());
        error
    }

    /// Wraps a version converter failure.
    #[cold]
    pub fn conversion(source: anyhow::Error) -> Self {
        let mut error = DecodeError::new(DecodeErrorKind::Conversion, 0);
        error.0.source = Some(source);
        error
    }

    /// The version converter failure, for a [`DecodeErrorKind::Conversion`] error.
    pub fn into_conversion(self) -> Result<anyhow::Error, Self> {
        let mut inner = self.0;
        match inner.source.take() {
            Some(source) => Ok(source),
            None => Err(DecodeError(inner)),
        }
    }

    pub fn kind(&self) -> DecodeErrorKind {
        self.0.kind
    }

    /// Byte offset in the input where decoding failed.
    pub fn offset(&self) -> usize {
        self.0.offset
    }

    /// The path to the failing value, for example `entries[3].body`. Empty at the root.
    pub fn path(&self) -> String {
        let mut path = String::new();
        for segment in self.0.path.iter().rev() {
            match segment {
                Segment::Field(name) => {
                    if !path.is_empty() {
                        path.push('.');
                    }
                    path.push_str(name);
                }
                Segment::Index(index) => {
                    path.push('[');
                    path.push_str(&index.to_string());
                    path.push(']');
                }
            }
        }
        path
    }

    /// Records that the error occurred inside the struct field or union member `name`.
    #[cold]
    pub fn in_field(mut self, name: &'static str) -> Self {
        self.0.path.push(Segment::Field(name));
        self
    }

    /// Records that the error occurred inside list element or map pair `index`.
    #[cold]
    pub fn in_index(mut self, index: u64) -> Self {
        self.0.path.push(Segment::Index(index));
        self
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.kind.describe())?;
        if let Some(detail) = &self.0.detail {
            write!(f, " ({detail})")?;
        }
        let path = self.path();
        if !path.is_empty() {
            write!(f, " at {path}")?;
        }
        write!(f, " (byte {})", self.0.offset)?;
        if let Some(source) = &self.0.source {
            write!(f, ": {source}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DecodeError")
            .field("kind", &self.0.kind)
            .field("offset", &self.0.offset)
            .field("path", &self.path())
            .field("detail", &self.0.detail)
            .field("source", &self.0.source)
            .finish()
    }
}

impl StdError for DecodeError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.0
            .source
            .as_ref()
            .map(|source| source.as_ref() as &(dyn StdError + 'static))
    }
}

/// Why a value could not be encoded.
#[derive(Debug)]
#[non_exhaustive]
pub enum EncodeError {
    /// The output slice is shorter than the encoded value.
    BufferTooSmall { needed: usize, available: usize },
    /// The requested version is not one the writer supports.
    UnsupportedVersion(u16),
    /// A version converter rejected the value.
    Conversion(anyhow::Error),
}

impl fmt::Display for EncodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EncodeError::BufferTooSmall { needed, available } => write!(
                f,
                "output buffer holds {available} bytes but the value needs {needed}"
            ),
            EncodeError::UnsupportedVersion(version) => {
                write!(f, "unsupported version {version}")
            }
            EncodeError::Conversion(error) => write!(f, "version conversion failed: {error}"),
        }
    }
}

impl StdError for EncodeError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            EncodeError::BufferTooSmall { .. } | EncodeError::UnsupportedVersion(_) => None,
            EncodeError::Conversion(error) => Some(error.as_ref()),
        }
    }
}
