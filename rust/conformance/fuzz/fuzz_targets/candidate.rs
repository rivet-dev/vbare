//! Candidate-only checks for schemas the reference cannot express.
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    vbare_conformance::fuzz(&["candidate", "spec", "rust"], data);
});
