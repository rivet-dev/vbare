//! `Versioned` conversion and framing with hand-written message types.

use anyhow::{bail, Result};
use vbare::{
    wire::{self, Wire},
    Bytes, DecodeError, DecodeErrorKind, Encode, EncodeError, Message, Reader, Sink, Str, Verifier,
    Versioned,
};

/// Defines a struct with a hand-written `Wire` implementation, as generated code would.
macro_rules! message {
    ($name:ident, $wire:ident { $($field:ident: $field_wire:ty),* $(,)? }) => {
        #[derive(Clone, Debug, PartialEq)]
        struct $name {
            $($field: <$field_wire as Wire>::Owned,)*
        }

        struct $wire;

        // SAFETY: every reader visits the fields in the order `verify` checks them.
        unsafe impl Wire for $wire {
            type Ref<'a> = ($(<$field_wire as Wire>::Ref<'a>,)*);
            type Owned = $name;
            type Root = ();

            const FIXED_SIZE: Option<usize> = None;
            const MIN_SIZE: usize = 1;

            fn verify<'a>(v: &mut Verifier<'a>) -> Result<Self::Ref<'a>, DecodeError> {
                Ok(($(<$field_wire as Wire>::verify(v)?,)*))
            }

            fn read<'a>(r: &mut Reader<'a>) -> Self::Ref<'a> {
                ($(<$field_wire as Wire>::read(r),)*)
            }

            fn root(_: &Self::Ref<'_>) {}

            fn from_root(r: Reader<'_>, _: ()) -> Self::Ref<'_> {
                Self::read_span(r)
            }

            fn read_owned(r: &mut Reader<'_>) -> $name {
                $name { $($field: <$field_wire as Wire>::read_owned(r),)* }
            }

            fn encoded_len(value: &$name) -> usize {
                0 $(+ <$field_wire as Wire>::encoded_len(&value.$field))*
            }

            fn encode<S: Sink>(value: &$name, out: &mut S) {
                $(<$field_wire as Wire>::encode(&value.$field, out);)*
            }

            fn ref_encoded_len(value: Self::Ref<'_>) -> usize {
                let ($($field,)*) = value;
                0 $(+ <$field_wire as Wire>::ref_encoded_len($field))*
            }

            fn encode_ref<S: Sink>(value: Self::Ref<'_>, out: &mut S) {
                let ($($field,)*) = value;
                $(<$field_wire as Wire>::encode_ref($field, out);)*
            }
        }

        impl Encode for $name {
            fn encoded_len(&self) -> usize {
                <$wire as Wire>::encoded_len(self)
            }

            fn encode<S: Sink>(&self, out: &mut S) {
                <$wire as Wire>::encode(self, out)
            }
        }

        impl Message for $name {
            type Wire = $wire;
        }
    };
}

message!(
    TestDataV1,
    TestDataV1Wire {
        id: wire::U32,
        name: wire::Str
    }
);
message!(
    TestDataV2,
    TestDataV2Wire {
        id: wire::U32,
        name: wire::Str,
        description: wire::Str,
    }
);
message!(TestDataV3, TestDataV3Wire {
    id: wire::U32,
    name: wire::Str,
    description: wire::Str,
    tags: wire::List<wire::Str>,
});

#[derive(Clone)]
enum TestData {
    V1(TestDataV1),
    V2(TestDataV2),
    V3(TestDataV3),
}

impl Versioned for TestData {
    type Latest = TestDataV3;

    const LATEST: u16 = 3;
    const UPGRADES: &'static [vbare::Converter<Self>] = &[Self::v1_to_v2, Self::v2_to_v3];
    const DOWNGRADES: &'static [vbare::Converter<Self>] = &[Self::v3_to_v2, Self::v2_to_v1];

    fn wrap_latest(latest: TestDataV3) -> Self {
        TestData::V3(latest)
    }

    fn unwrap_latest(self) -> Result<TestDataV3> {
        match self {
            TestData::V3(data) => Ok(data),
            TestData::V1(_) | TestData::V2(_) => bail!("version not latest"),
        }
    }

