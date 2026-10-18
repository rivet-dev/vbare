//! The [`Wire`] trait and marker types naming BARE types.
//!
//! A marker names a BARE type rather than a Rust type, because one Rust type can have several
//! encodings: `u64` is the owned form of both `uint` and `u64`. Generated code adds one marker per
//! user-defined type.

use crate::{
    error::{DecodeError, DecodeErrorKind},
    fixed::FixedBytes,
    list::ListRef,
    map::MapRef,
    read::{Reader, Verifier},
    sink::Sink,
    varint,
};
use bytes::Bytes;
use std::{cmp::Ordering, collections::BTreeMap, marker::PhantomData};

/// How a BARE type is verified, read in place, converted to an owned value and encoded.
///
/// # Safety
///
/// [`Reader`] trusts its input: it creates `&str` values without validating UTF-8 again. An
/// implementation must therefore read exactly the layout its [`verify`](Wire::verify) accepted,
/// consuming the same bytes, so that every `str` a reader reaches is one the verifier validated.
/// Generated implementations uphold this by construction; hand-written ones must too.
pub unsafe trait Wire: 'static {
    /// The borrowed form returned by accessors. Cheap to copy.
    type Ref<'a>: Copy;
    /// The owned form used for building and encoding values.
    type Owned;
    /// Lifetime-free state a root [`View`](crate::View) keeps to rebuild its `Ref` without
    /// walking the input again, such as a struct's field offsets.
    type Root: Copy + Send + Sync + 'static;

    /// The encoded size if every value of this type has the same size.
    const FIXED_SIZE: Option<usize>;
    /// A lower bound on the encoded size, used to reject impossible element counts early.
    const MIN_SIZE: usize;

    /// Checks one value at the verifier's position and returns its borrowed form.
    fn verify<'a>(v: &mut Verifier<'a>) -> Result<Self::Ref<'a>, DecodeError>;

    /// Reads one verified value and advances past it.
    fn read<'a>(r: &mut Reader<'a>) -> Self::Ref<'a>;

    /// Advances past one verified value.
    fn skip(r: &mut Reader<'_>) {
        Self::read(r);
    }

    /// Reads a verified value that spans exactly the reader. Types whose end is otherwise found
    /// by walking their contents, such as lists, use the known end instead.
    fn read_span(mut r: Reader<'_>) -> Self::Ref<'_> {
        Self::read(&mut r)
    }

    /// Extracts the state a root view keeps.
    fn root(value: &Self::Ref<'_>) -> Self::Root;

    /// Rebuilds the borrowed form of a root value spanning exactly `r`.
    fn from_root(r: Reader<'_>, root: Self::Root) -> Self::Ref<'_>;

    /// Reads one verified value as owned, sharing `str` and `data` bytes with the input.
    fn read_owned(r: &mut Reader<'_>) -> Self::Owned;

    fn encoded_len(value: &Self::Owned) -> usize;

    fn encode<S: Sink>(value: &Self::Owned, out: &mut S);

    fn ref_encoded_len(value: Self::Ref<'_>) -> usize;

    /// Encodes a borrowed value. Verified input is canonical, so this reproduces the input bytes.
    fn encode_ref<S: Sink>(value: Self::Ref<'_>, out: &mut S);
}

/// A [`Wire`] type that can be a map key. Its borrowed and owned forms must order identically, so
/// a map encoded in owned key order is sorted when read back.
pub trait Key: Wire {
    fn cmp_ref(a: Self::Ref<'_>, b: Self::Ref<'_>) -> Ordering;
}

/// Sums field sizes for a struct's [`Wire::FIXED_SIZE`]: `None` if any field varies in size.
pub const fn fixed_size_sum(sizes: &[Option<usize>]) -> Option<usize> {
    let mut total = 0usize;
    let mut index = 0;
    while index < sizes.len() {
        match sizes[index] {
            Some(size) => match total.checked_add(size) {
                Some(sum) => total = sum,
                None => return None,
            },
            None => return None,
        }
        index += 1;
    }
    Some(total)
}

/// Sums field minimum sizes for a struct's [`Wire::MIN_SIZE`], saturating on overflow.
pub const fn min_size_sum(sizes: &[usize]) -> usize {
    let mut total = 0usize;
    let mut index = 0;
    while index < sizes.len() {
        total = total.saturating_add(sizes[index]);
        index += 1;
    }
    total
}

/// Implements the `Wire` methods shared by types whose `Root` is `()`.
macro_rules! unit_root {
    () => {
        type Root = ();

        #[inline]
        fn root(_: &Self::Ref<'_>) {}

        #[inline]
        fn from_root(r: Reader<'_>, _: ()) -> Self::Ref<'_> {
            Self::read_span(r)
        }
    };
}

/// BARE `uint`.
pub struct Uint;

unsafe impl Wire for Uint {
    type Ref<'a> = u64;
    type Owned = u64;
    unit_root!();

    const FIXED_SIZE: Option<usize> = None;
    const MIN_SIZE: usize = 1;

    #[inline]
    fn verify(v: &mut Verifier<'_>) -> Result<u64, DecodeError> {
        v.uint()
    }

    #[inline]
    fn read(r: &mut Reader<'_>) -> u64 {
        r.uint()
    }

    #[inline]
    fn read_owned(r: &mut Reader<'_>) -> u64 {
        r.uint()
    }

    #[inline]
    fn encoded_len(value: &u64) -> usize {
        varint::uint_len(*value)
    }

    #[inline]
    fn encode<S: Sink>(value: &u64, out: &mut S) {
        varint::put_uint(out, *value);
    }

    #[inline]
    fn ref_encoded_len(value: u64) -> usize {
        varint::uint_len(value)
    }

    #[inline]
    fn encode_ref<S: Sink>(value: u64, out: &mut S) {
        varint::put_uint(out, value);
    }
}

impl Key for Uint {
    #[inline]
    fn cmp_ref(a: u64, b: u64) -> Ordering {
        a.cmp(&b)
    }
}

/// BARE `int`.
pub struct Int;

unsafe impl Wire for Int {
    type Ref<'a> = i64;
    type Owned = i64;
    unit_root!();

    const FIXED_SIZE: Option<usize> = None;
    const MIN_SIZE: usize = 1;

    #[inline]
    fn verify(v: &mut Verifier<'_>) -> Result<i64, DecodeError> {
        v.int()
    }

    #[inline]
    fn read(r: &mut Reader<'_>) -> i64 {
        r.int()
    }

    #[inline]
    fn read_owned(r: &mut Reader<'_>) -> i64 {
        r.int()
    }

    #[inline]
    fn encoded_len(value: &i64) -> usize {
        varint::int_len(*value)
    }

    #[inline]
    fn encode<S: Sink>(value: &i64, out: &mut S) {
        varint::put_int(out, *value);
    }

    #[inline]
    fn ref_encoded_len(value: i64) -> usize {
        varint::int_len(value)
    }

    #[inline]
    fn encode_ref<S: Sink>(value: i64, out: &mut S) {
        varint::put_int(out, value);
    }
}

impl Key for Int {
    #[inline]
    fn cmp_ref(a: i64, b: i64) -> Ordering {
        a.cmp(&b)
    }
}

/// Fixed-width little-endian numbers. Floats keep every bit pattern, including NaN payloads.
macro_rules! fixed_number {
    ($(#[$doc:meta])* $marker:ident, $ty:ty, $size:literal, $key:tt) => {
        $(#[$doc])*
        pub struct $marker;

        unsafe impl Wire for $marker {
            type Ref<'a> = $ty;
            type Owned = $ty;
            unit_root!();

            const FIXED_SIZE: Option<usize> = Some($size);
            const MIN_SIZE: usize = $size;

            #[inline]
            fn verify(v: &mut Verifier<'_>) -> Result<$ty, DecodeError> {
                v.array::<$size>().map(<$ty>::from_le_bytes)
            }

            #[inline]
            fn read(r: &mut Reader<'_>) -> $ty {
                <$ty>::from_le_bytes(r.array::<$size>())
            }

            #[inline]
            fn skip(r: &mut Reader<'_>) {
                r.skip($size);
            }

            #[inline]
            fn read_owned(r: &mut Reader<'_>) -> $ty {
                Self::read(r)
            }

            #[inline]
            fn encoded_len(_: &$ty) -> usize {
                $size
            }

            #[inline]
            fn encode<S: Sink>(value: &$ty, out: &mut S) {
                out.put_slice(&value.to_le_bytes());
            }

            #[inline]
            fn ref_encoded_len(_: $ty) -> usize {
                $size
            }

            #[inline]
            fn encode_ref<S: Sink>(value: $ty, out: &mut S) {
                out.put_slice(&value.to_le_bytes());
            }
        }

        fixed_number!(@key $marker, $ty, $key);
    };
    (@key $marker:ident, $ty:ty, key) => {
        impl Key for $marker {
            #[inline]
            fn cmp_ref(a: $ty, b: $ty) -> Ordering {
                a.cmp(&b)
            }
        }
    };
    (@key $marker:ident, $ty:ty, no_key) => {};
}

fixed_number!(
    /// BARE `u8`.
    U8, u8, 1, key
);
fixed_number!(
    /// BARE `u16`.
    U16, u16, 2, key
);
fixed_number!(
    /// BARE `u32`.
    U32, u32, 4, key
);
fixed_number!(
    /// BARE `u64`.
    U64, u64, 8, key
);
fixed_number!(
    /// BARE `i8`.
    I8, i8, 1, key
);
fixed_number!(
    /// BARE `i16`.
    I16, i16, 2, key
);
fixed_number!(
    /// BARE `i32`.
    I32, i32, 4, key
);
fixed_number!(
    /// BARE `i64`.
    I64, i64, 8, key
);
fixed_number!(
    /// BARE `f32`.
    F32, f32, 4, no_key
);
fixed_number!(
    /// BARE `f64`.
    F64, f64, 8, no_key
);

/// BARE `bool`.
pub struct Bool;

unsafe impl Wire for Bool {
    type Ref<'a> = bool;
    type Owned = bool;
    unit_root!();

    const FIXED_SIZE: Option<usize> = Some(1);
    const MIN_SIZE: usize = 1;

    #[inline]
    fn verify(v: &mut Verifier<'_>) -> Result<bool, DecodeError> {
        v.bool()
    }

    #[inline]
    fn read(r: &mut Reader<'_>) -> bool {
        r.u8() != 0
    }

    #[inline]
    fn skip(r: &mut Reader<'_>) {
        r.skip(1);
    }

    #[inline]
    fn read_owned(r: &mut Reader<'_>) -> bool {
        r.u8() != 0
    }

    #[inline]
    fn encoded_len(_: &bool) -> usize {
        1
    }

    #[inline]
    fn encode<S: Sink>(value: &bool, out: &mut S) {
        out.put_u8(u8::from(*value));
    }

    #[inline]
    fn ref_encoded_len(_: bool) -> usize {
        1
    }

    #[inline]
    fn encode_ref<S: Sink>(value: bool, out: &mut S) {
        out.put_u8(u8::from(value));
    }
}

impl Key for Bool {
    #[inline]
    fn cmp_ref(a: bool, b: bool) -> Ordering {
        a.cmp(&b)
    }
}

/// BARE `str`.
pub struct Str;

unsafe impl Wire for Str {
    type Ref<'a> = &'a str;
    type Owned = crate::Str;
    unit_root!();

    const FIXED_SIZE: Option<usize> = None;
    const MIN_SIZE: usize = 1;

    #[inline]
    fn verify<'a>(v: &mut Verifier<'a>) -> Result<&'a str, DecodeError> {
        v.str()
    }

    #[inline]
    fn read<'a>(r: &mut Reader<'a>) -> &'a str {
        r.str()
    }

    #[inline]
    fn skip(r: &mut Reader<'_>) {
        r.data();
    }

    #[inline]
    fn read_owned(r: &mut Reader<'_>) -> crate::Str {
        r.str_owned()
    }

    #[inline]
    fn encoded_len(value: &crate::Str) -> usize {
        varint::uint_len(value.len() as u64) + value.len()
    }

    #[inline]
    fn encode<S: Sink>(value: &crate::Str, out: &mut S) {
        varint::put_uint(out, value.len() as u64);
        out.put_data(value.as_bytes());
    }

    #[inline]
    fn ref_encoded_len(value: &str) -> usize {
        varint::uint_len(value.len() as u64) + value.len()
    }

    #[inline]
    fn encode_ref<S: Sink>(value: &str, out: &mut S) {
        varint::put_uint(out, value.len() as u64);
        out.put_slice(value.as_bytes());
    }
}

impl Key for Str {
    #[inline]
    fn cmp_ref(a: &str, b: &str) -> Ordering {
        a.cmp(b)
    }
}

/// BARE `data`.
pub struct Data;

unsafe impl Wire for Data {
    type Ref<'a> = &'a [u8];
    type Owned = Bytes;
    unit_root!();

    const FIXED_SIZE: Option<usize> = None;
    const MIN_SIZE: usize = 1;

    #[inline]
    fn verify<'a>(v: &mut Verifier<'a>) -> Result<&'a [u8], DecodeError> {
        v.data()
    }

    #[inline]
    fn read<'a>(r: &mut Reader<'a>) -> &'a [u8] {
        r.data()
    }

    #[inline]
    fn read_owned(r: &mut Reader<'_>) -> Bytes {
        r.data_owned()
    }

    #[inline]
    fn encoded_len(value: &Bytes) -> usize {
        varint::uint_len(value.len() as u64) + value.len()
    }

    #[inline]
    fn encode<S: Sink>(value: &Bytes, out: &mut S) {
        varint::put_uint(out, value.len() as u64);
        out.put_data(value);
    }

    #[inline]
    fn ref_encoded_len(value: &[u8]) -> usize {
        varint::uint_len(value.len() as u64) + value.len()
    }

    #[inline]
    fn encode_ref<S: Sink>(value: &[u8], out: &mut S) {
        varint::put_uint(out, value.len() as u64);
        out.put_slice(value);
    }
}

/// BARE `data[N]` whose owned form is `[u8; N]`. Generated code uses it for N up to 32.
pub struct FixedData<const N: usize>;

unsafe impl<const N: usize> Wire for FixedData<N> {
    type Ref<'a> = &'a [u8; N];
    type Owned = [u8; N];
    unit_root!();

    const FIXED_SIZE: Option<usize> = Some(N);
    const MIN_SIZE: usize = N;

    #[inline]
    fn verify<'a>(v: &mut Verifier<'a>) -> Result<&'a [u8; N], DecodeError> {
        v.array_ref::<N>()
    }

    #[inline]
    fn read<'a>(r: &mut Reader<'a>) -> &'a [u8; N] {
        r.array_ref::<N>()
    }

    #[inline]
    fn skip(r: &mut Reader<'_>) {
        r.skip(N);
    }

    #[inline]
    fn read_owned(r: &mut Reader<'_>) -> [u8; N] {
        r.array::<N>()
    }

    #[inline]
    fn encoded_len(_: &[u8; N]) -> usize {
        N
    }

    #[inline]
    fn encode<S: Sink>(value: &[u8; N], out: &mut S) {
        out.put_slice(value);
    }

    #[inline]
    fn ref_encoded_len(_: &[u8; N]) -> usize {
        N
    }

    #[inline]
    fn encode_ref<S: Sink>(value: &[u8; N], out: &mut S) {
        out.put_slice(value);
    }
}

