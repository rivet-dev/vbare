//! Top-level decoding and encoding: [`Encode`], [`Message`] and [`View`].

use crate::{
    error::{DecodeError, EncodeError},
    read::{Reader, Verifier},
    sink::Sink,
    wire::Wire,
};
use bytes::{Bytes, BytesMut};
use std::fmt;

/// A value that encodes to BARE. Encoding never fails for in-memory values.
pub trait Encode {
    /// The exact encoded length, computed without encoding.
    fn encoded_len(&self) -> usize;

    fn encode<S: Sink>(&self, out: &mut S);

    fn encode_to_vec(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.encoded_len());
        self.encode(&mut out);
        out
    }

    fn encode_to_bytes(&self) -> Bytes {
        let mut out = BytesMut::with_capacity(self.encoded_len());
        self.encode(&mut out);
        out.freeze()
    }

    /// Encodes into the front of `out` and returns the number of bytes written. Checks the length
    /// first instead of panicking when `out` is too short.
    fn encode_to_slice(&self, out: &mut [u8]) -> Result<usize, EncodeError> {
        let needed = self.encoded_len();
        if needed > out.len() {
            return Err(EncodeError::BufferTooSmall {
                needed,
                available: out.len(),
            });
        }
        let mut cursor = &mut out[..needed];
        self.encode(&mut cursor);
        Ok(needed)
    }
}

/// An owned schema type that can be decoded from and viewed over a buffer. Generated for every
/// struct, union and enum.
pub trait Message: Encode + Sized {
    /// The wire marker whose owned form is this type.
    type Wire: Wire<Owned = Self>;

    /// Verifies `bytes` and returns a zero-copy view over them.
    fn view(bytes: Bytes) -> Result<View<Self::Wire>, DecodeError> {
        View::new(bytes)
    }

    /// Verifies `bytes` and converts them to an owned value. `str` and `data` fields share
    /// `bytes` instead of copying.
    fn decode(bytes: Bytes) -> Result<Self, DecodeError> {
        decode::<Self::Wire>(&bytes)
    }

    /// Returns a copy whose `str` and `data` fields share one new buffer of exactly the encoded
    /// size, instead of the buffer this value was decoded from.
    ///
    /// A decoded value keeps its whole input alive, so a small record taken from a large frame
    /// or cached block pins all of it. Detaching costs one encode, one allocation of the encoded
    /// size and one decode; use it before storing a small value past the life of a large input.
    /// To keep only a few fields of a large value, detach those fields with
    /// [`Str::detach`](crate::Str::detach) or `Bytes::copy_from_slice` instead.
    ///
    /// A value whose encoding exceeds 4 GiB cannot be verified, so it is returned as a plain
    /// clone that still shares its buffers. That size is never a small value.
    fn detach(&self) -> Self
    where
        Self: Clone,
    {
        Self::decode(self.encode_to_bytes()).unwrap_or_else(|_| self.clone())
    }
}

/// Checks that `bytes` hold exactly one valid `W` and returns its borrowed form.
pub fn verify<W: Wire>(bytes: &Bytes) -> Result<W::Ref<'_>, DecodeError> {
    let mut verifier = Verifier::new(bytes)?;
    let value = W::verify(&mut verifier)?;
    verifier.finish()?;
    Ok(value)
}

/// Verifies `bytes` and converts them to an owned `W`, sharing `str` and `data` bytes.
///
/// Verification and construction happen in one pass over the input. The result and every
/// rejection match [`verify`] followed by [`View::to_owned`].
pub fn decode<W: Wire>(bytes: &Bytes) -> Result<W::Owned, DecodeError> {
    let mut verifier = Verifier::new(bytes)?;
    let value = W::verify_owned(&mut verifier)?;
    verifier.finish()?;
    Ok(value)
}

/// Runs a hand-written reader over `bytes` and checks that it consumed them exactly.
///
/// This is how a hand-written [`Body`](crate::Body) decodes a value field by field: `read` calls
/// [`Wire::verify_owned`] for each field's marker in order, as generated code does.
pub fn decode_with<T>(
    bytes: &Bytes,
    read: impl FnOnce(&mut Verifier<'_>) -> Result<T, DecodeError>,
) -> Result<T, DecodeError> {
    let mut verifier = Verifier::new(bytes)?;
    let value = read(&mut verifier)?;
    verifier.finish()?;
    Ok(value)
}

/// A verified message that owns its buffer. Accessing fields reads them in place.
///
/// `View` is `Send + Sync + 'static` and cheap to clone, so it can be stored or shared across
/// tasks. Struct views keep the field offsets found during verification, so [`get`](View::get)
/// does not walk the input again.
pub struct View<W: Wire> {
    bytes: Bytes,
    root: W::Root,
}

impl<W: Wire> View<W> {
    /// Verifies `bytes`, which must hold exactly one `W` with no trailing bytes.
    pub fn new(bytes: Bytes) -> Result<Self, DecodeError> {
        let root = W::root(&verify::<W>(&bytes)?);
        Ok(View { bytes, root })
    }

    /// The root value's borrowed form.
    pub fn get(&self) -> W::Ref<'_> {
        W::from_root(Reader::new(&self.bytes), self.root)
    }

    /// Converts to an owned value, sharing `str` and `data` bytes with this view's buffer.
    pub fn to_owned(&self) -> W::Owned {
        W::read_owned(&mut Reader::new(&self.bytes))
    }

    /// The verified encoded message.
    pub fn as_bytes(&self) -> &Bytes {
        &self.bytes
    }

    pub fn into_bytes(self) -> Bytes {
        self.bytes
    }
}

impl<W: Wire> Clone for View<W> {
    fn clone(&self) -> Self {
        View {
            bytes: self.bytes.clone(),
            root: self.root,
        }
    }
}

impl<W: Wire> fmt::Debug for View<W>
where
    for<'a> W::Ref<'a>: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.get(), f)
    }
}

/// Views are canonical, so two views are equal exactly when their bytes are, except that maps
/// with the same pairs in a different order compare unequal.
impl<W: Wire> PartialEq for View<W> {
    fn eq(&self, other: &Self) -> bool {
        self.bytes == other.bytes
    }
}

/// Encoding a view copies its verified bytes verbatim.
impl<W: Wire> Encode for View<W> {
    fn encoded_len(&self) -> usize {
        self.bytes.len()
    }

    fn encode<S: Sink>(&self, out: &mut S) {
        out.put_data(&self.bytes);
    }
}

/// A `void` type used as a whole message, such as an empty response, encodes to no bytes. This
/// lets a versioned envelope carry it as its latest value.
impl Encode for () {
    fn encoded_len(&self) -> usize {
        0
    }

    fn encode<S: Sink>(&self, _out: &mut S) {}
}

impl Message for () {
    type Wire = crate::wire::Void;
}
