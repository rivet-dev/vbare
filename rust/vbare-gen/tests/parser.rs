//! Parser behavior that the vendored grammar patch adds on top of `bare_proc` 0.2.0.

use vbare_gen::{
    ast::{AnyType, PrimitiveType},
    parse_schema,
};

/// The example schema from draft-devault-bare-11 Appendix B.1, verbatim.
const APPENDIX_B: &str = "type PublicKey data[128]
type Time str # ISO 8601

type Department enum {
  ACCOUNTING
  ADMINISTRATION
  CUSTOMER_SERVICE
  DEVELOPMENT

  # Reserved for the CEO
  JSMITH = 99
}

type Address list<str>[4] # street, city, state, country

type Customer struct {
  name: str
  email: str
  address: Address
  orders: list<struct {
    orderId: i64
    quantity: i32
  }>
  metadata: map<str><data>
}

type Employee struct {
  name: str
  email: str
  address: Address
  department: Department
  hireDate: Time
  publicKey: optional<PublicKey>
  metadata: map<str><data>
}

type TerminatedEmployee void

type Person union {Customer | Employee | TerminatedEmployee}
";

fn enum_values(source: &str, name: &str) -> Vec<(String, u64)> {
    match parse_schema(source).unwrap().remove(name) {
        Some(AnyType::Enum(values)) => values,
        other => panic!("expected enum {name}, got {other:?}"),
    }
}

fn union_tags(source: &str, name: &str) -> Vec<u64> {
    match parse_schema(source).unwrap().remove(name) {
        Some(AnyType::Union(members)) => members.into_iter().map(|(_, tag)| tag).collect(),
        other => panic!("expected union {name}, got {other:?}"),
    }
}

#[test]
fn parses_appendix_b() {
    let schema = parse_schema(APPENDIX_B).unwrap();
    assert_eq!(schema.len(), 8);
    assert!(matches!(
        schema["PublicKey"],
        AnyType::Primitive(PrimitiveType::Data(Some(128)))
    ));
    assert!(matches!(
        schema["Address"],
        AnyType::List {
            length: Some(4),
            ..
        }
    ));
    let values: Vec<u64> = enum_values(APPENDIX_B, "Department")
        .into_iter()
        .map(|(_, value)| value)
        .collect();
    assert_eq!(values, [0, 1, 2, 3, 99]);
    assert_eq!(union_tags(APPENDIX_B, "Person"), [0, 1, 2]);
}

#[test]
fn integers_tolerate_surrounding_whitespace() {
    for source in [
        "type E enum { A B = 5 }",
        "type E enum { A B = 5\n}",
        "type E enum { A B =5 C }",
        "type E enum {\n\tA\n\tB = 5 # comment\n\tC\n}\n",
    ] {
        let values = enum_values(source, "E");
        assert_eq!(values[1], ("B".to_owned(), 5), "{source:?}");
    }
    assert!(matches!(
        parse_schema("type D data[ 16 ]").unwrap()["D"],
        AnyType::Primitive(PrimitiveType::Data(Some(16)))
    ));
}

#[test]
fn enum_values_continue_from_explicit_values() {
    let values = enum_values("type E enum { A = 0 B C = 10 D E = 3 F }", "E");
    let values: Vec<u64> = values.into_iter().map(|(_, value)| value).collect();
    assert_eq!(values, [0, 1, 10, 11, 3, 4]);
}

#[test]
fn union_tags_continue_from_explicit_tags() {
    assert_eq!(union_tags("type U union { u8 = 1 | str }", "U"), [1, 2]);
    assert_eq!(
        union_tags("type U union { int | uint = 255 | str }", "U"),
        [0, 255, 256]
    );
    assert_eq!(
        union_tags("type U union {\n  | u8\n  | str = 7\n  | bool\n}", "U"),
        [0, 7, 8]
    );
    assert_eq!(union_tags("type U union { u8 | str | }", "U"), [0, 1]);
}

#[test]
fn accepts_zero_lengths_for_the_validator_to_reject() {
    assert!(matches!(
        parse_schema("type D data[0]").unwrap()["D"],
        AnyType::Primitive(PrimitiveType::Data(Some(0)))
    ));
}

#[test]
fn reports_out_of_range_integers() {
    let error = parse_schema("type E enum { A = 18446744073709551616 }").unwrap_err();
    assert!(error.starts_with("1:19:"), "{error}");
    assert!(error.contains("out of range"), "{error}");

    let error = parse_schema("type E enum { A = 18446744073709551615 B }").unwrap_err();
    assert!(error.contains("enum value B is out of range"), "{error}");

    let error = parse_schema("type U union { u8 = 18446744073709551615 | str }").unwrap_err();
    assert!(error.contains("union tag is out of range"), "{error}");

    let error = parse_schema("type D data[99999999999999999999999]").unwrap_err();
    assert!(error.contains("out of range"), "{error}");
}

#[test]
fn reports_errors_with_line_and_column() {
    let error = parse_schema("type A u8\ntype B struct {\n  a: C\n}").unwrap_err();
    assert_eq!(error, "3:6: user type C has not been defined yet");

    let error = parse_schema("type A u8\ntype A u16").unwrap_err();
    assert_eq!(error, "2:6: duplicate or shadowed type A");

    let error = parse_schema("type A u8\ntype B strukt { a: u8 }").unwrap_err();
    assert_eq!(error, "2:11: expected EOI or user_type");
}
