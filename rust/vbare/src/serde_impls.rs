//! `serde` impls for [`Str`] and [`FixedBytes`], used by code generated with
//! `vbare_gen::Config::serde`.
//!
//! Each impl uses the serde data model shape that makes `serde_bare` write the codec's bytes: a
//! `Str` is a string and a `FixedBytes<N>` is a tuple of `N` bytes, like `[u8; N]`. `Bytes` uses
//! the `bytes` crate's own impls, which serialize as a byte string.

use crate::{FixedBytes, Str};
use bytes::Bytes;
use serde::{
    de::{Error, SeqAccess, Visitor},
    ser::SerializeTuple,
    Deserialize, Deserializer, Serialize, Serializer,
};
use std::fmt;

impl Serialize for Str {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Str {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        String::deserialize(deserializer).map(Str::from)
    }
}

impl<const N: usize> Serialize for FixedBytes<N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(N)?;
        for byte in self.as_array() {
            tuple.serialize_element(byte)?;
        }
        tuple.end()
    }
}

impl<'de, const N: usize> Deserialize<'de> for FixedBytes<N> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_tuple(N, FixedBytesVisitor::<N>)
    }
}

struct FixedBytesVisitor<const N: usize>;

impl<'de, const N: usize> Visitor<'de> for FixedBytesVisitor<N> {
    type Value = FixedBytes<N>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a tuple of {N} bytes")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        let mut bytes = Vec::with_capacity(N);
        for index in 0..N {
            let byte = seq
                .next_element::<u8>()?
                .ok_or_else(|| A::Error::invalid_length(index, &self))?;
            bytes.push(byte);
        }
        FixedBytes::new(Bytes::from(bytes))
            .map_err(|bytes| A::Error::invalid_length(bytes.len(), &self))
    }
}