/// BARE `data[N]` whose owned form is [`FixedBytes<N>`], sharing the input buffer. Generated
/// code uses it for N above 32.
pub struct FixedDataBytes<const N: usize>;

unsafe impl<const N: usize> Wire for FixedDataBytes<N> {
    type Ref<'a> = &'a [u8; N];
    type Owned = FixedBytes<N>;
    unit_root!();

    const FIXED_SIZE: Option<usize> = Some(N);
    const MIN_SIZE: usize = N;

    #[inline]
    fn verify<'a>(v: &mut Verifier<'a>) -> Result<&'a [u8; N], DecodeError> {
        v.array_ref::<N>()
    }

    #[inline]
    fn read<'a>(r: &mut Reader<'a>) -> &'a [u8; N] {
        r.array_ref::<N>()
    }

    #[inline]
    fn skip(r: &mut Reader<'_>) {
        r.skip(N);
    }

    #[inline]
    fn read_owned(r: &mut Reader<'_>) -> FixedBytes<N> {
        let bytes = r.take_bytes(N);
        FixedBytes::new(bytes).unwrap_or_default()
    }

    #[inline]
    fn encoded_len(_: &FixedBytes<N>) -> usize {
        N
    }

    #[inline]
    fn encode<S: Sink>(value: &FixedBytes<N>, out: &mut S) {
        out.put_data(value.as_bytes());
    }

    #[inline]
    fn ref_encoded_len(_: &[u8; N]) -> usize {
        N
    }

    #[inline]
    fn encode_ref<S: Sink>(value: &[u8; N], out: &mut S) {
        out.put_slice(value);
    }
}

