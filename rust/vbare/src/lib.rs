//! Runtime for VBARE: a zero-copy BARE codec plus versioned framing.
//!
//! Generated code describes each schema type with a [`Wire`] marker. Decoding verifies the input
//! once ([`View::new`], [`Message::view`]) and then reads fields in place; owned values
//! ([`Message::decode`]) share `str` and `data` bytes with the input instead of copying them.
//! Encoding ([`Encode`]) writes once into a [`Sink`] with an exact precomputed length.

mod error;
mod fixed;
mod list;
mod map;
mod message;
mod read;
mod sink;
mod str;
pub mod varint;
mod versioned;
pub mod wire;

pub use bytes::{self, Bytes, BytesMut};
pub use error::{DecodeError, DecodeErrorKind, EncodeError};
pub use fixed::FixedBytes;
pub use list::{ListIter, ListRef};
pub use map::{MapIter, MapRef};
pub use message::{decode, verify, Encode, Message, View};
pub use read::{Reader, Verifier};
pub use sink::Sink;
pub use str::Str;
pub use versioned::{Converter, Versioned};
pub use wire::{Key, Wire};

use anyhow::{bail, Context, Result};

pub trait VersionedData<'a>: Sized {
    type Latest;

    fn wrap_latest(latest: Self::Latest) -> Self;
    fn unwrap_latest(self) -> Result<Self::Latest>;
    fn deserialize_version(payload: &'a [u8], version: u16) -> Result<Self>;
    fn serialize_version(self, version: u16) -> Result<Vec<u8>>;

    fn deserialize_converters() -> Vec<impl Fn(Self) -> Result<Self>> {
        // HACK: We do this for the type checker
        if false {
            vec![identity]
        } else {
            Vec::new()
        }
    }

    fn serialize_converters() -> Vec<impl Fn(Self) -> Result<Self>> {
        // HACK: We do this for the type checker
        if false {
            vec![identity]
        } else {
            Vec::new()
        }
    }

    fn deserialize(payload: &'a [u8], version: u16) -> Result<Self::Latest> {
        let mut data = Self::deserialize_version(payload, version)?;

        let skip_count = version
            .checked_sub(1)
            .with_context(|| format!("protocol version ({version}) must be > 0"))?;
        for converter in Self::deserialize_converters()
            .iter()
            .skip(skip_count as usize)
        {
            data = converter(data)?;
        }

        data.unwrap_latest()
    }

    fn serialize(self, version: u16) -> Result<Vec<u8>> {
        let mut data = self;

        let converters = Self::serialize_converters();
        let take_count = (converters.len() + 1)
            .checked_sub(version as usize)
            .with_context(|| {
                format!(
                    "protocol version ({version}) greater than latest version ({})",
                    converters.len() + 1
                )
            })?;
        for converter in converters.iter().take(take_count) {
            data = converter(data)?;
        }

        Self::serialize_version(data, version)
    }

    /// Serializes data with the version encoded as the first byte.
    fn deserialize_with_embedded_version(payload: &'a [u8]) -> Result<Self::Latest> {
        if payload.len() < 2 {
            bail!("payload too short for embedded version");
        }

        let version = u16::from_le_bytes([payload[0], payload[1]]);
        let payload = &payload[2..];

        Self::deserialize(payload, version)
    }

    /// Deserializes data with the version encoded as the first byte.
    fn serialize_with_embedded_version(self, version: u16) -> Result<Vec<u8>> {
        let payload = self.serialize(version)?;
        let mut result = Vec::with_capacity(2 + payload.len());
        result.extend_from_slice(&version.to_le_bytes());
        result.extend_from_slice(&payload);
        Ok(result)
    }
}

pub trait OwnedVersionedData: Sized {
    type Latest;

    fn wrap_latest(latest: Self::Latest) -> Self;
    fn unwrap_latest(self) -> Result<Self::Latest>;
    fn deserialize_version(payload: &[u8], version: u16) -> Result<Self>;
    fn serialize_version(self, version: u16) -> Result<Vec<u8>>;

    fn deserialize_converters() -> Vec<impl Fn(Self) -> Result<Self>> {
        // HACK: We do this for the type checker
        if false {
            vec![identity]
        } else {
            Vec::new()
        }
    }

    fn serialize_converters() -> Vec<impl Fn(Self) -> Result<Self>> {
        // HACK: We do this for the type checker
        if false {
            vec![identity]
        } else {
            Vec::new()
        }
    }

    fn deserialize(payload: &[u8], version: u16) -> Result<Self::Latest> {
        let mut data = Self::deserialize_version(payload, version)?;

        let skip_count = version
            .checked_sub(1)
            .with_context(|| format!("protocol version ({version}) must be > 0"))?;
        for converter in Self::deserialize_converters()
            .iter()
            .skip(skip_count as usize)
        {
            data = converter(data)?;
        }

        data.unwrap_latest()
    }

    fn serialize(self, version: u16) -> Result<Vec<u8>> {
        let mut data = self;

        let converters = Self::serialize_converters();
        let take_count = (converters.len() + 1)
            .checked_sub(version as usize)
            .with_context(|| {
                format!(
                    "protocol version ({version}) greater than latest version ({})",
                    converters.len() + 1
                )
            })?;
        for converter in converters.iter().take(take_count) {
            data = converter(data)?;
        }

        Self::serialize_version(data, version)
    }

    // See VersionedData::deserialize_with_embedded_version.
    fn deserialize_with_embedded_version(payload: &[u8]) -> Result<Self::Latest> {
        if payload.len() < 2 {
            bail!("payload too short for embedded version");
        }

        let version = u16::from_le_bytes([payload[0], payload[1]]);
        let payload = &payload[2..];

        Self::deserialize(payload, version)
    }

    // See VersionedData::serialize_with_embedded_version.
    fn serialize_with_embedded_version(self, version: u16) -> Result<Vec<u8>> {
        let payload = self.serialize(version)?;
        let mut result = Vec::with_capacity(2 + payload.len());
        result.extend_from_slice(&version.to_le_bytes());
        result.extend_from_slice(&payload);
        Ok(result)
    }
}

/// Helper for default trait methods.
fn identity<T>(v: T) -> Result<T> {
    Ok(v)
}
