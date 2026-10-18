//! Test 9: `view` and every `Ref` accessor allocate nothing, and `to_owned` allocates only for
//! list and map containers, never for `str` or `data` bytes.

use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    collections::BTreeMap,
};
use vbare::{Bytes, Encode, Message, View};
use vbare_conformance::cand::reference::{all_v1 as all, shared_common_v1 as shared};

/// Counts allocations made by the current thread, so parallel tests do not interfere.
struct Counting;

thread_local! {
    static ALLOCATIONS: Cell<(usize, usize)> = const { Cell::new((0, 0)) };
}

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _ = ALLOCATIONS.try_with(|count| {
            let (allocations, bytes) = count.get();
            count.set((allocations + 1, bytes + layout.size()));
        });
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

/// Runs `f` and returns its result with the number of allocations and bytes allocated.
fn measure<T>(f: impl FnOnce() -> T) -> (T, usize, usize) {
    let (before, before_bytes) = ALLOCATIONS.with(Cell::get);
    let result = f();
    let (after, after_bytes) = ALLOCATIONS.with(Cell::get);
    (result, after - before, after_bytes - before_bytes)
}

/// Encodes `value` into `Bytes` whose shared header already exists, so slicing it later does
/// not allocate.
fn shared<M: Encode>(value: &M) -> Bytes {
    let bytes = Bytes::from(value.encode_to_vec());
    drop(bytes.clone());
    bytes
}

fn primitives(text_len: usize) -> all::Primitives {
    all::Primitives {
        v_uint: 300,
        v_int: -5,
        v_u8: 1,
        v_u16: 2,
        v_u32: 3,
        v_u64: 4,
        v_i8: -1,
        v_i16: -2,
        v_i32: -3,
        v_i64: -4,
        v_f32: 1.5,
        v_f64: f64::NAN,
        v_bool: true,
        v_str: "x".repeat(text_len).into(),
        v_data: Bytes::from(vec![7u8; text_len]),
    }
}

fn point(x: i32) -> shared::SharedPoint {
    shared::SharedPoint { x, y: -x }
}

fn lists(text_len: usize) -> all::Lists {
    all::Lists {
        numbers: vec![1, 2, 3],
        strings: vec!["a".repeat(text_len).into(), "b".repeat(text_len).into()],
        nested: vec![vec!["c".repeat(text_len).into()], vec![]],
        points: vec![point(1), point(2)],
        values: vec![
            all::Value::Str("d".repeat(text_len).into()),
            all::Value::SharedPoint(point(3)),
            all::Value::SharedEmpty,
        ],
        maybe: vec![Some(1), None],
        blobs: vec![Bytes::from(vec![9u8; text_len])],
    }
}

fn maps(text_len: usize) -> all::Maps {
    let text = |c: &str| -> vbare::Str { c.repeat(text_len).into() };
    all::Maps {
        by_str: BTreeMap::from([(text("a"), text("b")), (text("c"), text("d"))]),
        by_u8: BTreeMap::from([(1, 2)]),
        by_u16: BTreeMap::from([(1, true), (2, false)]),
        by_u32: BTreeMap::from([(5, Bytes::from(vec![1u8; text_len]))]),
        by_u64: BTreeMap::from([(6, point(4))]),
        by_i8: BTreeMap::from([(-1, 1)]),
        by_i16: BTreeMap::from([(-2, text("e"))]),
        by_i32: BTreeMap::from([(-3, vec![1, 2])]),
        by_i64: BTreeMap::from([(-4, Some(text("f"))), (-5, None)]),
        by_bool: BTreeMap::from([(false, 0), (true, 1)]),
        by_alias: BTreeMap::from([(7, text("g"))]),
        nested: BTreeMap::from([(text("h"), BTreeMap::from([(text("i"), 1)]))]),
    }
}

#[test]
fn views_and_accessors_do_not_allocate() {
    let bytes = shared(&primitives(64));
    let (view, allocations, _) = measure(|| all::Primitives::view(bytes.clone()).unwrap());
    assert_eq!(allocations, 0, "Primitives::view");
    let (_, allocations, _) = measure(|| {
        let p = view.get();
        let sum = p.v_uint() as i64 + p.v_int() + i64::from(p.v_u8()) + p.v_i64();
        (
            sum,
            p.v_str().len(),
            p.v_data().len(),
            p.v_f64().is_nan(),
            p.v_bool(),
        )
    });
    assert_eq!(allocations, 0, "Primitives accessors");

    let bytes = shared(&lists(64));
    let (view, allocations, _) = measure(|| View::<all::ListsWire>::new(bytes.clone()).unwrap());
    assert_eq!(allocations, 0, "Lists::view");
    let (_, allocations, _) = measure(|| {
        let l = view.get();
        let mut total = 0usize;
        total += l.numbers().iter().sum::<u64>() as usize;
        total += l.strings().iter().map(str::len).sum::<usize>();
        for inner in l.nested() {
            total += inner.iter().map(str::len).sum::<usize>();
        }
        total += l.points().get(1).map_or(0, |p| p.x() as usize);
        for value in l.values() {
            if let all::ValueRef::Str(text) = value {
                total += text.len();
            }
        }
        total += l.maybe().iter().flatten().count();
        total += l.blobs().iter().map(<[u8]>::len).sum::<usize>();
        total
    });
    assert_eq!(allocations, 0, "Lists accessors");

    let bytes = shared(&maps(64));
    let (view, allocations, _) = measure(|| View::<all::MapsWire>::new(bytes.clone()).unwrap());
    assert_eq!(allocations, 0, "Maps::view of key-ordered pairs");
    let (_, allocations, _) = measure(|| {
        let m = view.get();
        let key = "c".repeat(64);
        (
            m.by_str().get(&key).map(str::len),
            m.by_u64().get(6).map(|p| p.y()),
            m.nested().iter().count(),
            m.by_i64().get(-4),
        )
    });
    // `"c".repeat(64)` above is the only allocation.
    assert_eq!(allocations, 1, "Maps accessors");
}

#[test]
fn to_owned_shares_str_and_data_bytes() {
    let bytes = shared(&primitives(1000));
    let view = all::Primitives::view(bytes.clone()).unwrap();
    let (owned, allocations, _) = measure(|| view.to_owned());
    assert_eq!(allocations, 0, "Primitives to_owned");
    assert_eq!(owned.v_str.len(), 1000);
    let (_, allocations, _) = measure(|| all::Primitives::decode(bytes.clone()).unwrap());
    assert_eq!(allocations, 0, "Primitives::decode");
}

#[test]
fn to_owned_allocates_only_containers() {
    for (name, small, large) in [
        ("Lists", shared(&lists(1)), shared(&lists(5000))),
        ("Maps", shared(&maps(1)), shared(&maps(5000))),
    ] {
        let decode = |bytes: &Bytes| match name {
            "Lists" => measure(|| drop(all::Lists::decode(bytes.clone()).unwrap())),
            _ => measure(|| drop(all::Maps::decode(bytes.clone()).unwrap())),
        };
        let (_, small_allocations, small_bytes) = decode(&small);
        let (_, large_allocations, large_bytes) = decode(&large);
        assert!(small_allocations > 0, "{name} has containers");
        assert_eq!(
            small_allocations, large_allocations,
            "{name} allocation count"
        );
        assert_eq!(
            small_bytes, large_bytes,
            "{name} allocated bytes must not grow with str and data lengths"
        );
    }
}
