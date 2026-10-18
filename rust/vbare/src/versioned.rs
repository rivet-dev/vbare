//! Versioned framing: a 2-byte little-endian version header followed by the message body.

use crate::{
    error::{DecodeError, DecodeErrorKind, EncodeError},
    message::{Encode, Message, View},
    sink::Sink,
};
use bytes::Bytes;

/// A converter between adjacent versions of a versioned enum.
pub type Converter<T> = fn(T) -> anyhow::Result<T>;

/// A hand-written enum with one variant per schema version, plus converters between adjacent
/// versions. The trait provides decoding, viewing and encoding at any supported version.
///
/// Versions are numbered from 1. Payloads at [`LATEST`](Versioned::LATEST) are viewed and
/// decoded directly from the input; older payloads are decoded, upgraded, and re-encoded at the
/// latest version, so only legacy data pays for conversion.
///
/// The header is the version as a little-endian `u16`, byte-identical to vbare 0.1's
/// `serialize_with_embedded_version`.
pub trait Versioned: Sized + 'static {
    type Latest: Message + Clone;

    /// The newest version, which [`Latest`](Versioned::Latest) encodes.
    const LATEST: u16;

    /// `UPGRADES[i]` converts version `i + 1` to version `i + 2`.
    const UPGRADES: &'static [Converter<Self>] = &[];

    /// `DOWNGRADES[i]` converts version `LATEST - i` to version `LATEST - i - 1`.
    const DOWNGRADES: &'static [Converter<Self>] = &[];

    fn wrap_latest(latest: Self::Latest) -> Self;

    fn unwrap_latest(self) -> anyhow::Result<Self::Latest>;

    /// Decodes a body written at `version`, which is never [`LATEST`](Versioned::LATEST).
    fn decode_version(version: u16, body: Bytes) -> Result<Self, DecodeError>;

    /// Encodes the variant this value holds. `version` is the version being written.
    fn encode_version<S: Sink>(&self, version: u16, out: &mut S) -> Result<(), EncodeError>;

    /// Decodes a body without a header, written at `version`.
    fn decode_body(body: Bytes, version: u16) -> Result<Self::Latest, DecodeError> {
        if version == Self::LATEST {
            return Self::Latest::decode(body);
        }
        check_decode_version::<Self>(version)?;
        let mut value = Self::decode_version(version, body)?;
        let skip = usize::from(version) - 1;
        for upgrade in Self::UPGRADES.iter().skip(skip) {
            value = upgrade(value).map_err(DecodeError::conversion)?;
        }
        value.unwrap_latest().map_err(DecodeError::conversion)
    }

    /// Views a body without a header, written at `version`. Latest-version bodies are viewed in
    /// place; older bodies are converted and re-encoded first.
    fn open_body(
        body: Bytes,
        version: u16,
    ) -> Result<View<<Self::Latest as Message>::Wire>, DecodeError> {
        if version == Self::LATEST {
            return Self::Latest::view(body);
        }
        let latest = Self::decode_body(body, version)?;
        View::new(latest.encode_to_bytes())
    }

    /// Encodes `value` at `version` without a header, converting it down if needed.
    fn encode_body<S: Sink>(
        value: &Self::Latest,
        version: u16,
        out: &mut S,
    ) -> Result<(), EncodeError> {
        if version == Self::LATEST {
            value.encode(out);
            return Ok(());
        }
        downgrade::<Self>(value, version)?.encode_version(version, out)
    }

    /// Decodes a payload with a version header to the latest version.
    fn decode(bytes: Bytes) -> Result<Self::Latest, DecodeError> {
        let (version, body) = split_header(bytes)?;
        Self::decode_body(body, version)
    }

    /// Views a payload with a version header. A latest-version payload is viewed without copying.
    fn open(bytes: Bytes) -> Result<View<<Self::Latest as Message>::Wire>, DecodeError> {
        let (version, body) = split_header(bytes)?;
        Self::open_body(body, version)
    }

    /// Writes the version header and `value` encoded at `version` into `out`.
    fn seal<S: Sink>(value: &Self::Latest, version: u16, out: &mut S) -> Result<(), EncodeError> {
        if version == Self::LATEST {
            out.put_u16_le(version);
            value.encode(out);
            return Ok(());
        }
        let converted = downgrade::<Self>(value, version)?;
        out.put_u16_le(version);
        converted.encode_version(version, out)
    }

    /// [`seal`](Versioned::seal) into a new `Vec`.
    fn seal_to_vec(value: &Self::Latest, version: u16) -> Result<Vec<u8>, EncodeError> {
        let mut out = Vec::with_capacity(2 + value.encoded_len());
        Self::seal(value, version, &mut out)?;
        Ok(out)
    }

    /// [`decode`](Versioned::decode) from a slice, under vbare 0.1's name. Copies `payload` once;
    /// callers that hold `Bytes` should call `decode` instead.
    fn deserialize_with_embedded_version(payload: &[u8]) -> anyhow::Result<Self::Latest> {
        Ok(Self::decode(Bytes::copy_from_slice(payload))?)
    }

    /// [`seal_to_vec`](Versioned::seal_to_vec) under vbare 0.1's name. Takes the value by
    /// reference, so callers no longer clone it into [`wrap_latest`](Versioned::wrap_latest).
    fn serialize_with_embedded_version(
        value: &Self::Latest,
        version: u16,
    ) -> anyhow::Result<Vec<u8>> {
        Ok(Self::seal_to_vec(value, version)?)
    }

    /// [`decode_body`](Versioned::decode_body) from a slice, under vbare 0.1's name. Copies
    /// `payload` once.
    fn deserialize(payload: &[u8], version: u16) -> anyhow::Result<Self::Latest> {
        Ok(Self::decode_body(Bytes::copy_from_slice(payload), version)?)
    }

    /// [`encode_body`](Versioned::encode_body) into a new `Vec`, under vbare 0.1's name.
    fn serialize(value: &Self::Latest, version: u16) -> anyhow::Result<Vec<u8>> {
        let mut out = Vec::with_capacity(value.encoded_len());
        Self::encode_body(value, version, &mut out)?;
        Ok(out)
    }
}

fn split_header(mut bytes: Bytes) -> Result<(u16, Bytes), DecodeError> {
    if bytes.len() < 2 {
        return Err(DecodeError::with_detail(
            DecodeErrorKind::Truncated,
            bytes.len(),
            "missing version header",
        ));
    }
    let body = bytes.split_off(2);
    let version = u16::from_le_bytes([bytes[0], bytes[1]]);
    Ok((version, body))
}

fn check_decode_version<T: Versioned>(version: u16) -> Result<(), DecodeError> {
    if version == 0 || version > T::LATEST {
        return Err(DecodeError::with_detail(
            DecodeErrorKind::UnsupportedVersion,
            0,
            format!("version {version}, latest is {}", T::LATEST),
        ));
    }
    Ok(())
}

/// Converts `value` from the latest version down to `version`.
fn downgrade<T: Versioned>(value: &T::Latest, version: u16) -> Result<T, EncodeError> {
    if version == 0 || version > T::LATEST {
        return Err(EncodeError::UnsupportedVersion(version));
    }
    let steps = usize::from(T::LATEST - version);
    let mut converted = T::wrap_latest(value.clone());
    for downgrade in T::DOWNGRADES.iter().take(steps) {
        converted = downgrade(converted).map_err(EncodeError::Conversion)?;
    }
    Ok(converted)
}
