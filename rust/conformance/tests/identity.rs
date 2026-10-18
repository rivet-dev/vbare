//! Tests 1 to 3: encode identity, decode identity and verbatim view encoding, for every type of
//! every schema set.

use vbare_conformance::{
    entries,
    harness::{cases, Entry},
    is_extra_set, ENTRIES,
};

/// Sets compared with the reference implementation by default. Extra sets run in
/// `extra_schemas`.
const REFERENCE_SETS: &[&str] = &["reference", "basic"];

/// Runs `check` for every entry in parallel and reports every failure.
fn run_all(
    selected: Vec<&'static Entry>,
    check: impl Fn(&'static Entry) -> Option<Result<(), String>> + Sync,
) -> usize {
    assert!(!selected.is_empty(), "no entries selected");
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = selected.len().div_ceil(threads);
    let (checked, failures): (usize, Vec<String>) = std::thread::scope(|scope| {
        let handles: Vec<_> = selected
            .chunks(chunk)
            .map(|entries| {
                let check = &check;
                scope.spawn(move || {
                    let mut checked = 0;
                    let mut failures = Vec::new();
                    for entry in entries {
                        if let Some(result) = check(entry) {
                            checked += 1;
                            if let Err(error) = result {
                                failures.push(format!("{}: {error}", entry.id));
                            }
                        }
                    }
                    (checked, failures)
                })
            })
            .collect();
        handles
            .into_iter()
            .fold((0, Vec::new()), |(count, mut all), handle| {
                let (checked, failures) = handle.join().expect("checker thread panicked");
                all.extend(failures);
                (count + checked, all)
            })
    });
    assert!(
        failures.is_empty(),
        "{} of {checked} types failed:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
    checked
}

fn non_extra() -> Vec<&'static Entry> {
    ENTRIES
        .iter()
        .filter(|entry| !is_extra_set(entry.id.set))
        .collect()
}

fn reference_sets() -> Vec<&'static Entry> {
    REFERENCE_SETS.iter().flat_map(|set| entries(set)).collect()
}

/// Candidate round trips for every type: encode, view, verbatim re-encode, decode, `to_owned`,
/// prefix truncation and trailing bytes.
#[test]
fn candidate_round_trips() {
    let cases = cases();
    let checked = run_all(non_extra(), |entry| Some((entry.roundtrip)(cases)));
    eprintln!("candidate round trips: {checked} types, {cases} cases each");
}

/// Test 1: candidate encoding equals `serde_bare::to_vec` of the reference type.
#[test]
fn encode_identity() {
    let cases = cases();
    let checked = run_all(reference_sets(), |entry| {
        entry
            .reference
            .as_ref()
            .map(|reference| (reference.encode_identity)(cases))
    });
    eprintln!("encode identity: {checked} types, {cases} cases each");
}

/// Tests 2 and 3: reference bytes decode in the candidate through `view`, `to_owned` and
/// `decode`, views re-encode verbatim, and candidate bytes decode in the reference.
#[test]
fn decode_identity() {
    let cases = cases();
    let checked = run_all(reference_sets(), |entry| {
        entry
            .reference
            .as_ref()
            .map(|reference| (reference.decode_identity)(cases))
    });
    eprintln!("decode identity: {checked} types, {cases} cases each");
}

/// Tests 1 to 3 over schemas from `VBARE_EXTRA_SCHEMAS`, set when building. Test 4 for these
/// schemas runs in `bare_ts::extra_schemas`.
#[test]
#[ignore = "needs VBARE_EXTRA_SCHEMAS at build time"]
fn extra_schemas() {
    let extra: Vec<&'static Entry> = ENTRIES
        .iter()
        .filter(|entry| is_extra_set(entry.id.set))
        .collect();
    assert!(
        !extra.is_empty(),
        "no extra schemas were built; rebuild with VBARE_EXTRA_SCHEMAS=<dir>"
    );
    let cases = cases();
    let roundtrips = run_all(extra.clone(), |entry| Some((entry.roundtrip)(cases)));
    let encode = run_all(extra.clone(), |entry| {
        entry
            .reference
            .as_ref()
            .map(|reference| (reference.encode_identity)(cases))
    });
    let decode = run_all(extra.clone(), |entry| {
        entry
            .reference
            .as_ref()
            .map(|reference| (reference.decode_identity)(cases))
    });
    let bug_cases: Vec<String> = extra
        .iter()
        .filter(|entry| entry.desc.has_reference_bug())
        .map(|entry| entry.id.to_string())
        .collect();
    assert!(
        bug_cases.is_empty(),
        "extra schemas needed reference byte transforms for: {bug_cases:?}"
    );
    eprintln!(
        "extra schemas: {roundtrips} round trips, {encode} encode identity, {decode} decode identity, {cases} cases each"
    );
}
