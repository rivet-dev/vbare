//! [`Body`], the value a [`Versioned`](crate::Versioned) envelope holds at its latest version.

use crate::{
    error::{DecodeError, DecodeErrorKind},
    message::{decode, Message},
    sink::Sink,
    wire::{self, Wire},
};
use bytes::Bytes;

/// A value that a [`Versioned`](crate::Versioned) envelope decodes to and encodes from at its
/// latest version, written without a version header.
///
/// Every generated message is a `Body`. Implement it by hand when the latest value is not a
/// schema type: a domain type converted to and from a generated message, or a value with a
/// hand-written encoding, which can decode field by field with [`decode_with`](crate::decode_with).
/// `Option<T>`, `bool` and the fixed-width numbers are bodies encoded as
/// BARE `optional<T>`, `bool` and `u8` to `f64`, so an envelope can store, for example, an
/// optional timestamp directly.
pub trait Body: Clone + Sized + 'static {
    /// The exact length [`put_body`](Body::put_body) writes.
    fn body_len(&self) -> usize;

    fn put_body<S: Sink>(&self, out: &mut S);

    /// Decodes a body that fills `bytes` exactly.
    fn from_body(bytes: Bytes) -> Result<Self, DecodeError>;
}

impl<T: Message + Clone + 'static> Body for T {
    #[inline]
    fn body_len(&self) -> usize {
        self.encoded_len()
    }

    #[inline]
    fn put_body<S: Sink>(&self, out: &mut S) {
        self.encode(out);
    }

    #[inline]
    fn from_body(bytes: Bytes) -> Result<Self, DecodeError> {
        T::decode(bytes)
    }
}

/// BARE `optional<T>`: a tag byte, then `T` when the tag is 1.
impl<T: Body> Body for Option<T> {
    fn body_len(&self) -> usize {
        match self {
            Some(inner) => 1 + inner.body_len(),
            None => 1,
        }
    }

    fn put_body<S: Sink>(&self, out: &mut S) {
        match self {
            Some(inner) => {
                out.put_u8(1);
                inner.put_body(out);
            }
            None => out.put_u8(0),
        }
    }

    fn from_body(mut bytes: Bytes) -> Result<Self, DecodeError> {
        let Some(&tag) = bytes.first() else {
            return Err(DecodeError::new(DecodeErrorKind::Truncated, 0));
        };
        match tag {
            0 if bytes.len() == 1 => Ok(None),
            0 => Err(DecodeError::new(DecodeErrorKind::TrailingBytes, 1)),
            1 => T::from_body(bytes.split_off(1)).map(Some),
            _ => Err(DecodeError::new(DecodeErrorKind::InvalidTag, 0)),
        }
    }
}

/// Implements [`Body`] for a primitive through its [`Wire`] marker.
macro_rules! wire_body {
    ($($ty:ty => $marker:ty),* $(,)?) => {
        $(
            impl Body for $ty {
                #[inline]
                fn body_len(&self) -> usize {
                    <$marker as Wire>::encoded_len(self)
                }

                #[inline]
                fn put_body<S: Sink>(&self, out: &mut S) {
                    <$marker as Wire>::encode(self, out);
                }

                #[inline]
                fn from_body(bytes: Bytes) -> Result<Self, DecodeError> {
                    decode::<$marker>(&bytes)
                }
            }
        )*
    };
}

wire_body!(
    bool => wire::Bool,
    u8 => wire::U8,
    u16 => wire::U16,
    u32 => wire::U32,
    u64 => wire::U64,
    i8 => wire::I8,
    i16 => wire::I16,
    i32 => wire::I32,
    i64 => wire::I64,
    f32 => wire::F32,
    f64 => wire::F64,
);
