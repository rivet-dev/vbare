//! Conformance suite for the vbare zero-copy codec.
//!
//! `build/` generates, from the same schemas:
//!
//! - the candidate: `vbare-gen` output (`cand`);
//! - the reference: `bare_proc` 0.2.0 types encoded by `serde_bare` 0.5.0, with adapters that
//!   convert to and from candidate types (`reference`);
//! - schema descriptors (`desc`) and proptest strategies (`strat`);
//! - one [`harness::Entry`] per type in [`ENTRIES`];
//! - schema copies for `bare-ts` under `OUT_DIR/bare-ts`.
//!
//! Schema sets: `reference` and `basic` are compared with the reference implementation;
//! `candidate` and `spec` use constructs the reference cannot express and are checked against
//! bare-ts only; `rust` is checked by the candidate alone. Sets named `extra_*` come from the
//! `VBARE_EXTRA_SCHEMAS` directory at build time.
//!
//! Running:
//!
//! - `cargo test -p vbare-conformance`: everything except the extra schemas. Set `CI=1` or
//!   `CONFORMANCE_CASES=<n>` for more property cases per type (CI uses 10,000). Test 4 needs
//!   Node.js and installs the pinned bare-ts packages in `bare-ts/` on first use.
//! - `VBARE_UPDATE_CORPUS=1 cargo test -p vbare-conformance --test bare_ts`: regenerates
//!   `vectors/corpus.json`.
//! - `VBARE_EXTRA_SCHEMAS=<dir> cargo test -p vbare-conformance extra_schemas -- --ignored`:
//!   tests 1 to 4 over another schema tree, without checking anything from it in.
//! - `cargo +nightly fuzz run <reference|basic|candidate>` in `fuzz/` (test 7). Seed the corpora
//!   first with `cargo test -p vbare-conformance --test differential write_fuzz_seeds -- --ignored`.

pub mod corpus;
pub mod desc;
pub mod harness;

#[allow(
    non_snake_case,
    non_upper_case_globals,
    clippy::all,
    unused_imports,
    dead_code
)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/generated.rs"));
}

pub use generated::{cand, cases, ENTRIES};

/// Directory holding the bare-ts schema copies, one subdirectory per set.
pub const BARE_TS_SCHEMAS: &str = concat!(env!("OUT_DIR"), "/bare-ts");

/// Entries of one schema set.
pub fn entries(set: &str) -> impl Iterator<Item = &'static harness::Entry> + '_ {
    ENTRIES.iter().filter(move |entry| entry.id.set == set)
}

/// Whether a set came from `VBARE_EXTRA_SCHEMAS`.
pub fn is_extra_set(set: &str) -> bool {
    set.starts_with("extra")
}

/// Entry point for the `cargo fuzz` targets in `fuzz/`.
///
/// The first input byte selects a type among the entries of `sets`; the rest is decoded. Types
/// with a reference implementation get the differential check, the others the candidate-only
/// checks. Any failure panics, which the fuzzer reports.
pub fn fuzz(sets: &[&str], data: &[u8]) {
    let selected: Vec<&harness::Entry> = ENTRIES
        .iter()
        .filter(|entry| sets.contains(&entry.id.set))
        .collect();
    let Some((&selector, input)) = data.split_first() else {
        return;
    };
    let Some(entry) = selected.get(usize::from(selector) % selected.len().max(1)) else {
        return;
    };
    let result = match &entry.reference {
        Some(reference) => (reference.differential)(input),
        None => (entry.candidate_fuzz)(input),
    };
    if let Err(error) = result {
        panic!("{}: {error}", entry.id);
    }
}

/// Seed inputs for [`fuzz`]: a few sampled values of every type in `sets`, each prefixed with
/// its selector byte.
pub fn fuzz_seeds(sets: &[&str]) -> Vec<Vec<u8>> {
    let selected: Vec<&harness::Entry> = ENTRIES
        .iter()
        .filter(|entry| sets.contains(&entry.id.set))
        .collect();
    let mut runner = harness::deterministic_runner();
    let mut seeds = Vec::new();
    for (index, entry) in selected.iter().enumerate() {
        let Ok(selector) = u8::try_from(index) else {
            break;
        };
        for _ in 0..4 {
            let mut seed = vec![selector];
            seed.extend((entry.sample)(&mut runner));
            seeds.push(seed);
        }
    }
    seeds
}
