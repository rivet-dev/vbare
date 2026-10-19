//! Generated code shape. The generated code is compiled and exercised by `rust/examples/basic`
//! and `rust/conformance`.

use std::{fs, path::Path};
use vbare_gen::{Config, SchemaLoader};

fn generate(schema: &str) -> Result<String, String> {
    generate_with(schema, Config::default())
}

fn generate_with(schema: &str, config: Config) -> Result<String, String> {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("schema.v1.bare"), schema).unwrap();
    let tokens = SchemaLoader::new(dir.path())
        .generate(Path::new("schema.v1.bare"), config)
        .map_err(|error| error.to_string())?;
    let file: syn::File = syn::parse2(tokens).map_err(|error| error.to_string())?;
    Ok(prettyplease::unparse(&file))
}

#[test]
fn generates_four_items_per_type() {
    let code = generate(
        "type Point struct { x: i32 y: i32 }\n\
         type Shape union { Point | str }\n\
         type Kind enum { A B = 5 C }\n\
         type Points list<Point>",
    )
    .unwrap();
    for item in [
        "pub struct Point {",
        "pub struct PointRef<'a> {",
        "pub struct PointWire;",
        "pub type PointView = ::vbare::View<PointWire>;",
        "pub enum Shape {",
        "pub enum ShapeRef<'a> {",
        "pub enum Kind {",
        "pub type KindRef<'a> = Kind;",
        "pub type Points = ::std::vec::Vec<Point>;",
        "pub type PointsRef<'a> = ::vbare::ListRef<'a, PointWire>;",
        "pub type PointsWire = ::vbare::wire::List<PointWire>;",
    ] {
        assert!(code.contains(item), "missing {item:?} in:\n{code}");
    }
    // Explicit enum values are honored and automatic values continue from them.
    assert!(code.contains("Kind::B => 5"), "{code}");
    assert!(code.contains("Kind::C => 6"), "{code}");
}

#[test]
fn maps_bare_types_to_owned_types() {
    let code = generate(
        "type T struct {\n\
           a: uint\n b: int\n c: str\n d: data\n e: data[32]\n f: data[33]\n\
           g: list<u8>[3]\n h: map<str><u8>\n i: optional<f64>\n\
         }",
    )
    .unwrap();
    for field in [
        "pub a: u64,",
        "pub b: i64,",
        "pub c: ::vbare::Str,",
        "pub d: ::vbare::Bytes,",
        "pub e: [u8; 32],",
        "pub f: ::vbare::FixedBytes<33>,",
        "pub g: [u8; 3],",
        "pub h: ::std::collections::BTreeMap<::vbare::Str, u8>,",
        "pub i: ::std::option::Option<f64>,",
    ] {
        assert!(code.contains(field), "missing {field:?} in:\n{code}");
    }
    // Floats rule out Eq and Hash.
    assert!(!code.contains("Eq, Hash"), "{code}");
}

#[test]
fn hoists_anonymous_aggregates() {
    let code = generate(
        "type Customer struct {\n\
           orders: list<struct { orderId: i64 }>\n\
           status: enum { ON OFF }\n\
         }\n\
         type Event union { struct { at: u64 } | void | list<u8> }\n\
         type Tree optional<struct { label: str }>",
    )
    .unwrap();
    for item in [
        "pub struct CustomerOrders {",
        "pub order_id: i64,",
        "pub enum CustomerStatus {",
        "pub struct EventMember0 {",
        "Member0(EventMember0)",
        "Void,",
        "Member2(::std::vec::Vec<u8>)",
        "pub struct TreeValue {",
        "pub type Tree = ::std::option::Option<TreeValue>;",
    ] {
        assert!(code.contains(item), "missing {item:?} in:\n{code}");
    }
}

#[test]
fn unions_without_borrowed_members_are_their_own_ref() {
    let code = generate("type Small union { u8 | bool | void }").unwrap();
    assert!(code.contains("pub type SmallRef<'a> = Small;"), "{code}");
    assert!(
        code.contains("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]"),
        "{code}"
    );
}

#[test]
fn escapes_keyword_field_names() {
    let code = generate("type T struct { type: u8 match: str }").unwrap();
    assert!(code.contains("pub r#type: u8,"), "{code}");
    assert!(code.contains("pub fn r#match(&self)"), "{code}");
}

#[test]
fn rejects_name_collisions() {
    let error = generate("type Foo u8\ntype FooRef u16").unwrap_err();
    assert!(error.contains("FooRef collides"), "{error}");

    let error = generate("type T struct { fooBar: u8 foo_bar: u8 }").unwrap_err();
    assert!(error.contains("same Rust name foo_bar"), "{error}");

    let error = generate("type T struct { toOwned: u8 }").unwrap_err();
    assert!(
        error.contains("conflicts with the generated to_owned method"),
        "{error}"
    );

    let error = generate("type E enum { FOO_BAR FooBar }").unwrap_err();
    assert!(error.contains("same Rust variant FooBar"), "{error}");

    let error = generate("type T struct { a: struct { b: u8 } }\ntype TA u8").unwrap_err();
    assert!(error.contains("TA"), "{error}");
}

#[test]
fn serde_derives_are_opt_in() {
    let schema = "type P struct { x: i32 name: str }\n\
                  type U union { P | str }\n\
                  type K enum { A B }";

    let code = generate(schema).unwrap();
    assert!(!code.contains("serde"), "{code}");

    // One derive each on the struct, the union and the enum, and none on the borrowed forms.
    let code = generate_with(schema, Config::default().with_serde(true)).unwrap();
    assert_eq!(code.matches("::serde::Serialize").count(), 3, "{code}");
    assert_eq!(code.matches("::serde::Deserialize").count(), 3, "{code}");
    assert!(
        code.contains(
            "#[derive(Clone, Debug, PartialEq, Eq, Hash, ::serde::Serialize, ::serde::Deserialize)]\npub struct P {"
        ),
        "{code}"
    );
}

#[test]
fn serde_rejects_constructs_serde_bare_writes_differently() {
    let serde = Config::default().with_serde(true);
    for (schema, expected) in [
        ("type T struct { a: uint }", "T.a: uint and int"),
        ("type T struct { a: optional<list<int>> }", "T.a: uint and int"),
        ("type T union { u8 | uint }", "T member 1: uint and int"),
        ("type T map<str><uint>", "T: uint and int"),
        ("type T struct { a: struct { b: int } }", "TA.b: uint and int"),
        ("type E enum { A B = 5 }", "enum value B = 5 differs from its index 1"),
        ("type U union { u8 | str = 3 }", "union member 1 has tag 3"),
        ("type T list<u8>[33]", "fixed-length lists longer than 32"),
    ] {
        let error = generate_with(schema, serde).unwrap_err();
        assert!(error.contains(expected), "{schema}: {error}");
        generate(schema).unwrap();
    }

    // Explicit values equal to the index, fixed lists up to 32 and long fixed data are accepted.
    generate_with(
        "type E enum { A = 0 B = 1 C }\n\
         type U union { u8 = 0 | str }\n\
         type T struct { a: list<u8>[32] b: data[64] }",
        serde,
    )
    .unwrap();
}
