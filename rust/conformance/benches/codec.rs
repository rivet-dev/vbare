//! Test 10: serde_bare `from_slice`/`to_vec` against the candidate's `view`, `decode` and
//! `encode` for a small struct, a 4 KiB `data` message and a 1,000-element list.

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use std::hint::black_box;
use vbare::{Bytes, Encode, Message, View};
use vbare_conformance::{
    cand::reference::bench_v1 as bench, cases::reference::bench_v1 as cases, harness::ReferenceCase,
};

fn compare<C: ReferenceCase>(c: &mut Criterion, name: &str, value: C::Owned)
where
    C::Owned: Message<Wire = C::Wire>,
{
    let bytes = Bytes::from(value.encode_to_vec());
    let reference = C::to_ref(&value);
    let mut group = c.benchmark_group(name);
    group.throughput(Throughput::Bytes(bytes.len() as u64));

    group.bench_function(BenchmarkId::new("decode", "serde_bare"), |b| {
        b.iter(|| C::ref_from_slice(black_box(&bytes)).unwrap())
    });
    group.bench_function(BenchmarkId::new("decode", "vbare view"), |b| {
        b.iter(|| View::<C::Wire>::new(black_box(bytes.clone())).unwrap())
    });
    group.bench_function(BenchmarkId::new("decode", "vbare decode"), |b| {
        b.iter(|| C::Owned::decode(black_box(bytes.clone())).unwrap())
    });
    group.bench_function(BenchmarkId::new("encode", "serde_bare"), |b| {
        b.iter(|| C::ref_to_vec(black_box(&reference)))
    });
    group.bench_function(BenchmarkId::new("encode", "vbare"), |b| {
        b.iter(|| black_box(&value).encode_to_vec())
    });
    group.finish();
}

fn codec(c: &mut Criterion) {
    compare::<cases::Small>(
        c,
        "small struct",
        bench::Small {
            id: 42,
            name: "a small message".into(),
            active: true,
            score: 7,
        },
    );
    compare::<cases::Blob>(
        c,
        "4 KiB data",
        bench::Blob {
            id: 1,
            body: Bytes::from(vec![0xab; 4096]),
        },
    );
    compare::<cases::Many>(
        c,
        "1000-element list",
        bench::Many {
            items: (0..1000)
                .map(|id| bench::Item {
                    id,
                    label: format!("item {id}").into(),
                })
                .collect(),
        },
    );
}

criterion_group!(benches, codec);
criterion_main!(benches);
