use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rustkv::Bitcask;
use tempfile::tempdir;

fn bench_bitcask_throughput(c: &mut Criterion) {
    let dir = tempdir().unwrap();
    let mut db = Bitcask::open(dir.path()).unwrap();
    let payload = vec![b'a'; 256];

    // Pre-populate 1,000 keys for single-seek read benchmark
    for i in 0..1_000 {
        db.put(format!("bench_key_{:04}", i), payload.clone())
            .unwrap();
    }

    let mut counter = 0u64;
    c.bench_function("bitcask_sequential_put_256B", |b| {
        b.iter(|| {
            counter += 1;
            db.put(
                format!("bench_key_{:04}", counter % 1_000),
                black_box(payload.clone()),
            )
            .unwrap();
        })
    });

    c.bench_function("bitcask_single_seek_get_256B", |b| {
        b.iter(|| {
            counter += 1;
            let val = db
                .get(black_box(&format!("bench_key_{:04}", counter % 1_000)))
                .unwrap();
            black_box(val);
        })
    });
}

criterion_group!(benches, bench_bitcask_throughput);
criterion_main!(benches);
