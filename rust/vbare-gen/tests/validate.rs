//! Schema invariants from draft-devault-bare-11 section 2.4.

use vbare_gen::parse_schema;

fn rejects(source: &str, expected: &str) {
    let error = parse_schema(source).expect_err(source);
    assert!(
        error.contains(expected),
        "{source:?}: expected {expected:?} in {error:?}"
    );
}

#[test]
fn accepts_valid_schemas() {
    for source in [
        "type Empty void\ntype U union { Empty | u8 | void }",
        "type E enum { A B }\ntype M map<E><str>",
        "type K uint\ntype M map<K><list<str>>",
        "type M map<bool><map<i8><u8>>",
        "type D data[1]\ntype L list<D>[1]",
        "type A u8\ntype B A\ntype C B",
    ] {
        parse_schema(source).unwrap_or_else(|error| panic!("{source:?}: {error}"));
    }
}

#[test]
fn rejects_void_outside_unions() {
    rejects(
        "type S struct { a: void }",
        "void may only be a union member",
    );
    rejects("type V void\ntype S struct { a: V }", "not a struct field");
    rejects(
        "type V void\ntype W V\ntype L list<W>",
        "not a list element",
    );
    rejects("type O optional<void>", "not an optional type");
    rejects("type M map<str><void>", "not a map value");
    rejects("type U union { u8 | list<void> }", "not a list element");
}

#[test]
fn rejects_duplicates() {
    rejects("type E enum { A A }", "duplicate enum value name A");
    rejects("type E enum { A = 1 B = 1 }", "duplicate enum value 1");
    rejects("type E enum { A = 1 B C = 2 }", "duplicate enum value 2");
    rejects("type U union { u8 | str | u8 }", "duplicate union member");
    rejects(
        "type A u8\ntype U union { A | A }",
        "duplicate union member",
    );
    rejects("type U union { u8 = 1 | str = 1 }", "duplicate union tag 1");
    rejects(
        "type U union { u8 = 2 | str = 1 | bool }",
        "duplicate union tag 2",
    );
    rejects("type S struct { a: u8 a: str }", "duplicate struct field a");
}

#[test]
fn rejects_zero_lengths() {
    rejects("type D data[0]", "fixed-length data");
    rejects("type L list<u8>[0]", "fixed-length list");
    rejects("type S struct { a: list<data[0]>[2] }", "fixed-length data");
}

#[test]
fn rejects_illegal_map_keys() {
    for key in [
        "f32",
        "f64",
        "data",
        "data[4]",
        "void",
        "list<u8>",
        "optional<u8>",
        "struct { a: u8 }",
        "union { u8 }",
    ] {
        rejects(&format!("type M map<{key}><u8>"), "map key");
    }
    rejects("type F f64\ntype M map<F><u8>", "map key");
    rejects("type D data[16]\ntype K D\ntype M map<K><u8>", "map key");
}

#[test]
fn reports_the_offending_type() {
    rejects(
        "type A u8\ntype S struct { ok: A bad: list<u8>[0] }",
        "type S: field bad: fixed-length list",
    );
}
