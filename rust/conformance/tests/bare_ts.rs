//! Test 4: cross-check the candidate and bare-ts on a shared vector corpus.

use std::{
    path::{Path, PathBuf},
    process::Command,
};
use vbare_conformance::{corpus, is_extra_set, BARE_TS_SCHEMAS, ENTRIES};

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn corpus_path() -> PathBuf {
    manifest_dir().join("vectors/corpus.json")
}

fn spec_path() -> PathBuf {
    manifest_dir().join("vectors/spec.json")
}

fn read_json(path: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The checked-in corpus agrees with the candidate. `VBARE_UPDATE_CORPUS=1` regenerates it.
#[test]
fn corpus_matches_candidate() {
    if std::env::var_os("VBARE_UPDATE_CORPUS").is_some() {
        let corpus = corpus::generate(corpus::CORPUS_SETS);
        std::fs::write(corpus_path(), corpus::to_text(&corpus)).expect("write corpus");
    }
    for path in [corpus_path(), spec_path()] {
        match corpus::check_candidate(&read_json(&path)) {
            Ok(checked) => eprintln!("{}: candidate agrees on {checked} vectors", path.display()),
            Err(failures) => panic!("{}:\n{}", path.display(), failures.join("\n")),
        }
    }
}

/// Installs the pinned bare-ts packages if needed and runs `check.mjs` over `corpora`.
fn run_bare_ts(corpora: &[PathBuf]) {
    let dir = manifest_dir().join("bare-ts");
    if Command::new("node").arg("--version").output().is_err() {
        panic!("test 4 needs Node.js on PATH to run bare-ts (rust/conformance/bare-ts/check.mjs)");
    }
    if !dir.join("node_modules/@bare-ts/tools").exists() {
        // `--ignore-workspace` keeps pnpm from installing an enclosing workspace instead when this
        // crate is vendored inside another pnpm repository.
        let installed = Command::new("pnpm")
            .args(["install", "--frozen-lockfile", "--ignore-workspace"])
            .current_dir(&dir)
            .status()
            .is_ok_and(|status| status.success());
        assert!(
            installed,
            "could not install bare-ts; run `pnpm install --frozen-lockfile --ignore-workspace` in {}",
            dir.display()
        );
    }
    let output = Command::new("node")
        .arg(dir.join("check.mjs"))
        .arg(BARE_TS_SCHEMAS)
        .args(corpora)
        .output()
        .expect("run node");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "bare-ts check failed:\n{stdout}\n{stderr}"
    );
    eprintln!("{stdout}");
}

#[test]
fn bare_ts_cross_check() {
    run_bare_ts(&[corpus_path(), spec_path()]);
}

/// Test 4 over schemas from `VBARE_EXTRA_SCHEMAS`, set when building.
#[test]
#[ignore = "needs VBARE_EXTRA_SCHEMAS at build time"]
fn extra_schemas() {
    let mut sets: Vec<&str> = ENTRIES
        .iter()
        .map(|entry| entry.id.set)
        .filter(|set| is_extra_set(set))
        .collect();
    sets.dedup();
    assert!(
        !sets.is_empty(),
        "no extra schemas were built; rebuild with VBARE_EXTRA_SCHEMAS=<dir>"
    );
    let corpus = corpus::generate(&sets);
    let checked = corpus::check_candidate(&corpus)
        .unwrap_or_else(|failures| panic!("{}", failures.join("\n")));
    let path = std::env::temp_dir().join(format!("vbare-extra-corpus-{}.json", std::process::id()));
    std::fs::write(
        &path,
        serde_json::to_string(&corpus).expect("serializable corpus"),
    )
    .expect("write extra corpus");
    eprintln!("extra corpus: candidate agrees on {checked} vectors");
    run_bare_ts(std::slice::from_ref(&path));
    std::fs::remove_file(path).ok();
}
