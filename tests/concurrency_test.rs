use std::thread;

use rustkv::SharedKvStore;
use tempfile::tempdir;

#[test]
fn test_concurrent_readers_and_writers() {
    let dir = tempdir().unwrap();
    let store = SharedKvStore::open(dir.path()).unwrap();

    // Seed 100 initial keys
    for i in 0..100 {
        store
            .put(format!("k:{}", i), format!("val:{}", i).into_bytes())
            .unwrap();
    }

    let mut handles = Vec::new();

    // Spawn 4 writer threads updating keys concurrently
    for t in 0..4 {
        let writer_store = store.clone();
        handles.push(thread::spawn(move || {
            for i in 0..50 {
                let key = format!("w:{}:{}", t, i);
                writer_store.put(key, b"concurrent_write".to_vec()).unwrap();
            }
        }));
    }

    // Spawn 8 reader threads performing parallel pread lookups under RwLock::read
    for _ in 0..8 {
        let reader_store = store.clone();
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let key = format!("k:{}", i);
                let got = reader_store.get(&key).unwrap().unwrap();
                assert_eq!(got, format!("val:{}", i).into_bytes());
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(store.stats().live_keys, 100 + 4 * 50);
}
