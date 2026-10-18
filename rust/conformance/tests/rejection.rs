//! Test 8: malformed input is rejected with a precise error, and boundary-valid input is
//! accepted. Truncation at every offset and trailing bytes are also checked for every sampled
//! value in `identity::candidate_round_trips`.

use vbare::{Bytes, DecodeErrorKind, Message, View, Wire};
use vbare_conformance::{
    cand::{candidate::tags_v1 as tags, reference::all_v1 as all},
    corpus::from_hex,
};

fn verify<W: Wire>(hex: &str) -> Result<(), vbare::DecodeError> {
    vbare::verify::<W>(&Bytes::from(from_hex(hex))).map(|_| ())
}

fn kind<W: Wire>(hex: &str) -> DecodeErrorKind {
    verify::<W>(hex).expect_err(hex).kind()
}

#[test]
fn invalid_utf8() {
    assert_eq!(
        kind::<all::NameWire>("02c080"),
        DecodeErrorKind::InvalidUtf8
    );
    assert_eq!(
        kind::<all::NameWire>("03eda080"),
        DecodeErrorKind::InvalidUtf8
    );
    assert_eq!(
        kind::<all::NameWire>("04f4908080"),
        DecodeErrorKind::InvalidUtf8
    );
}

#[test]
fn boundary_valid_utf8() {
    for (hex, text) in [
        ("04f48fbfbf", "\u{10FFFF}"),
        ("04f09f9880", "\u{1F600}"),
        ("03610062", "a\0b"),
        ("00", ""),
    ] {
        let view = View::<all::NameWire>::new(Bytes::from(from_hex(hex))).unwrap();
        assert_eq!(view.get(), text);
    }
}

#[test]
fn unknown_tags() {
    assert_eq!(kind::<all::ColorWire>("03"), DecodeErrorKind::InvalidTag);
    assert_eq!(kind::<all::SparseWire>("02"), DecodeErrorKind::InvalidTag);
    assert_eq!(kind::<all::ValueWire>("07"), DecodeErrorKind::InvalidTag);
    assert_eq!(
        kind::<all::MaybeU8Wire>("0205"),
        DecodeErrorKind::InvalidTag
    );
    // Explicit union tags: 3, 4, 100 and 101 are defined.
    assert!(verify::<tags::TaggedWire>("0400").is_ok());
    assert!(verify::<tags::TaggedWire>("65").is_ok());
    for tag in ["00", "01", "02", "05", "63", "66"] {
        assert_eq!(
            kind::<tags::TaggedWire>(&format!("{tag}00")),
            DecodeErrorKind::InvalidTag
        );
    }
    // Explicit enum values: 0, 1 and 10 are defined.
    assert!(verify::<tags::LevelWire>("0a").is_ok());
    assert_eq!(kind::<tags::LevelWire>("02"), DecodeErrorKind::InvalidTag);
}

#[test]
fn varint_boundaries() {
    // 1, 2, 9 and 10 bytes are valid at their extremes.
    assert!(verify::<all::CounterWire>("7f").is_ok());
    assert!(verify::<all::CounterWire>("8001").is_ok());
    assert!(verify::<all::CounterWire>("ffffffffffffffff7f").is_ok());
    assert!(verify::<all::CounterWire>("ffffffffffffffffff01").is_ok());
    // Non-minimal at 2 and 10 bytes, a 10th byte above 1, and 11 bytes.
    assert_eq!(
        kind::<all::CounterWire>("8000"),
        DecodeErrorKind::NonCanonicalVarint
    );
    assert_eq!(
        kind::<all::CounterWire>("ffffffffffffffffff00"),
        DecodeErrorKind::NonCanonicalVarint
    );
    assert_eq!(
        kind::<all::CounterWire>("ffffffffffffffffff02"),
        DecodeErrorKind::VarintOverflow
    );
    assert_eq!(
        kind::<all::CounterWire>("ffffffffffffffffff8101"),
        DecodeErrorKind::VarintOverflow
    );
    assert_eq!(kind::<all::CounterWire>("ff"), DecodeErrorKind::Truncated);
}

#[test]
fn lengths_and_counts_above_remaining() {
    assert_eq!(
        kind::<all::NameWire>("05616263"),
        DecodeErrorKind::LengthExceedsInput
    );
    assert_eq!(
        kind::<all::DictWire>("05016101"),
        DecodeErrorKind::LengthExceedsInput
    );
    // A count of 2^64 - 1 elements with no bytes behind it.
    assert_eq!(
        kind::<all::ListsWire>("ffffffffffffffffff01"),
        DecodeErrorKind::LengthExceedsInput
    );
    // Struct elements take at least 8 bytes each, so 3 points need 24.
    let points = format!("00 00 00 03 {} 00 00 00", "00".repeat(16)).replace(' ', "");
    let error = vbare::verify::<all::ListsWire>(&Bytes::from(from_hex(&points))).unwrap_err();
    assert_eq!(error.kind(), DecodeErrorKind::LengthExceedsInput);
}

#[test]
fn errors_name_the_failing_field() {
    // Lists { numbers: [], strings: ["a", <invalid>] ... }
    let input = from_hex("00020161");
    let mut bad = input.clone();
    bad.extend(from_hex("01ff"));
    let error = all::Lists::decode(Bytes::from(bad)).unwrap_err();
    assert_eq!(error.kind(), DecodeErrorKind::InvalidUtf8);
    assert_eq!(error.path(), "strings[1]");
}
