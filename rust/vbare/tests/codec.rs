//! Runtime codec behavior over the built-in wire markers.

use std::collections::BTreeMap;
use vbare::{
    decode, varint, verify,
    wire::{self, Wire},
    Bytes, DecodeErrorKind, Encode, FixedBytes, Str, View,
};

fn bytes(input: &[u8]) -> Bytes {
    Bytes::copy_from_slice(input)
}

fn encode<W: Wire>(value: &W::Owned) -> Vec<u8> {
    let mut out = Vec::new();
    W::encode(value, &mut out);
    assert_eq!(out.len(), W::encoded_len(value));
    out
}

fn reject<W: Wire>(input: &[u8]) -> DecodeErrorKind {
    match verify::<W>(&bytes(input)) {
        Ok(_) => panic!("accepted {input:02x?}"),
        Err(error) => error.kind(),
    }
}

#[test]
fn uint_boundaries() {
    for (value, len) in [
        (0u64, 1),
        (127, 1),
        (128, 2),
        (16_383, 2),
        (16_384, 3),
        ((1 << 56) - 1, 8),
        (1 << 56, 9),
        ((1 << 63) - 1, 9),
        (1 << 63, 10),
        (u64::MAX, 10),
    ] {
        let encoded = encode::<wire::Uint>(&value);
        assert_eq!(encoded.len(), len, "{value}");
        assert_eq!(varint::uint_len(value), len);
        assert_eq!(decode::<wire::Uint>(&bytes(&encoded)).unwrap(), value);
    }
}

#[test]
fn int_boundaries() {
    for value in [0i64, 1, -1, 63, -64, 64, -65, i64::MAX, i64::MIN] {
        let encoded = encode::<wire::Int>(&value);
        assert_eq!(decode::<wire::Int>(&bytes(&encoded)).unwrap(), value);
        assert_eq!(varint::unzigzag(varint::zigzag(value)), value);
    }
    assert_eq!(encode::<wire::Int>(&-1), [0x01]);
    assert_eq!(encode::<wire::Int>(&-65), [0x81, 0x01]);
    assert_eq!(
        encode::<wire::Int>(&i64::MIN),
        [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01]
    );
}

#[test]
fn varint_rejections() {
    assert_eq!(reject::<wire::Uint>(&[]), DecodeErrorKind::Truncated);
    assert_eq!(reject::<wire::Uint>(&[0x80]), DecodeErrorKind::Truncated);
    assert_eq!(
        reject::<wire::Uint>(&[0x80, 0x00]),
        DecodeErrorKind::NonCanonicalVarint
    );
    assert_eq!(
        reject::<wire::Uint>(&[0xff, 0x80, 0x00]),
        DecodeErrorKind::NonCanonicalVarint
    );
    let mut ten = [0xffu8; 10];
    ten[9] = 0x02;
    assert_eq!(reject::<wire::Uint>(&ten), DecodeErrorKind::VarintOverflow);
    ten[9] = 0x81;
    assert_eq!(reject::<wire::Uint>(&ten), DecodeErrorKind::VarintOverflow);
    ten[9] = 0x00;
    assert_eq!(
        reject::<wire::Uint>(&ten),
        DecodeErrorKind::NonCanonicalVarint
    );
    assert_eq!(
        reject::<wire::Uint>(&[0x00, 0x00]),
        DecodeErrorKind::TrailingBytes
    );
}

#[test]
fn fixed_numbers_and_bools() {
    assert_eq!(encode::<wire::U32>(&255), [0xff, 0, 0, 0]);
    assert_eq!(encode::<wire::I16>(&-255), [0x01, 0xff]);
    assert_eq!(
        encode::<wire::F64>(&-25.5),
        [0, 0, 0, 0, 0, 0x80, 0x39, 0xc0]
    );
    let nan = f32::from_bits(0x7f80_0001);
    let decoded = decode::<wire::F32>(&bytes(&encode::<wire::F32>(&nan))).unwrap();
    assert_eq!(decoded.to_bits(), 0x7f80_0001);
    assert!(decode::<wire::Bool>(&bytes(&[1])).unwrap());
    assert_eq!(reject::<wire::Bool>(&[2]), DecodeErrorKind::InvalidBool);
    assert_eq!(reject::<wire::U16>(&[1]), DecodeErrorKind::Truncated);
}

#[test]
fn strings_share_the_input() {
    let input = bytes(&[4, b'B', b'A', b'R', b'E']);
    let owned = decode::<wire::Str>(&input).unwrap();
    assert_eq!(owned, "BARE");
    assert_eq!(owned.as_bytes().as_ptr(), input[1..].as_ptr());
    assert_eq!(*verify::<wire::Str>(&input).unwrap(), *"BARE");
    assert_eq!(encode::<wire::Str>(&Str::from("BARE")), input);

    assert_eq!(
        reject::<wire::Str>(&[2, 0xc0, 0x80]),
        DecodeErrorKind::InvalidUtf8
    );
    assert_eq!(
        reject::<wire::Str>(&[3, 0xed, 0xa0, 0x80]),
        DecodeErrorKind::InvalidUtf8
    );
    assert_eq!(
        reject::<wire::Str>(&[4, 0xf4, 0x90, 0x80, 0x80]),
        DecodeErrorKind::InvalidUtf8
    );
    assert_eq!(
        reject::<wire::Str>(&[5, b'a']),
        DecodeErrorKind::LengthExceedsInput
    );
    let error = verify::<wire::Str>(&bytes(&[3, b'a', 0xff, b'b'])).unwrap_err();
    assert_eq!(error.offset(), 2);
}

