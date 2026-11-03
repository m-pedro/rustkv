use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};

use rustkv::segment::segment_path;
use rustkv::{Bitcask, KvError};
use tempfile::tempdir;

#[test]
fn test_crash_recovery_heals_torn_write_at_tail() {
    let dir = tempdir().unwrap();

    {
        let mut db = Bitcask::open(dir.path()).unwrap();
        db.put("user:100", b"alice".to_vec()).unwrap();
        db.put("user:200", b"bob".to_vec()).unwrap();
        db.sync().unwrap();
    }

    // Simulate a power failure mid-write by appending 9 half-written bytes to 000001.data
    let seg_path = segment_path(dir.path(), 1);
    let valid_size = std::fs::metadata(&seg_path).unwrap().len();
    {
        let mut f = OpenOptions::new().append(true).open(&seg_path).unwrap();
        f.write_all(&[0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04, 0x05])
            .unwrap();
        f.sync_all().unwrap();
    }
    assert_eq!(
        std::fs::metadata(&seg_path).unwrap().len(),
        valid_size + 9
    );

    // Reopening must detect UnexpectedEof at `valid_size`, truncate the torn tail, and recover both keys
    let mut db = Bitcask::open(dir.path()).unwrap();
    assert_eq!(db.get("user:100").unwrap().unwrap(), b"alice");
    assert_eq!(db.get("user:200").unwrap().unwrap(), b"bob");
    assert_eq!(std::fs::metadata(&seg_path).unwrap().len(), valid_size);
}

#[test]
fn test_crash_recovery_detects_bit_rot_crc_mismatch() {
    let dir = tempdir().unwrap();

    {
        let mut db = Bitcask::open(dir.path()).unwrap();
        db.put("account:1", b"balance_1000".to_vec()).unwrap();
        db.sync().unwrap();
    }

    // Flip a bit inside the stored value payload on disk
    let seg_path = segment_path(dir.path(), 1);
    {
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&seg_path)
            .unwrap();
        f.seek(SeekFrom::End(-2)).unwrap();
        f.write_all(&[0xFF]).unwrap();
        f.sync_all().unwrap();
    }

    let err = Bitcask::open(dir.path()).err().expect("expected CRC error");
    assert!(matches!(err, KvError::CrcMismatch { .. }));
}
