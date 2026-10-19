//! Serde derives (`vbare_gen::Config::serde`): `serde_bare` writes and reads exactly the bytes the
//! codec writes, so state hashed or stored through serde keeps its existing format.

use proptest::{collection, option, prelude::*, test_runner::Config};
use std::{collections::BTreeMap, fmt::Debug};
use vbare::{Bytes, FixedBytes, Message, Str};
use vbare_conformance::serde_cand::serde_v1::{
    Empty, Event, EventMember5, Everything, Inner, Kind,
};

/// Checks one value in both directions between the codec and `serde_bare`.
fn assert_same_bytes<T>(value: &T)
where
    T: Message + serde::Serialize + serde::de::DeserializeOwned + PartialEq + Debug,
{
    let codec = value.encode_to_vec();
    let serde = serde_bare::to_vec(value).expect("serde_bare encodes the value");
    assert_eq!(serde, codec, "serde_bare and the codec disagree on {value:?}");

    let from_serde: T = serde_bare::from_slice(&codec).expect("serde_bare decodes codec bytes");
    assert_eq!(&from_serde, value);
    let from_codec = T::decode(Bytes::from(serde)).expect("the codec decodes serde_bare bytes");
    assert_eq!(&from_codec, value);
}

fn inner(seed: u8) -> Inner {
    Inner {
        id: [seed; 16],
        label: Str::from(format!("label-{seed}")),
    }
}

fn kind(index: u8) -> Kind {
    match index % 3 {
        0 => Kind::Alpha,
        1 => Kind::Beta,
        _ => Kind::Gamma,
    }
}

fn every_event() -> Vec<Event> {
    vec![
        Event::Inner(inner(1)),
        Event::Empty,
        Event::Str(Str::from_static("text")),
        Event::Data(Bytes::from_static(b"\x00\x01\xff")),
        Event::Kind(Kind::Gamma),
        Event::Member5(EventMember5 {
            at: -42,
            note: Some(Str::from_static("note")),
        }),
        Event::Member5(EventMember5 { at: 0, note: None }),
    ]
}

fn everything() -> Everything {
    Everything {
        v_u8: u8::MAX,
        v_u16: 0x1234,
        v_u32: u32::MAX,
        v_u64: u64::MAX,
        v_i8: i8::MIN,
        v_i16: -2,
        v_i32: i32::MIN,
        v_i64: i64::MIN,
        v_f32: 1.5,
        v_f64: -0.25,
        v_bool: true,
        v_str: Str::from_static("héllo"),
        v_data: Bytes::from(vec![7; 5000]),
        id: [9; 16],
        wide: FixedBytes::from([3; 40]),
        maybe: Some(inner(2)),
        kinds: vec![Kind::Alpha, Kind::Beta, Kind::Gamma],
        blobs: vec![Bytes::new(), Bytes::from_static(b"blob")],
        pair: [1, u16::MAX],
        by_name: BTreeMap::from([
            (Str::from_static("a"), Bytes::from_static(b"1")),
            (Str::from_static("b"), Bytes::new()),
        ]),
        by_id: BTreeMap::from([(1, inner(3)), (u32::MAX, inner(4))]),
        events: every_event(),
    }
}

#[test]
fn void_alias_is_unit() {
    let _: Empty = ();
}

#[test]
fn fixed_values_match() {
    assert_same_bytes(&everything());
    assert_same_bytes(&Everything {
        v_str: Str::new(),
        v_data: Bytes::new(),
        maybe: None,
        kinds: Vec::new(),
        blobs: Vec::new(),
        by_name: BTreeMap::new(),
        by_id: BTreeMap::new(),
        events: Vec::new(),
        ..everything()
    });
    for event in every_event() {
        assert_same_bytes(&event);
    }
    for index in 0..3 {
        assert_same_bytes(&kind(index));
    }
}

fn arb_inner() -> impl Strategy<Value = Inner> {
    (any::<[u8; 16]>(), ".{0,8}").prop_map(|(id, label)| Inner {
        id,
        label: Str::from(label),
    })
}

fn arb_bytes(max: usize) -> impl Strategy<Value = Bytes> {
    collection::vec(any::<u8>(), 0..max).prop_map(Bytes::from)
}

fn arb_event() -> impl Strategy<Value = Event> {
    prop_oneof![
        arb_inner().prop_map(Event::Inner),
        Just(Event::Empty),
        ".{0,8}".prop_map(|text: String| Event::Str(Str::from(text))),
        arb_bytes(16).prop_map(Event::Data),
        any::<u8>().prop_map(|index| Event::Kind(kind(index))),
        (any::<i64>(), option::of(".{0,8}")).prop_map(|(at, note)| {
            Event::Member5(EventMember5 {
                at,
                note: note.map(Str::from),
            })
        }),
    ]
}

fn arb_everything() -> impl Strategy<Value = Everything> {
    let integers = (
        any::<u8>(),
        any::<u16>(),
        any::<u32>(),
        any::<u64>(),
        any::<i8>(),
        any::<i16>(),
        any::<i32>(),
        any::<i64>(),
    );
    // NaN is excluded because it is not equal to itself; its bytes are covered by the other suites.
    let scalars = (
        any::<f32>().prop_filter("not NaN", |value| !value.is_nan()),
        any::<f64>().prop_filter("not NaN", |value| !value.is_nan()),
        any::<bool>(),
        ".{0,16}",
        arb_bytes(64),
        any::<[u8; 16]>(),
        collection::vec(any::<u8>(), 40),
    );
    let containers = (
        option::of(arb_inner()),
        collection::vec(any::<u8>().prop_map(kind), 0..4),
        collection::vec(arb_bytes(8), 0..4),
        any::<[u16; 2]>(),
        collection::btree_map(".{0,4}".prop_map(Str::from), arb_bytes(8), 0..4),
        collection::btree_map(any::<u32>(), arb_inner(), 0..4),
        collection::vec(arb_event(), 0..4),
    );
    (integers, scalars, containers).prop_map(
        |(
            (v_u8, v_u16, v_u32, v_u64, v_i8, v_i16, v_i32, v_i64),
            (v_f32, v_f64, v_bool, v_str, v_data, id, wide),
            (maybe, kinds, blobs, pair, by_name, by_id, events),
        )| {
            let wide: [u8; 40] = wide.try_into().expect("40 bytes");
            Everything {
                v_u8,
                v_u16,
                v_u32,
                v_u64,
                v_i8,
                v_i16,
                v_i32,
                v_i64,
                v_f32,
                v_f64,
                v_bool,
                v_str: Str::from(v_str),
                v_data,
                id,
                wide: FixedBytes::from(wide),
                maybe,
                kinds,
                blobs,
                pair,
                by_name,
                by_id,
                events,
            }
        },
    )
}

proptest! {
    #![proptest_config(Config::with_cases(512))]

    #[test]
    fn arbitrary_values_match(value in arb_everything()) {
        assert_same_bytes(&value);
    }
}
