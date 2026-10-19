use std::{fs, path::Path};

/// Compiles the example's three schema versions, which use the current grammar.
#[test]
fn processes_example_schemas() {
    let schema_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../examples/basic/schemas");
    let out_dir = tempfile::tempdir().expect("create output dir");

    vbare_compiler::process_schemas_to_dir(&schema_dir, out_dir.path(), &Default::default())
        .expect("process schemas");

    for file in [
        "v1_generated.rs",
        "v2_generated.rs",
        "v3_generated.rs",
        "combined_imports.rs",
    ] {
        let path = out_dir.path().join(file);
        let metadata = fs::metadata(&path)
            .unwrap_or_else(|error| panic!("missing generated file {}: {error}", path.display()));
        assert!(
            metadata.len() > 0,
            "empty generated file: {}",
            path.display()
        );
    }

    let combined = fs::read_to_string(out_dir.path().join("combined_imports.rs")).unwrap();
    assert!(combined.contains(
        "pub mod v1 {\n    include!(concat!(env!(\"OUT_DIR\"), \"/v1_generated.rs\"));\n}\n"
    ));
}