/// BARE `void`. Only valid as a union member, where it occupies no bytes after the tag.
pub struct Void;

unsafe impl Wire for Void {
    type Ref<'a> = ();
    type Owned = ();
    unit_root!();

    const FIXED_SIZE: Option<usize> = Some(0);
    const MIN_SIZE: usize = 0;

    #[inline]
    fn verify(_: &mut Verifier<'_>) -> Result<(), DecodeError> {
        Ok(())
    }

    #[inline]
    fn read(_: &mut Reader<'_>) {}

    #[inline]
    fn read_owned(_: &mut Reader<'_>) {}

    #[inline]
    fn encoded_len(_: &()) -> usize {
        0
    }

    #[inline]
    fn encode<S: Sink>(_: &(), _: &mut S) {}

    #[inline]
    fn ref_encoded_len(_: ()) -> usize {
        0
    }

    #[inline]
    fn encode_ref<S: Sink>(_: (), _: &mut S) {}
}

/// BARE `optional<T>`.
pub struct Optional<T>(PhantomData<T>);

unsafe impl<T: Wire> Wire for Optional<T> {
    type Ref<'a> = Option<T::Ref<'a>>;
    type Owned = Option<T::Owned>;
    unit_root!();

    const FIXED_SIZE: Option<usize> = None;
    const MIN_SIZE: usize = 1;

    #[inline]
    fn verify<'a>(v: &mut Verifier<'a>) -> Result<Option<T::Ref<'a>>, DecodeError> {
        match v.array::<1>()? {
            [0] => Ok(None),
            [1] => T::verify(v).map(Some),
            _ => Err(DecodeError::new(
                DecodeErrorKind::InvalidTag,
                v.position() - 1,
            )),
        }
    }

