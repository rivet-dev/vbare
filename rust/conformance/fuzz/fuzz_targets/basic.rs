//! Differential decoding of the `basic` example schemas against `bare_proc` and `serde_bare`.
#![no_main]

libfuzzer_sys::fuzz_target!(|data: &[u8]| {
    vbare_conformance::fuzz(&["basic"], data);
});
