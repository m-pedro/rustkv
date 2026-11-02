use std::io::Read;

use crate::crc::{compute_record_crc, verify_record_crc};
use crate::error::{KvError, Result};

/// Fixed 19-byte binary frame header:
/// [crc32: 4B][timestamp_ms: 8B][tombstone: 1B][key_len: 2B][val_len: 4B]
pub const HEADER_SIZE: usize = 19;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogRecord {
    pub timestamp_ms: u64,
    pub tombstone: bool,
    pub key: String,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodedRecordMeta {
    pub record_offset: u64,
    pub value_offset: u64,
    pub value_len: u32,
    pub total_size: u64,
}

impl LogRecord {
    pub fn put(timestamp_ms: u64, key: impl Into<String>, value: Vec<u8>) -> Self {
        Self {
            timestamp_ms,
            tombstone: false,
            key: key.into(),
            value,
        }
    }

    pub fn tombstone(timestamp_ms: u64, key: impl Into<String>) -> Self {
        Self {
            timestamp_ms,
            tombstone: true,
            key: key.into(),
            value: Vec::new(),
        }
    }

    /// Encodes the record into its on-disk binary frame protected by IEEE CRC32.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let key_bytes = self.key.as_bytes();
        if key_bytes.len() > u16::MAX as usize {
            return Err(KvError::KeyTooLarge(key_bytes.len()));
        }
        if self.value.len() > u32::MAX as usize {
            return Err(KvError::ValueTooLarge(self.value.len()));
        }

        let key_len = key_bytes.len() as u16;
        let val_len = self.value.len() as u32;

        // 15 bytes of metadata covered by CRC32: [timestamp: 8B][tombstone: 1B][key_len: 2B][val_len: 4B]
        let mut meta = [0u8; 15];
        meta[0..8].copy_from_slice(&self.timestamp_ms.to_le_bytes());
        meta[8] = if self.tombstone { 1 } else { 0 };
        meta[9..11].copy_from_slice(&key_len.to_le_bytes());
        meta[11..15].copy_from_slice(&val_len.to_le_bytes());

        let crc = compute_record_crc(&meta, key_bytes, &self.value);

        let total_len = HEADER_SIZE + key_bytes.len() + self.value.len();
        let mut buf = Vec::with_capacity(total_len);
        buf.extend_from_slice(&crc.to_le_bytes());
        buf.extend_from_slice(&meta);
        buf.extend_from_slice(key_bytes);
        buf.extend_from_slice(&self.value);
        Ok(buf)
    }

    /// Reads and validates a single framed record from `reader` starting at `record_offset`.
    /// Returns `Ok(None)` if the reader is cleanly at EOF on the very first byte.
    pub fn decode_from<R: Read>(
        reader: &mut R,
        record_offset: u64,
    ) -> Result<Option<(Self, DecodedRecordMeta)>> {
        let mut header = [0u8; HEADER_SIZE];
        let mut read_bytes = 0;

        while read_bytes < HEADER_SIZE {
            match reader.read(&mut header[read_bytes..]) {
                Ok(0) => {
                    if read_bytes == 0 {
                        return Ok(None); // Clean EOF at record boundary
                    }
                    return Err(KvError::UnexpectedEof {
                        offset: record_offset,
                    });
                }
                Ok(n) => read_bytes += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(KvError::Io(e)),
            }
        }

        let expected_crc = u32::from_le_bytes(header[0..4].try_into().unwrap());
        let meta = &header[4..HEADER_SIZE];
        let timestamp_ms = u64::from_le_bytes(meta[0..8].try_into().unwrap());
        let tombstone = meta[8] != 0;
        let key_len = u16::from_le_bytes(meta[9..11].try_into().unwrap()) as usize;
        let val_len = u32::from_le_bytes(meta[11..15].try_into().unwrap()) as usize;

        let mut key_bytes = vec![0u8; key_len];
        reader.read_exact(&mut key_bytes).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                KvError::UnexpectedEof {
                    offset: record_offset,
                }
            } else {
                KvError::Io(e)
            }
        })?;

        let mut value = vec![0u8; val_len];
        reader.read_exact(&mut value).map_err(|e| {
            if e.kind() == std::io::ErrorKind::UnexpectedEof {
                KvError::UnexpectedEof {
                    offset: record_offset,
                }
            } else {
                KvError::Io(e)
            }
        })?;

        verify_record_crc(record_offset, expected_crc, meta, &key_bytes, &value)?;

        let key = String::from_utf8(key_bytes).map_err(|_| KvError::InvalidUtf8Key)?;
        let value_offset = record_offset + (HEADER_SIZE as u64) + (key_len as u64);
        let total_size = (HEADER_SIZE + key_len + val_len) as u64;

        Ok(Some((
            Self {
                timestamp_ms,
                tombstone,
                key,
                value,
            },
            DecodedRecordMeta {
                record_offset,
                value_offset,
                value_len: val_len as u32,
                total_size,
            },
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_encode_decode_roundtrip_and_value_offset() {
        let rec = LogRecord::put(1_700_000_000_000, "session:42", b"active_token".to_vec());
        let bytes = rec.encode().unwrap();
        assert_eq!(bytes.len(), HEADER_SIZE + 10 + 12);

        let mut cursor = Cursor::new(&bytes);
        let (decoded, meta) = LogRecord::decode_from(&mut cursor, 100).unwrap().unwrap();
        assert_eq!(decoded, rec);
        assert_eq!(meta.record_offset, 100);
        assert_eq!(meta.value_offset, 100 + (HEADER_SIZE as u64) + 10);
        assert_eq!(meta.value_len, 12);
    }

    #[test]
    fn test_truncated_frame_returns_unexpected_eof() {
        let rec = LogRecord::put(100, "k1", b"hello_world".to_vec());
        let bytes = rec.encode().unwrap();
        let mut cursor = Cursor::new(&bytes[..bytes.len() - 3]);

        let err = LogRecord::decode_from(&mut cursor, 0).unwrap_err();
        assert!(matches!(err, KvError::UnexpectedEof { offset: 0 }));
    }
}