    #[inline]
    fn read<'a>(r: &mut Reader<'a>) -> Option<T::Ref<'a>> {
        if r.u8() == 0 {
            None
        } else {
            Some(T::read(r))
        }
    }

    #[inline]
    fn skip(r: &mut Reader<'_>) {
        if r.u8() != 0 {
            T::skip(r);
        }
    }

    #[inline]
    fn read_span(mut r: Reader<'_>) -> Option<T::Ref<'_>> {
        if r.u8() == 0 {
            None
        } else {
            Some(T::read_span(r.rest()))
        }
    }

    #[inline]
    fn read_owned(r: &mut Reader<'_>) -> Option<T::Owned> {
        if r.u8() == 0 {
            None
        } else {
            Some(T::read_owned(r))
        }
    }

    #[inline]
    fn encoded_len(value: &Option<T::Owned>) -> usize {
        match value {
            Some(inner) => 1 + T::encoded_len(inner),
            None => 1,
        }
    }

    #[inline]
    fn encode<S: Sink>(value: &Option<T::Owned>, out: &mut S) {
        match value {
            Some(inner) => {
                out.put_u8(1);
                T::encode(inner, out);
            }
            None => out.put_u8(0),
        }
    }

    #[inline]
    fn ref_encoded_len(value: Option<T::Ref<'_>>) -> usize {
        match value {
            Some(inner) => 1 + T::ref_encoded_len(inner),
            None => 1,
        }
    }

    #[inline]
    fn encode_ref<S: Sink>(value: Option<T::Ref<'_>>, out: &mut S) {
        match value {
            Some(inner) => {
                out.put_u8(1);
                T::encode_ref(inner, out);
            }
            None => out.put_u8(0),
        }
    }
}

