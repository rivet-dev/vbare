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
