mod schemas {
    #![allow(dead_code)]
    include!(concat!(env!("OUT_DIR"), "/import_fixtures.rs"));
}

use vbare::{Bytes, Encode, Message};

#[test]
fn shared_type_identity_and_round_trip_survive_transitive_imports() {
    use schemas::{
        nested_transitive_v3::{Choice, Message as Envelope},
        shared_common_v1::{Fraction, SharedKey},
        shared_middle_v2::Middle,
    };
    let _: schemas::shared_common_v1::Empty = ();
    let key = SharedKey { value: 42 };
    let simple = schemas::simple_v1::Simple { key: key.clone() };
    let diamond = schemas::diamond_v1::Diamond { key: simple.key };
    let message = Envelope {
        middle: Middle { key: diamond.key },
        key: Some(key.clone()),
        keys: vec![key],
        fraction: Fraction { value: 0.5 },
        payload: Bytes::from_static(&[1, 2, 3]),
        choice: Choice::Empty,
    };
    let bytes = message.encode_to_bytes();
    let decoded = Envelope::decode(bytes.clone()).unwrap();
    assert_eq!(decoded, message);
    let view = Envelope::view(bytes).unwrap();
    assert_eq!(view.get().middle().key().value(), 42);
    assert_eq!(view.get().payload(), [1, 2, 3]);

    let with_shared_variant = Choice::SharedKey(message.middle.key);
    let bytes = with_shared_variant.encode_to_bytes();
    assert_eq!(Choice::decode(bytes).unwrap(), with_shared_variant);
    // The imported v7 type does not change the importer's v1 module or the v1 shared type.
    let pinned = schemas::pinned_v1::Pinned {
        key: schemas::shared_common_v7::SharedKey { value: "v7".into() },
    };
    assert_eq!(pinned.key.value, "v7");
}