    fn decode_version(version: u16, body: Bytes) -> Result<Self, DecodeError> {
        match version {
            1 => Ok(TestData::V1(TestDataV1::decode(body)?)),
            2 => Ok(TestData::V2(TestDataV2::decode(body)?)),
            3 => Ok(TestData::V3(TestDataV3::decode(body)?)),
            _ => Err(DecodeError::new(DecodeErrorKind::UnsupportedVersion, 0)),
        }
    }

    fn encode_version<S: Sink>(&self, _version: u16, out: &mut S) -> Result<(), EncodeError> {
        match self {
            TestData::V1(data) => data.encode(out),
            TestData::V2(data) => data.encode(out),
            TestData::V3(data) => data.encode(out),
        }
        Ok(())
    }
}

impl TestData {
    fn v1_to_v2(self) -> Result<Self> {
        match self {
            TestData::V1(v1) => Ok(TestData::V2(TestDataV2 {
                id: v1.id,
                name: v1.name,
                description: Str::from("default"),
            })),
            other => Ok(other),
        }
    }

    fn v2_to_v3(self) -> Result<Self> {
        match self {
            TestData::V2(v2) => Ok(TestData::V3(TestDataV3 {
                id: v2.id,
                name: v2.name,
                description: v2.description,
                tags: Vec::new(),
            })),
            other => Ok(other),
        }
    }

    fn v3_to_v2(self) -> Result<Self> {
        match self {
            TestData::V3(v3) => Ok(TestData::V2(TestDataV2 {
                id: v3.id,
                name: v3.name,
                description: v3.description,
            })),
            other => Ok(other),
        }
    }

    fn v2_to_v1(self) -> Result<Self> {
        match self {
            TestData::V2(v2) => Ok(TestData::V1(TestDataV1 {
                id: v2.id,
                name: v2.name,
            })),
            other => Ok(other),
        }
    }
}

/// A single-version family with no converters.
#[derive(Clone)]
enum OnlyV1 {
    V1(TestDataV1),
}

impl Versioned for OnlyV1 {
    type Latest = TestDataV1;

    const LATEST: u16 = 1;

    fn wrap_latest(latest: TestDataV1) -> Self {
        OnlyV1::V1(latest)
    }

    fn unwrap_latest(self) -> Result<TestDataV1> {
        match self {
            OnlyV1::V1(data) => Ok(data),
        }
    }

    fn decode_version(version: u16, _body: Bytes) -> Result<Self, DecodeError> {
        Err(DecodeError::with_detail(
            DecodeErrorKind::UnsupportedVersion,
            0,
            format!("version {version}"),
        ))
    }

    fn encode_version<S: Sink>(&self, _version: u16, out: &mut S) -> Result<(), EncodeError> {
        match self {
            OnlyV1::V1(data) => data.encode(out),
        }
        Ok(())
    }
}

fn latest() -> TestDataV3 {
    TestDataV3 {
        id: 999,
        name: "v3".into(),
        description: "described".into(),
        tags: vec!["a".into(), "b".into()],
    }
}

fn sealed(value: &TestDataV3, version: u16) -> Bytes {
    Bytes::from(TestData::seal_to_vec(value, version).unwrap())
}

#[test]
fn upgrades_older_bodies() {
    let v1 = TestDataV1 {
        id: 456,
        name: "test".into(),
    };
    let decoded = TestData::decode_body(v1.encode_to_bytes(), 1).unwrap();
    assert_eq!(decoded.id, 456);
    assert_eq!(decoded.name, "test");
    assert_eq!(decoded.description, "default");
    assert!(decoded.tags.is_empty());

    let v2 = TestDataV2 {
        id: 1,
        name: "n".into(),
        description: "kept".into(),
    };
    let decoded = TestData::decode_body(v2.encode_to_bytes(), 2).unwrap();
    assert_eq!(decoded.description, "kept");
}

