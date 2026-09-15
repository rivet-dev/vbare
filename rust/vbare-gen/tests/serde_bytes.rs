//! Verifies that `data` fields mapping to `Vec<u8>` are annotated with `serde_bytes`, so serde_bare
//! encodes them in bulk instead of one element at a time.

use std::{
    env, fs,
    path::PathBuf,
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

const SCHEMA: &str = r#"
type Blob data
type SmallKey data[16]
type BigKey data[64]

type Empty void
type Label str

type Value union {
  Empty |
  Label |
  Blob
}

type Message struct {
  body: data
  maybeBody: optional<data>
  alias: Blob
  small: SmallKey
  big: BigKey
  chunks: list<data>
  name: str
  value: Value
}
"#;

/// Tests run in parallel, so each generation gets its own schema file.
static SCHEMA_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Generated code is compared with whitespace removed, because `TokenStream::to_string` spaces
/// tokens out in ways that are irrelevant to what is being asserted.
fn generate_compact() -> String {
    let path = temp_schema_path();
    fs::write(&path, SCHEMA).expect("write schema");

    let tokens = vbare_gen::bare_schema(&path, vbare_gen::Config::default());

    fs::remove_file(&path).expect("remove schema");

    tokens
        .to_string()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

fn temp_schema_path() -> PathBuf {
    let seq = SCHEMA_SEQ.fetch_add(1, Ordering::Relaxed);
    env::temp_dir().join(format!("vbare_serde_bytes_{}_{seq}.bare", process::id()))
}

#[test]
fn annotates_variable_length_data_fields() {
    let generated = generate_compact();

    // `alias` and `big` resolve to `data` through a type alias, `maybe_body` through `optional`.
    for field in ["body", "maybe_body", "alias", "big"] {
        assert!(
            generated.contains(&format!("#[serde(with=\"serde_bytes\")]pub{field}:")),
            "expected `{field}` to be annotated with serde_bytes, got: {generated}"
        );
    }
}

#[test]
fn leaves_other_fields_unannotated() {
    let generated = generate_compact();

    // Arrays and nested data are not supported by serde_bytes, and `str` is unrelated.
    for field in ["small", "chunks", "name"] {
        assert!(
            !generated.contains(&format!("#[serde(with=\"serde_bytes\")]pub{field}:")),
            "expected `{field}` to be left unannotated, got: {generated}"
        );
    }
}

#[test]
fn annotates_union_variants_carrying_data() {
    let generated = generate_compact();

    // A union member that resolves to `data` becomes a newtype variant holding `Vec<u8>`.
    assert!(
        generated.contains("#[serde(with=\"serde_bytes\")]Blob(Blob)"),
        "expected the Blob variant to be annotated, got: {generated}"
    );
    assert!(
        !generated.contains("#[serde(with=\"serde_bytes\")]Label(Label)"),
        "expected the Label variant to be left unannotated, got: {generated}"
    );
}

#[test]
fn keeps_underlying_field_types() {
    let generated = generate_compact();

    // The annotation must not change the generated Rust types, only how serde encodes them.
    assert!(generated.contains("pubbody:Vec<u8>"), "got: {generated}");
    assert!(
        generated.contains("pubmaybe_body:Option<Vec<u8>>"),
        "got: {generated}"
    );
    assert!(generated.contains("pubchunks:Vec<Vec<u8>>"), "got: {generated}");
    assert!(
        generated.contains("pubtypeSmallKey=[u8;16"),
        "got: {generated}"
    );
    assert!(generated.contains("pubtypeBigKey=Vec<u8>"), "got: {generated}");
    assert!(generated.contains("pubtypeBlob=Vec<u8>"), "got: {generated}");
}