#[test]
fn data_and_fixed_data() {
    let input = bytes(&[3, 1, 2, 3]);
    assert_eq!(
        decode::<wire::Data>(&input).unwrap(),
        Bytes::from_static(&[1, 2, 3])
    );
    assert_eq!(
        decode::<wire::FixedData<3>>(&bytes(&[1, 2, 3])).unwrap(),
        [1, 2, 3]
    );
    let big = decode::<wire::FixedDataBytes<40>>(&bytes(&[7; 40])).unwrap();
    assert_eq!(big, FixedBytes::from([7; 40]));
    assert_eq!(
        reject::<wire::FixedData<3>>(&[1, 2]),
        DecodeErrorKind::Truncated
    );
}

#[test]
fn optionals() {
    type W = wire::Optional<wire::U8>;
    assert_eq!(decode::<W>(&bytes(&[0])).unwrap(), None);
    assert_eq!(decode::<W>(&bytes(&[1, 9])).unwrap(), Some(9));
    assert_eq!(reject::<W>(&[2, 9]), DecodeErrorKind::InvalidTag);
    assert_eq!(reject::<W>(&[1]), DecodeErrorKind::Truncated);
}

#[test]
fn lists() {
    type W = wire::List<wire::Str>;
    let input = bytes(&[
        3, 3, b'f', b'o', b'o', 3, b'b', b'a', b'r', 4, b'b', b'u', b'z', b'z',
    ]);
    let view = View::<W>::new(input.clone()).unwrap();
    let list = view.get();
    assert_eq!(list.len(), 3);
    assert_eq!(list.iter().collect::<Vec<_>>(), ["foo", "bar", "buzz"]);
    assert_eq!(list.get(2), Some("buzz"));
    assert_eq!(list.get(3), None);
    assert_eq!(view.to_owned(), ["foo", "bar", "buzz"]);
    assert_eq!(view.encode_to_vec(), input);
    let mut reencoded = Vec::new();
    W::encode_ref(list, &mut reencoded);
    assert_eq!(reencoded, input);

    assert_eq!(reject::<W>(&[2, 0]), DecodeErrorKind::LengthExceedsInput);
    let error = verify::<W>(&bytes(&[2, 0, 1, 0xff])).unwrap_err();
    assert_eq!(error.kind(), DecodeErrorKind::InvalidUtf8);
    assert_eq!(error.path(), "[1]");

    type Fixed = wire::FixedList<wire::U16, 3>;
    let fixed = View::<Fixed>::new(bytes(&[1, 0, 2, 0, 3, 0])).unwrap();
    assert_eq!(fixed.get().get(1), Some(2));
    assert_eq!(fixed.to_owned(), [1, 2, 3]);
}

#[test]
fn maps() {
    type W = wire::Map<wire::U32, wire::Str>;
    let input = bytes(&[
        3, 0, 0, 0, 0, 4, b'z', b'e', b'r', b'o', 1, 0, 0, 0, 3, b'o', b'n', b'e', 0xff, 0, 0, 0,
        3, b't', b'w', b'o',
    ]);
    let view = View::<W>::new(input.clone()).unwrap();
    assert_eq!(view.get().get(1), Some("one"));
    assert_eq!(view.get().get(2), None);
    let owned = view.to_owned();
    assert_eq!(
        owned,
        BTreeMap::from([(0, "zero".into()), (1, "one".into()), (255, "two".into())])
    );
    assert_eq!(encode::<W>(&owned), input);

    // Pairs in any order decode; duplicates do not.
    let reordered = bytes(&[2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(decode::<W>(&reordered).unwrap().len(), 2);
    assert_eq!(
        reject::<W>(&[2, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0]),
        DecodeErrorKind::DuplicateMapKey
    );
    assert_eq!(
        reject::<W>(&[3, 2, 0, 0, 0, 0, 1, 0, 0, 0, 0, 2, 0, 0, 0, 0]),
        DecodeErrorKind::DuplicateMapKey
    );
    // An unsorted map's encoding is preserved by the view.
    let view = View::<W>::new(reordered.clone()).unwrap();
    let mut reencoded = Vec::new();
    W::encode_ref(view.get(), &mut reencoded);
    assert_eq!(reencoded, reordered);
}

#[test]
fn trailing_bytes_and_slices() {
    assert_eq!(reject::<wire::U8>(&[1, 2]), DecodeErrorKind::TrailingBytes);
    let value = 0x1234u16;
    let mut out = [0u8; 1];
    struct Wrapper(u16);
    impl Encode for Wrapper {
        fn encoded_len(&self) -> usize {
            wire::U16::encoded_len(&self.0)
        }
        fn encode<S: vbare::Sink>(&self, out: &mut S) {
            wire::U16::encode(&self.0, out)
        }
    }
    assert!(Wrapper(value).encode_to_slice(&mut out).is_err());
    let mut out = [0u8; 3];
    assert_eq!(Wrapper(value).encode_to_slice(&mut out).unwrap(), 2);
    assert_eq!(out, [0x34, 0x12, 0]);
}
