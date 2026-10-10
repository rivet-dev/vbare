mod schemas {
    include!(concat!(env!("OUT_DIR"), "/import_fixtures.rs"));
}

#[test]
fn shared_type_identity_and_round_trip_survive_transitive_imports() {
    use schemas::{
        nested_transitive_v3::{Choice, Message},
        shared_common_v1::{Fraction, SharedKey},
        shared_middle_v2::Middle,
    };
    let _: schemas::shared_common_v1::Empty = ();
    let key = SharedKey { value: 42 };
    let simple = schemas::simple_v1::Simple { key: key.clone() };
    let diamond = schemas::diamond_v1::Diamond { key: simple.key };
    let message = Message {
        middle: Middle { key: diamond.key },
        key: Some(key.clone()),
        keys: vec![key],
        fraction: Fraction { value: 0.5 },
        payload: vec![1, 2, 3],
        choice: Choice::Empty,
    };
    let bytes = serde_bare::to_vec(&message).unwrap();
    let decoded: Message = serde_bare::from_slice(&bytes).unwrap();
    assert_eq!(decoded, message);
    let with_shared_variant = Choice::SharedKey(message.middle.key);
    let bytes = serde_bare::to_vec(&with_shared_variant).unwrap();
    assert_eq!(
        serde_bare::from_slice::<Choice>(&bytes).unwrap(),
        with_shared_variant
    );
    // The imported v7 type does not change the importer's v1 module or the v1 shared type.
    let pinned = schemas::pinned_v1::Pinned {
        key: schemas::shared_common_v7::SharedKey { value: "v7".into() },
    };
    assert_eq!(pinned.key.value, "v7");
}
