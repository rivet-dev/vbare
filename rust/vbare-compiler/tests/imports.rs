use std::{
    fs,
    path::{Path, PathBuf},
};

fn fixtures(case: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/tests/imports")
        .join(case)
}

#[test]
fn generates_shared_types_once_and_imports_transitive_modules() {
    let output = tempfile::tempdir().unwrap();
    vbare_compiler::process_schemas_to_dir(&fixtures("valid"), output.path(), &Default::default())
        .unwrap();
    let modules = fs::read_to_string(output.path().join("combined_imports.rs")).unwrap();
    assert_eq!(modules.matches("pub mod shared_common_v1").count(), 1);
    assert!(modules.contains("pub mod shared_common_v7"));
    assert!(modules.contains("pub mod nested_transitive_v3"));
    let code = fs::read_to_string(output.path().join("nested_transitive.v3_generated.rs")).unwrap();
    assert!(code.contains("use super::shared_common_v1::*;"));
    assert!(code.contains("use super::shared_middle_v2::*;"));
    assert!(!code.contains("pub struct SharedKey"));
    assert!(code.contains("pub key: Option<SharedKey>"));
    assert!(code.contains("pub keys: Vec<SharedKey>"));
    assert!(
        !code.contains("Eq, Hash"),
        "floating imported types must prevent Eq/Hash derives"
    );
    let pinned = fs::read_to_string(output.path().join("pinned.v1_generated.rs")).unwrap();
    assert!(pinned.contains("use super::shared_common_v7::*;"));
    assert!(!pinned.contains("shared_common_v1"));
}

#[test]
fn reports_invalid_imports_with_file_context() {
    for (case, expected) in [
        ("cycle", vec!["cycle", "a.v1.bare", "b.v1.bare"]),
        ("missing", vec!["a.v1.bare", "shared/missing.v1.bare"]),
        ("conflict", vec!["Shared", "b.v1.bare", "c.v1.bare"]),
        ("shadow", vec!["Shared", "a.v1.bare", "b.v1.bare"]),
        ("late", vec!["a.v1.bare"]),
        ("malformed", vec!["a.v1.bare"]),
        ("escape", vec!["a.v1.bare", "root-relative"]),
        (
            "module-conflict",
            vec!["shared/a.v1.bare", "shared_a.v1.bare", "same module"],
        ),
    ] {
        let output = tempfile::tempdir().unwrap();
        let error = vbare_compiler::process_schemas_to_dir(
            &fixtures(case),
            output.path(),
            &Default::default(),
        )
        .expect_err(case)
        .to_string();
        for expected in expected {
            assert!(
                error.contains(expected),
                "{case}: expected {expected:?} in {error:?}"
            );
        }
    }
}
