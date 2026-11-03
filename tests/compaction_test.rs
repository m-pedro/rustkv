use std::fs;
use std::path::Path;

use rustkv::hint::hint_path;
use rustkv::{Bitcask, Config};
use tempfile::tempdir;

fn total_data_bytes(dir: &Path) -> u64 {
    fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().and_then(|s| s.to_str()) == Some("data"))
        .map(|e| e.metadata().unwrap().len())
        .sum()
}

#[test]
fn test_compaction_reclaims_stale_versions_and_generates_hint_file() {
    let dir = tempdir().unwrap();
    let cfg = Config {
        max_segment_bytes: 512,
        sync_on_write: false,
    };

    let mut db = Bitcask::open_with_config(dir.path(), cfg.clone()).unwrap();

    // Write 50 updates to the same 5 keys + 10 transient keys that we then delete
    for round in 0..50 {
        for k in 0..5 {
            let key = format!("hot_key_{}", k);
            let val = format!("round_{}_payload_padding_1234567890", round).into_bytes();
            db.put(key, val).unwrap();
        }
    }
    for k in 0..10 {
        let key = format!("temp_key_{}", k);
        db.put(&key, vec![b'z'; 64]).unwrap();
        db.delete(&key).unwrap();
    }
    db.sync().unwrap();

    let size_before = total_data_bytes(dir.path());
    let stats_before = db.stats();
    assert!(stats_before.uncompacted_bytes > 5_000);
    assert!(stats_before.total_segments > 5);

    let compact_id = db.compact().unwrap();
    let size_after = total_data_bytes(dir.path());
    let stats_after = db.stats();

    assert!(
        size_after < size_before / 5,
        "expected compacted size ({}) to be < 20% of pre-compaction size ({})",
        size_after,
        size_before
    );
    assert_eq!(stats_after.uncompacted_bytes, 0);
    assert_eq!(stats_after.live_keys, 5);
    assert!(hint_path(dir.path(), compact_id).exists());

    // Reopen from disk (will boot via the generated .hint file) and verify latest values
    drop(db);
    let reopened = Bitcask::open_with_config(dir.path(), cfg).unwrap();
    assert_eq!(reopened.stats().live_keys, 5);
    for k in 0..5 {
        let key = format!("hot_key_{}", k);
        let expected = b"round_49_payload_padding_1234567890".to_vec();
        assert_eq!(reopened.get(&key).unwrap().unwrap(), expected);
    }
    for k in 0..10 {
        assert!(reopened.get(&format!("temp_key_{}", k)).unwrap().is_none());
    }
}
