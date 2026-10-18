//! Differential decoding of the `reference` schema set against `bare_proc` and `serde_bare`.
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    vbare_conformance::fuzz(&["reference"], data);
});
