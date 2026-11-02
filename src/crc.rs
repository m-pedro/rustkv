use crc32fast::Hasher;

use crate::error::{KvError, Result};

/// Computes an IEEE CRC32 checksum over the header metadata (excluding the 4-byte CRC field itself)
/// followed by the raw key and value slices.
pub fn compute_record_crc(meta_bytes: &[u8], key: &[u8], value: &[u8]) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(meta_bytes);
    hasher.update(key);
    hasher.update(value);
    hasher.finalize()
}

/// Verifies that the stored CRC32 matches the computed payload checksum.
pub fn verify_record_crc(
    offset: u64,
    expected_crc: u32,
    meta_bytes: &[u8],
    key: &[u8],
    value: &[u8],
) -> Result<()> {
    let actual_crc = compute_record_crc(meta_bytes, key, value);
    if actual_crc != expected_crc {
        return Err(KvError::CrcMismatch {
            offset,
            expected: expected_crc,
            actual: actual_crc,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc_detects_single_bit_flip() {
        let meta = [1u8, 2, 3, 4, 5, 6, 7, 8, 0, 4, 0, 5, 0, 0, 0];
        let key = b"user";
        let val = b"alice";

        let crc = compute_record_crc(&meta, key, val);
        assert!(verify_record_crc(0, crc, &meta, key, val).is_ok());

        let mut corrupted_val = *val;
        corrupted_val[2] ^= 0b0000_0001; // Flip a single bit
        let err = verify_record_crc(128, crc, &meta, key, &corrupted_val).unwrap_err();
        match err {
            KvError::CrcMismatch { offset, .. } => assert_eq!(offset, 128),
            other => panic!("expected CrcMismatch, got {:?}", other),
        }
    }
}