/// Reads elements of a verified list or fixed-length list into a reader over their span.
fn split_elements<'a, T: Wire>(r: &mut Reader<'a>, count: u64) -> Reader<'a> {
    let start = r.offset();
    match T::FIXED_SIZE {
        Some(size) => {
            let len = usize::try_from(count)
                .ok()
                .and_then(|count| count.checked_mul(size))
                .unwrap_or(usize::MAX);
            r.skip(len);
        }
        None => {
            for _ in 0..count {
                if r.remaining().is_empty() {
                    break;
                }
                T::skip(r);
            }
        }
    }
    r.since(start)
}

fn verify_elements<'a, T: Wire>(
    v: &mut Verifier<'a>,
    count: u64,
) -> Result<Reader<'a>, DecodeError> {
    let start = v.position();
    for index in 0..count {
        T::verify(v).map_err(|error| error.in_index(index))?;
    }
    Ok(v.reader_since(start))
}

/// Caps preallocation by the remaining input, which bounds a verified element count.
fn capacity(count: u64, r: &Reader<'_>) -> usize {
    usize::try_from(count)
        .unwrap_or(usize::MAX)
        .min(r.remaining().len())
}

/// BARE `list<T>`.
pub struct List<T>(PhantomData<T>);

unsafe impl<T: Wire> Wire for List<T> {
    type Ref<'a> = ListRef<'a, T>;
    type Owned = Vec<T::Owned>;
    unit_root!();

