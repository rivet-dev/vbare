//! Test 7, short form: the differential decode check from `fuzz/` driven by proptest, over
//! random bytes and mutations of valid messages.

use proptest::{
    prelude::*,
    test_runner::{Config, TestRunner},
};
use vbare_conformance::{
    harness::{cases, Entry},
    is_extra_set, ENTRIES,
};

/// Applies one random edit: flip a byte, insert a byte, delete a byte or truncate.
fn mutate(mut bytes: Vec<u8>, kind: u8, position: usize, byte: u8) -> Vec<u8> {
    let index = if bytes.is_empty() {
        0
    } else {
        position % bytes.len()
    };
    match kind % 4 {
        0 if !bytes.is_empty() => bytes[index] = byte,
        1 => bytes.insert(index.min(bytes.len()), byte),
        2 if !bytes.is_empty() => {
            bytes.remove(index);
        }
        _ => bytes.truncate(index),
    }
    bytes
}

fn check(entry: &Entry, bytes: &[u8]) -> Result<(), String> {
    match &entry.reference {
        Some(reference) => (reference.differential)(bytes),
        None => (entry.candidate_fuzz)(bytes),
    }
}

#[test]
fn differential_decoding() {
    let cases = (cases() / 4).max(64);
    let mut failures = Vec::new();
    for entry in ENTRIES.iter().filter(|entry| !is_extra_set(entry.id.set)) {
        let mut runner = TestRunner::new(Config {
            cases,
            failure_persistence: None,
            ..Config::default()
        });
        let samples = (entry.sample)(&mut runner);
        let strategy = prop_oneof![
            proptest::collection::vec(any::<u8>(), 0..64),
            (any::<u8>(), any::<usize>(), any::<u8>()).prop_map(move |(kind, position, byte)| {
                mutate(samples.clone(), kind, position, byte)
            }),
        ];
        let result = runner.run(&strategy, |bytes| {
            check(entry, &bytes).map_err(proptest::test_runner::TestCaseError::fail)
        });
        if let Err(error) = result {
            failures.push(format!("{}: {error}", entry.id));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Writes seed inputs for the `cargo fuzz` targets into `fuzz/corpus/<target>/`.
#[test]
#[ignore = "run before fuzzing to seed the corpora"]
fn write_fuzz_seeds() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("fuzz/corpus");
    for (target, sets) in [
        ("reference", &["reference"][..]),
        ("basic", &["basic"][..]),
        ("candidate", &["candidate", "spec", "rust"][..]),
    ] {
        let dir = root.join(target);
        std::fs::create_dir_all(&dir).unwrap();
        for (index, seed) in vbare_conformance::fuzz_seeds(sets).into_iter().enumerate() {
            std::fs::write(dir.join(format!("seed-{index}")), seed).unwrap();
        }
    }
}
