//! Test 6: each documented divergence between the candidate and the reference implementation
//! (`bare_proc` 0.2.0 with `serde_bare` 0.5.0), asserting both sides so a change in either is
//! caught.

use vbare::{Bytes, DecodeErrorKind, Encode};
use vbare_conformance::{
    cand::reference::all_v1 as cand,
    cases::reference::all_v1 as cases,
    desc::reference_to_spec,
    harness::{self, Case, ReferenceCase},
};

fn reference_encode<C: ReferenceCase>(value: &C::Owned) -> Vec<u8> {
    C::ref_to_vec(&C::to_ref(value))
}

fn reference_decode<C: ReferenceCase>(bytes: &[u8]) -> C::Owned {
    C::from_ref(C::ref_from_slice(bytes).expect("reference accepts the input"))
}

fn candidate_rejects<C: Case>(bytes: &[u8]) -> DecodeErrorKind {
    vbare::decode::<C::Wire>(&Bytes::copy_from_slice(bytes))
        .expect_err("candidate rejects the input")
        .kind()
}

// Decision 6: spec bugs in the reference, fixed in the candidate.

#[test]
fn explicit_enum_values_are_written_as_variant_indices_by_the_reference() {
    // `type Sparse enum { FIRST SECOND = 5 THIRD BIG = 300 ... }`
    assert_eq!(cand::Sparse::Second.encode_to_vec(), [0x05]);
    assert_eq!(
        reference_encode::<cases::Sparse>(&cand::Sparse::Second),
        [0x01]
    );
    assert_eq!(cand::Sparse::Big.encode_to_vec(), [0xac, 0x02]);
    assert_eq!(
        reference_encode::<cases::Sparse>(&cand::Sparse::Big),
        [0x03]
    );
    let transformed = reference_to_spec(
        <cases::Sparse as Case>::DESC,
        &reference_encode::<cases::Sparse>(&cand::Sparse::Big),
    )
    .unwrap();
    assert_eq!(transformed, [0xac, 0x02]);
}

#[test]
fn data_longer_than_32_bytes_gets_a_length_prefix_from_the_reference() {
    let value = vbare::FixedBytes::from([7u8; 33]);
    assert_eq!(value.len(), 33);
    let candidate = harness::encode::<cases::Data33>(&value);
    assert_eq!(candidate, [7u8; 33]);
    let reference = reference_encode::<cases::Data33>(&value);
    assert_eq!(reference[0], 33);
    assert_eq!(reference[1..], [7u8; 33]);
}

#[test]
fn fixed_lists_longer_than_32_elements_get_a_count_prefix_from_the_reference() {
    let value: cand::Words33 = std::array::from_fn(|index| index as u16);
    let candidate = harness::encode::<cases::Words33>(&value);
    assert_eq!(candidate.len(), 66);
    let reference = reference_encode::<cases::Words33>(&value);
    assert_eq!(reference[0], 33);
    assert_eq!(reference[1..], candidate[..]);
}

// Decision 7: the candidate rejects malformed input serde_bare 0.5.0 accepts.

#[test]
fn bool_bytes_above_one() {
    assert!(reference_decode::<cases::Flag>(&[0x02]));
    assert_eq!(
        candidate_rejects::<cases::Flag>(&[0x02]),
        DecodeErrorKind::InvalidBool
    );
}

#[test]
fn optional_tags_above_one() {
    assert_eq!(reference_decode::<cases::MaybeU8>(&[0x02, 0x05]), Some(5));
    assert_eq!(
        candidate_rejects::<cases::MaybeU8>(&[0x02, 0x05]),
        DecodeErrorKind::InvalidTag
    );
}

#[test]
fn non_minimal_varints() {
    assert_eq!(reference_decode::<cases::Counter>(&[0x80, 0x00]), 0);
    assert_eq!(
        candidate_rejects::<cases::Counter>(&[0x80, 0x00]),
        DecodeErrorKind::NonCanonicalVarint
    );
    let ten = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00];
    assert_eq!(reference_decode::<cases::Counter>(&ten), (1 << 63) - 1);
    assert_eq!(
        candidate_rejects::<cases::Counter>(&ten),
        DecodeErrorKind::NonCanonicalVarint
    );
}

#[test]
fn duplicate_map_keys() {
    // Two pairs with key "a"; the reference keeps the last value.
    let input = [0x02, 0x01, b'a', 0x01, 0x01, b'a', 0x02];
    let decoded = reference_decode::<cases::Dict>(&input);
    assert_eq!(decoded.len(), 1);
    assert_eq!(decoded.get("a"), Some(&2));
    assert_eq!(
        candidate_rejects::<cases::Dict>(&input),
        DecodeErrorKind::DuplicateMapKey
    );
}

#[test]
fn trailing_bytes() {
    // `serde_bare::from_slice` never checks for the end of input.
    assert_eq!(reference_decode::<cases::Name>(&[0x01, b'a', 0xff]), "a");
    assert_eq!(
        candidate_rejects::<cases::Name>(&[0x01, b'a', 0xff]),
        DecodeErrorKind::TrailingBytes
    );
}

/// Not divergences: both sides reject varints over 10 bytes and a 10th byte above 1.
#[test]
fn both_reject_overlong_varints() {
    let eleven = [
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x81, 0x01,
    ];
    let tenth_above_one = [0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x02];
    for input in [&eleven[..], &tenth_above_one[..]] {
        assert!(<cases::Counter as ReferenceCase>::ref_from_slice(input).is_err());
        assert_eq!(
            candidate_rejects::<cases::Counter>(input),
            DecodeErrorKind::VarintOverflow
        );
    }
}