#[test]
fn downgrades_when_encoding_older_versions() {
    let value = latest();
    let mut v2 = Vec::new();
    TestData::encode_body(&value, 2, &mut v2).unwrap();
    let decoded = TestDataV2::decode(Bytes::from(v2)).unwrap();
    assert_eq!(decoded.description, "described");

    let mut v1 = Vec::new();
    TestData::encode_body(&value, 1, &mut v1).unwrap();
    let decoded = TestDataV1::decode(Bytes::from(v1)).unwrap();
    assert_eq!(decoded.id, 999);
    assert_eq!(decoded.name, "v3");
}

#[test]
fn round_trips_every_version_with_header() {
    let value = latest();
    for version in 1..=3u16 {
        let bytes = sealed(&value, version);
        assert_eq!(bytes[..2], version.to_le_bytes());
        let decoded = TestData::decode(bytes.clone()).unwrap();
        assert_eq!(decoded.id, 999);
        assert_eq!(decoded.name, "v3");
        let opened = TestData::open(bytes).unwrap();
        assert_eq!(opened.to_owned(), decoded);
    }
    let decoded = TestData::decode(sealed(&value, 3)).unwrap();
    assert_eq!(decoded, value);
}

#[test]
fn latest_payloads_open_without_copying() {
    let bytes = sealed(&latest(), 3);
    let view = TestData::open(bytes.clone()).unwrap();
    assert_eq!(view.as_bytes().as_ptr(), bytes[2..].as_ptr());
    let (id, name, _, tags) = view.get();
    assert_eq!(id, 999);
    assert_eq!(name, "v3");
    assert_eq!(tags.iter().collect::<Vec<_>>(), ["a", "b"]);
}

#[test]
fn header_matches_vbare_0_1() {
    // vbare 0.1 wrote the version as a little-endian u16 before the body.
    let value = TestDataV1 {
        id: 1,
        name: "x".into(),
    };
    let bytes = OnlyV1::seal_to_vec(&value, 1).unwrap();
    assert_eq!(bytes, [1, 0, 1, 0, 0, 0, 1, b'x']);
}

#[test]
fn rejects_unsupported_versions() {
    let value = latest();
    for version in [0u16, 4, 99] {
        let mut body = 3u32.to_le_bytes().to_vec();
        body.splice(0..0, version.to_le_bytes());
        let error = TestData::decode(Bytes::from(body)).unwrap_err();
        assert_eq!(error.kind(), DecodeErrorKind::UnsupportedVersion);
        assert!(matches!(
            TestData::seal_to_vec(&value, version),
            Err(EncodeError::UnsupportedVersion(_))
        ));
    }
    let error = TestData::decode(Bytes::from_static(&[1])).unwrap_err();
    assert_eq!(error.kind(), DecodeErrorKind::Truncated);
}

#[test]
fn no_converters() {
    let value = TestDataV1 {
        id: 7,
        name: "only".into(),
    };
    let bytes = Bytes::from(OnlyV1::seal_to_vec(&value, 1).unwrap());
    assert_eq!(OnlyV1::decode(bytes).unwrap(), value);
    assert_eq!(
        OnlyV1::decode(Bytes::from_static(&[2, 0]))
            .unwrap_err()
            .kind(),
        DecodeErrorKind::UnsupportedVersion
    );
}

#[test]
fn slice_helpers_match_the_bytes_api() {
    let value = latest();
    for version in 1..=3u16 {
        let sealed = TestData::serialize_with_embedded_version(&value, version).unwrap();
        assert_eq!(sealed, TestData::seal_to_vec(&value, version).unwrap());
        assert_eq!(
            TestData::deserialize_with_embedded_version(&sealed).unwrap(),
            TestData::decode(Bytes::from(sealed.clone())).unwrap()
        );

        let body = TestData::serialize(&value, version).unwrap();
        assert_eq!(body, sealed[2..]);
        assert_eq!(
            TestData::deserialize(&body, version).unwrap(),
            TestData::decode_body(Bytes::from(body.clone()), version).unwrap()
        );
    }
    assert_eq!(
        TestData::deserialize_with_embedded_version(&TestData::seal_to_vec(&value, 3).unwrap())
            .unwrap(),
        value
    );

    let error = TestData::deserialize_with_embedded_version(&[9, 0]).unwrap_err();
    assert_eq!(
        error.downcast_ref::<DecodeError>().map(DecodeError::kind),
        Some(DecodeErrorKind::UnsupportedVersion)
    );
    let error = TestData::serialize(&value, 0).unwrap_err();
    assert!(matches!(
        error.downcast_ref::<EncodeError>(),
        Some(EncodeError::UnsupportedVersion(0))
    ));
}

