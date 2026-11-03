use std::collections::HashMap;
use std::fs;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{KvError, Result};
use crate::keydir::{KeyDir, KeyDirEntry};
use crate::record::{LogRecord, HEADER_SIZE};
use crate::segment::{segment_path, SegmentReader, SegmentWriter};

#[derive(Debug, Clone)]
pub struct Config {
    pub max_segment_bytes: u64,
    pub sync_on_write: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_segment_bytes: 4 * 1024 * 1024, // 4 MiB default segment size
            sync_on_write: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineStats {
    pub live_keys: usize,
    pub active_file_id: u64,
    pub total_segments: usize,
    pub uncompacted_bytes: u64,
}

pub struct Bitcask {
    dir: PathBuf,
    cfg: Config,
    keydir: KeyDir,
    readers: HashMap<u64, SegmentReader>,
    active_writer: SegmentWriter,
}

impl Bitcask {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_config(dir, Config::default())
    }

    pub fn open_with_config(dir: impl AsRef<Path>, cfg: Config) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir)?;

        let mut file_ids = list_data_file_ids(&dir)?;
        file_ids.sort_unstable();

        let mut keydir = KeyDir::new();
        let mut readers = HashMap::new();

        for &file_id in &file_ids {
            let reader = SegmentReader::open(&dir, file_id)?;
            replay_segment(&dir, file_id, &reader, &mut keydir)?;
            readers.insert(file_id, reader);
        }

        let active_file_id = file_ids.last().copied().unwrap_or(1);
        let active_writer = SegmentWriter::open(&dir, active_file_id, cfg.sync_on_write)?;
        readers.entry(active_file_id).or_insert(SegmentReader::open(&dir, active_file_id)?);

        Ok(Self {
            dir,
            cfg,
            keydir,
            readers,
            active_writer,
        })
    }

    /// Writes a key-value pair to the active append-only segment and updates KeyDir.
    pub fn put(&mut self, key: impl Into<String>, value: Vec<u8>) -> Result<()> {
        let key = key.into();
        let key_len = key.len() as u64;
        let record = LogRecord::put(now_ms(), key.clone(), value);

        let meta = self.active_writer.append(&record)?;
        let file_id = self.active_writer.file_id;

        let entry = KeyDirEntry {
            file_id,
            value_offset: meta.value_offset,
            value_len: meta.value_len,
            timestamp_ms: record.timestamp_ms,
        };

        self.keydir.insert(key, entry, |prev| {
            (HEADER_SIZE as u64) + key_len + (prev.value_len as u64)
        });
        Ok(())
    }

    /// Performs an O(1) single-seek positional read using the in-memory KeyDir coordinate.
    pub fn get(&mut self, key: &str) -> Result<Option<Vec<u8>>> {
        let entry = match self.keydir.get(key) {
            Some(&e) => e,
            None => return Ok(None),
        };

        let reader = self
            .readers
            .get_mut(&entry.file_id)
            .ok_or(KvError::SegmentNotFound(entry.file_id))?;

        let value = reader.read_value(entry.value_offset, entry.value_len)?;
        Ok(Some(value))
    }

    /// Appends a tombstone record if the key exists and removes the key from KeyDir.
    pub fn delete(&mut self, key: &str) -> Result<bool> {
        if self.keydir.get(key).is_none() {
            return Ok(false);
        }

        let key_len = key.len() as u64;
        let tombstone = LogRecord::tombstone(now_ms(), key);
        let meta = self.active_writer.append(&tombstone)?;

        self.keydir.remove(key, meta.total_size, |prev| {
            (HEADER_SIZE as u64) + key_len + (prev.value_len as u64)
        });
        Ok(true)
    }

    pub fn sync(&mut self) -> Result<()> {
        self.active_writer.sync()
    }

    pub fn list_keys(&self) -> Vec<String> {
        self.keydir.keys_sorted()
    }

    pub fn stats(&self) -> EngineStats {
        EngineStats {
            live_keys: self.keydir.len(),
            active_file_id: self.active_writer.file_id,
            total_segments: self.readers.len(),
            uncompacted_bytes: self.keydir.uncompacted_bytes(),
        }
    }
}

fn replay_segment(
    dir: &Path,
    file_id: u64,
    reader: &SegmentReader,
    keydir: &mut KeyDir,
) -> Result<()> {
    let path = segment_path(dir, file_id);
    let file = fs::File::open(path)?;
    let mut buf_reader = BufReader::new(file);
    let mut offset = 0u64;

    loop {
        match LogRecord::decode_from(&mut buf_reader, offset) {
            Ok(Some((rec, meta))) => {
                let key_len = rec.key.len() as u64;
                if rec.tombstone {
                    keydir.remove(&rec.key, meta.total_size, |prev| {
                        (HEADER_SIZE as u64) + key_len + (prev.value_len as u64)
                    });
                } else {
                    let entry = KeyDirEntry {
                        file_id,
                        value_offset: meta.value_offset,
                        value_len: meta.value_len,
                        timestamp_ms: rec.timestamp_ms,
                    };
                    keydir.insert(rec.key, entry, |prev| {
                        (HEADER_SIZE as u64) + key_len + (prev.value_len as u64)
                    });
                }
                offset += meta.total_size;
            }
            Ok(None) => break,
            Err(KvError::UnexpectedEof { offset: valid_len }) => {
                // Heal torn write at the tail of the segment by truncating partial bytes
                reader.truncate_to(valid_len)?;
                break;
            }
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

pub(crate) fn list_data_file_ids(dir: &Path) -> Result<Vec<u64>> {
    let mut ids = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("data") {
            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                if let Ok(id) = stem.parse::<u64>() {
                    ids.push(id);
                }
            }
        }
    }
    Ok(ids)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_engine_put_get_delete_and_reopen() {
        let dir = tempdir().unwrap();
        {
            let mut db = Bitcask::open(dir.path()).unwrap();
            db.put("k1", b"v1".to_vec()).unwrap();
            db.put("k2", b"v2".to_vec()).unwrap();
            db.put("k1", b"v1_updated".to_vec()).unwrap();
            assert_eq!(db.get("k1").unwrap().unwrap(), b"v1_updated");
            assert!(db.delete("k2").unwrap());
            assert!(db.get("k2").unwrap().is_none());
        }

        // Reopen from disk and verify KeyDir reconstruction
        let mut db2 = Bitcask::open(dir.path()).unwrap();
        assert_eq!(db2.get("k1").unwrap().unwrap(), b"v1_updated");
        assert!(db2.get("k2").unwrap().is_none());
        assert_eq!(db2.list_keys(), vec!["k1".to_string()]);
    }
}
