//! Test 5: every encoded example in draft-devault-bare-11 Appendices A and B (Appendix C has no
//! encoded examples). The vectors are in `vectors/spec.json`; bare-ts checks the same file in
//! `bare_ts.rs`.

use std::collections::BTreeMap;
use vbare::{Bytes, Encode, Message};
use vbare_conformance::{
    cand::spec::{appendix_a_v1 as a, appendix_b_v1 as b},
    corpus,
};

fn spec_vectors() -> serde_json::Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/vectors/spec.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("read spec vectors"))
        .expect("parse spec vectors")
}

/// Every vector decodes to its JSON value and re-encodes to its bytes.
#[test]
fn appendix_vectors_decode_and_reencode() {
    let checked = corpus::check_candidate(&spec_vectors())
        .unwrap_or_else(|failures| panic!("{}", failures.join("\n")));
    assert_eq!(checked, 57);
}

fn hex(value: &str) -> Vec<u8> {
    corpus::from_hex(&value.replace([' ', '\n'], ""))
}

fn address() -> b::Address {
    [
        "123 Main St".into(),
        "Philadelphia".into(),
        "PA".into(),
        "United States".into(),
    ]
}

/// Appendix B.2 values built with the generated types encode to the published bytes.
#[test]
fn appendix_b_values_encode() {
    let customer = b::Person::Customer(b::Customer {
        name: "James Smith".into(),
        email: "jsmith@example.org".into(),
        address: address(),
        orders: vec![b::CustomerOrders {
            order_id: 4_242_424_242,
            quantity: 5,
        }],
        metadata: BTreeMap::new(),
    });
    let expected = hex(
        "00 0b 4a 61 6d 65 73 20 53 6d 69 74 68 12 6a 73 6d 69 74 68 40 65 78 61 6d 70 6c 65 2e
         6f 72 67 0b 31 32 33 20 4d 61 69 6e 20 53 74 0c 50 68 69 6c 61 64 65 6c 70 68 69 61 02
         50 41 0d 55 6e 69 74 65 64 20 53 74 61 74 65 73 01 b2 41 de fc 00 00 00 00 05 00 00 00
         00",
    );
    assert_eq!(customer.encode_to_vec(), expected);
    assert_eq!(b::Person::decode(Bytes::from(expected)).unwrap(), customer);

    let employee = b::Person::Employee(b::Employee {
        name: "Tiffany Doe".into(),
        email: "tiffanyd@acme.corp".into(),
        address: address(),
        department: b::Department::Administration,
        hire_date: "2020-06-21T21:18:05Z".into(),
        public_key: None,
        metadata: BTreeMap::new(),
    });
    let expected = hex(
        "01 0b 54 69 66 66 61 6e 79 20 44 6f 65 12 74 69 66 66 61 6e 79 64 40 61 63 6d 65 2e 63
         6f 72 70 0b 31 32 33 20 4d 61 69 6e 20 53 74 0c 50 68 69 6c 61 64 65 6c 70 68 69 61 02
         50 41 0d 55 6e 69 74 65 64 20 53 74 61 74 65 73 01 14 32 30 32 30 2d 30 36 2d 32 31 54
         32 31 3a 31 38 3a 30 35 5a 00 00",
    );
    assert_eq!(employee.encode_to_vec(), expected);
    let view = b::Person::view(Bytes::from(expected)).unwrap();
    match view.get() {
        b::PersonRef::Employee(employee) => {
            assert_eq!(employee.name(), "Tiffany Doe");
            assert_eq!(employee.department(), b::Department::Administration);
            assert_eq!(employee.address().get(3), Some("United States"));
        }
        other => panic!("expected an employee, got {other:?}"),
    }

    assert_eq!(b::Person::TerminatedEmployee.encode_to_vec(), [0x02]);
    assert_eq!(b::Department::Jsmith.value(), 99);
}

/// Appendix A's enum and union examples, which use explicit values and tags.
#[test]
fn appendix_a_explicit_values_and_tags() {
    assert_eq!(a::AEnum::Bar.encode_to_vec(), [0xff, 0x01]);
    assert_eq!(a::AEnum::Buzz.encode_to_vec(), [0x80, 0x02]);
    assert_eq!(a::AUnion::Uint(1).encode_to_vec(), [0xff, 0x01, 0x01]);
    assert_eq!(a::AUnion::Int(-255).encode_to_vec(), [0x00, 0xfd, 0x03]);
    assert_eq!(
        a::AUnion::Str("BARE".into()).encode_to_vec(),
        [0x80, 0x02, 0x04, b'B', b'A', b'R', b'E']
    );
}