#[test]
fn seals_into_caller_buffers_at_every_version() {
    let value = latest();
    assert_eq!(
        TestData::sealed_len(&value),
        TestData::seal_to_vec(&value, 3).unwrap().len()
    );

    for version in 1..=3u16 {
        let expected = TestData::seal_to_vec(&value, version).unwrap();

        let bytes = TestData::seal_to_bytes(&value, version).unwrap();
        assert_eq!(bytes, expected);

        // The message lands at the front of a larger buffer and the rest is untouched.
        let mut out = [0xaa; 64];
        let written = TestData::seal_to_slice(&value, version, &mut out).unwrap();
        assert_eq!(&out[..written], &expected[..]);
        assert!(out[written..].iter().all(|byte| *byte == 0xaa));

        // An exact fit works; one byte less fails without writing.
        let mut exact = vec![0; expected.len()];
        assert_eq!(
            TestData::seal_to_slice(&value, version, &mut exact).unwrap(),
            expected.len()
        );
        assert_eq!(exact, expected);
        let mut short = vec![0; expected.len() - 1];
        assert!(matches!(
            TestData::seal_to_slice(&value, version, &mut short),
            Err(EncodeError::BufferTooSmall { needed, available })
                if needed == expected.len() && available == expected.len() - 1
        ));
        assert!(short.iter().all(|byte| *byte == 0));
    }
}

#[test]
fn seals_behind_existing_bytes_in_one_buffer() {
    // A caller framing the message behind its own prefix seals straight into its buffer, so the
    // body is never encoded into a buffer of its own and copied.
    let value = latest();
    let mut frame = vbare::BytesMut::with_capacity(4 + TestData::sealed_len(&value));
    frame.extend_from_slice(
        &u32::try_from(TestData::sealed_len(&value))
            .unwrap()
            .to_be_bytes(),
    );
    let capacity = frame.capacity();
    TestData::seal(&value, 3, &mut frame).unwrap();
    assert_eq!(frame.capacity(), capacity);
    assert_eq!(&frame[4..], &TestData::seal_to_vec(&value, 3).unwrap()[..]);
}

/// Whether `inner` points into `outer`'s memory.
fn shares(outer: &[u8], inner: &[u8]) -> bool {
    let range = outer.as_ptr_range();
    range.contains(&inner.as_ptr())
}

#[test]
fn detached_values_stop_sharing_the_source_buffer() {
    // A small record at the front of a 1 MiB buffer, as a record decoded from a large frame or a
    // cached block would be.
    let record = TestDataV2 {
        id: 5,
        name: "short-name".into(),
        description: "short-description".into(),
    };
    let mut buffer = record.encode_to_vec();
    let len = buffer.len();
    buffer.resize(1 << 20, 0);
    let source = Bytes::from(buffer);

    let decoded = TestDataV2::decode(source.slice(..len)).unwrap();
    assert!(shares(&source, decoded.name.as_bytes()));

    let name = decoded.name.detach();
    assert_eq!(name, decoded.name);
    assert!(!shares(&source, name.as_bytes()));

    let detached = decoded.detach();
    assert_eq!(detached, decoded);
    assert!(!shares(&source, detached.name.as_bytes()));
    assert!(!shares(&source, detached.description.as_bytes()));
    // Both fields share one new buffer the size of the record.
    let gap = detached.description.as_ptr() as usize - detached.name.as_ptr() as usize;
    assert!(gap < len);

    let source = Bytes::from(vec![7u8; 4096]);
    let fixed = vbare::FixedBytes::<40>::new(source.slice(..40)).unwrap();
    let fixed_detached = fixed.detach();
    assert_eq!(fixed_detached, fixed);
    assert!(!shares(&source, fixed_detached.as_bytes()));
}
