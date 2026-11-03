use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::FileExt;

use crate::error::Result;
use crate::record::{DecodedRecordMeta, LogRecord, HEADER_SIZE};

pub fn segment_path(dir: &Path, file_id: u64) -> PathBuf {
    dir.join(format!("{:06}.data", file_id))
}

/// Append-only writer for the active WAL segment file.
pub struct SegmentWriter {
    pub file_id: u64,
    writer: BufWriter<File>,
    pub write_offset: u64,
    sync_on_write: bool,
}

impl SegmentWriter {
    pub fn open(dir: &Path, file_id: u64, sync_on_write: bool) -> Result<Self> {
        let path = segment_path(dir, file_id);
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(path)?;
        let write_offset = file.seek(SeekFrom::End(0))?;
        Ok(Self {
            file_id,
            writer: BufWriter::new(file),
            write_offset,
            sync_on_write,
        })
    }

    /// Appends a framed LogRecord and returns its DecodedRecordMeta (including `value_offset`).
    pub fn append(&mut self, record: &LogRecord) -> Result<DecodedRecordMeta> {
        let encoded = record.encode()?;
        let record_offset = self.write_offset;
        self.writer.write_all(&encoded)?;
        self.writer.flush()?;
        if self.sync_on_write {
            self.writer.get_ref().sync_data()?;
        }

        let key_len = record.key.len() as u64;
        let value_len = record.value.len() as u32;
        let value_offset = record_offset + (HEADER_SIZE as u64) + key_len;
        let total_size = encoded.len() as u64;
        self.write_offset += total_size;

        Ok(DecodedRecordMeta {
            record_offset,
            value_offset,
            value_len,
            total_size,
        })
    }

    pub fn sync(&mut self) -> Result<()> {
        self.writer.flush()?;
        self.writer.get_ref().sync_data()?;
        Ok(())
    }
}

/// Random-access reader for committed `.data` segment files.
pub struct SegmentReader {
    pub file_id: u64,
    file: File,
}

impl SegmentReader {
    pub fn open(dir: &Path, file_id: u64) -> Result<Self> {
        let path = segment_path(dir, file_id);
        let file = File::open(path)?;
        Ok(Self { file_id, file })
    }

    /// Performs an O(1) single-seek positional read of `value_len` bytes directly at `value_offset`.
    pub fn read_value(&mut self, value_offset: u64, value_len: u32) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; value_len as usize];
        #[cfg(unix)]
        {
            self.file.read_exact_at(&mut buf, value_offset)?;
        }
        #[cfg(not(unix))]
        {
            use std::io::Read;
            self.file.seek(SeekFrom::Start(value_offset))?;
            self.file.read_exact(&mut buf)?;
        }
        Ok(buf)
    }

    /// Sequentially scans all records in the segment from offset 0 (used during crash recovery).
    pub fn scan_records(&self) -> Result<Vec<(LogRecord, DecodedRecordMeta)>> {
        let cloned = self.file.try_clone()?;
        let mut reader = BufReader::new(cloned);
        let mut offset = 0u64;
        let mut records = Vec::new();

        while let Some((rec, meta)) = LogRecord::decode_from(&mut reader, offset)? {
            offset += meta.total_size;
            records.push((rec, meta));
        }
        Ok(records)
    }

    /// Truncates the segment file at `valid_len` to heal a torn write after a power failure.
    pub fn truncate_to(&self, valid_len: u64) -> Result<()> {
        let path_file = self.file.try_clone()?;
        path_file.set_len(valid_len)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_segment_append_and_single_seek_positional_read() {
        let dir = tempdir().unwrap();
        let mut writer = SegmentWriter::open(dir.path(), 1, true).unwrap();

        let r1 = LogRecord::put(1000, "alpha", b"first_payload".to_vec());
        let r2 = LogRecord::put(2000, "beta", b"second_payload_123".to_vec());

        let m1 = writer.append(&r1).unwrap();
        let m2 = writer.append(&r2).unwrap();

        let mut reader = SegmentReader::open(dir.path(), 1).unwrap();
        // Read second record's value directly by offset without touching the first record
        let val2 = reader.read_value(m2.value_offset, m2.value_len).unwrap();
        assert_eq!(val2, b"second_payload_123");

        let val1 = reader.read_value(m1.value_offset, m1.value_len).unwrap();
        assert_eq!(val1, b"first_payload");

        let scanned = reader.scan_records().unwrap();
        assert_eq!(scanned.len(), 2);
        assert_eq!(scanned[0].0.key, "alpha");
        assert_eq!(scanned[1].0.key, "beta");
    }
}