    const FIXED_SIZE: Option<usize> = None;
    const MIN_SIZE: usize = 1;

    #[inline]
    fn verify<'a>(v: &mut Verifier<'a>) -> Result<ListRef<'a, T>, DecodeError> {
        let count = v.length(T::MIN_SIZE)?;
        Ok(ListRef::new(verify_elements::<T>(v, count)?, count))
    }

    #[inline]
    fn read<'a>(r: &mut Reader<'a>) -> ListRef<'a, T> {
        let count = r.uint();
        ListRef::new(split_elements::<T>(r, count), count)
    }

    #[inline]
    fn read_span(mut r: Reader<'_>) -> ListRef<'_, T> {
        let count = r.uint();
        ListRef::new(r.rest(), count)
    }

    #[inline]
    fn read_owned(r: &mut Reader<'_>) -> Vec<T::Owned> {
        let count = r.uint();
        let mut out = Vec::with_capacity(capacity(count, r));
        for _ in 0..count {
            if r.remaining().is_empty() && T::MIN_SIZE > 0 {
                break;
            }
            out.push(T::read_owned(r));
        }
        out
    }

    #[inline]
    fn encoded_len(value: &Vec<T::Owned>) -> usize {
        let elements: usize = value.iter().map(T::encoded_len).sum();
        varint::uint_len(value.len() as u64) + elements
    }

    #[inline]
    fn encode<S: Sink>(value: &Vec<T::Owned>, out: &mut S) {
        varint::put_uint(out, value.len() as u64);
        for element in value {
            T::encode(element, out);
        }
    }

    #[inline]
    fn ref_encoded_len(value: ListRef<'_, T>) -> usize {
        varint::uint_len(value.len() as u64) + value.as_bytes().len()
    }

    #[inline]
    fn encode_ref<S: Sink>(value: ListRef<'_, T>, out: &mut S) {
        varint::put_uint(out, value.len() as u64);
        out.put_slice(value.as_bytes());
    }
}

/// BARE `list<T>[N]`.
pub struct FixedList<T, const N: usize>(PhantomData<T>);

unsafe impl<T: Wire, const N: usize> Wire for FixedList<T, N> {
    type Ref<'a> = ListRef<'a, T>;
    type Owned = [T::Owned; N];
    unit_root!();

    const FIXED_SIZE: Option<usize> = match T::FIXED_SIZE {
        Some(size) => size.checked_mul(N),
        None => None,
    };
    const MIN_SIZE: usize = T::MIN_SIZE.saturating_mul(N);

    #[inline]
    fn verify<'a>(v: &mut Verifier<'a>) -> Result<ListRef<'a, T>, DecodeError> {
        Ok(ListRef::new(verify_elements::<T>(v, N as u64)?, N as u64))
    }

    #[inline]
    fn read<'a>(r: &mut Reader<'a>) -> ListRef<'a, T> {
        ListRef::new(split_elements::<T>(r, N as u64), N as u64)
    }

    #[inline]
    fn read_span(r: Reader<'_>) -> ListRef<'_, T> {
        ListRef::new(r, N as u64)
    }

    #[inline]
    fn read_owned(r: &mut Reader<'_>) -> [T::Owned; N] {
        std::array::from_fn(|_| T::read_owned(r))
    }

    #[inline]
    fn encoded_len(value: &[T::Owned; N]) -> usize {
        value.iter().map(T::encoded_len).sum()
    }

    #[inline]
    fn encode<S: Sink>(value: &[T::Owned; N], out: &mut S) {
        for element in value {
            T::encode(element, out);
        }
    }

    #[inline]
    fn ref_encoded_len(value: ListRef<'_, T>) -> usize {
        value.as_bytes().len()
    }

    #[inline]
    fn encode_ref<S: Sink>(value: ListRef<'_, T>, out: &mut S) {
        out.put_slice(value.as_bytes());
    }
}

/// BARE `map<K><V>`. Owned maps are `BTreeMap`s and encode in key order. Decoding accepts any
/// pair order and rejects duplicate keys.
pub struct Map<K, V>(PhantomData<(K, V)>);

impl<K: Key, V: Wire> Map<K, V> {
    /// Rejects duplicate keys. Pairs written in strictly ascending key order, as this crate
    /// encodes them, are checked in one pass without allocating. Other orders sort a list of the
    /// keys.
    #[inline]
    fn check_duplicates(
        pairs: Reader<'_>,
        count: u64,
        sorted: bool,
        offset: usize,
    ) -> Result<(), DecodeError> {
        if sorted {
            return Ok(());
        }
        let mut r = pairs;
        let mut keys = Vec::with_capacity(capacity(count, &r));
        for _ in 0..count {
            keys.push(K::read(&mut r));
            V::skip(&mut r);
        }
        keys.sort_unstable_by(|a, b| K::cmp_ref(*a, *b));
        let duplicate = keys
            .windows(2)
            .any(|pair| K::cmp_ref(pair[0], pair[1]) == Ordering::Equal);
        if duplicate {
            Err(DecodeError::new(DecodeErrorKind::DuplicateMapKey, offset))
        } else {
            Ok(())
        }
    }
}

unsafe impl<K: Key, V: Wire> Wire for Map<K, V>
where
    K::Owned: Ord,
{
    type Ref<'a> = MapRef<'a, K, V>;
    type Owned = BTreeMap<K::Owned, V::Owned>;
    unit_root!();

    const FIXED_SIZE: Option<usize> = None;
    const MIN_SIZE: usize = 1;

    #[inline]
    fn verify<'a>(v: &mut Verifier<'a>) -> Result<MapRef<'a, K, V>, DecodeError> {
        let count_offset = v.position();
        let count = v.length(K::MIN_SIZE.saturating_add(V::MIN_SIZE))?;
        let start = v.position();
        let mut previous: Option<K::Ref<'a>> = None;
        let mut sorted = true;
        for index in 0..count {
            let key = K::verify(v).map_err(|error| error.in_index(index))?;
            V::verify(v).map_err(|error| error.in_index(index))?;
            if let Some(previous) = previous {
                if K::cmp_ref(previous, key) != Ordering::Less {
                    sorted = false;
                }
            }
            previous = Some(key);
        }
        let pairs = v.reader_since(start);
        Self::check_duplicates(pairs, count, sorted, count_offset)?;
        Ok(MapRef::new(pairs, count))
    }

    #[inline]
    fn read<'a>(r: &mut Reader<'a>) -> MapRef<'a, K, V> {
        let count = r.uint();
        let start = r.offset();
        for _ in 0..count {
            if r.remaining().is_empty() {
                break;
            }
            K::skip(r);
            V::skip(r);
        }
        MapRef::new(r.since(start), count)
    }

    #[inline]
    fn read_span(mut r: Reader<'_>) -> MapRef<'_, K, V> {
        let count = r.uint();
        MapRef::new(r.rest(), count)
    }

    #[inline]
    fn read_owned(r: &mut Reader<'_>) -> BTreeMap<K::Owned, V::Owned> {
        let count = r.uint();
        let mut out = BTreeMap::new();
        for _ in 0..count {
            if r.remaining().is_empty() {
                break;
            }
            let key = K::read_owned(r);
            let value = V::read_owned(r);
            out.insert(key, value);
        }
        out
    }

    #[inline]
    fn encoded_len(value: &BTreeMap<K::Owned, V::Owned>) -> usize {
        let pairs: usize = value
            .iter()
            .map(|(key, value)| K::encoded_len(key) + V::encoded_len(value))
            .sum();
        varint::uint_len(value.len() as u64) + pairs
    }

    #[inline]
    fn encode<S: Sink>(value: &BTreeMap<K::Owned, V::Owned>, out: &mut S) {
        varint::put_uint(out, value.len() as u64);
        for (key, value) in value {
            K::encode(key, out);
            V::encode(value, out);
        }
    }

    #[inline]
    fn ref_encoded_len(value: MapRef<'_, K, V>) -> usize {
        varint::uint_len(value.len() as u64) + value.as_bytes().len()
    }

    /// Writes the pairs in their original order, so a view re-encodes to its input bytes.
    #[inline]
    fn encode_ref<S: Sink>(value: MapRef<'_, K, V>, out: &mut S) {
        varint::put_uint(out, value.len() as u64);
        out.put_slice(value.as_bytes());
    }
}
