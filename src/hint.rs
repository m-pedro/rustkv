use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use crate::error::{KvError, Result};
use crate::keydir::{KeyDir, KeyDirEntry};

/// Fixed 22-byte binary hint header:
/// [timestamp_ms: 8B][key_len: 2B][value_len: 4B][value_offset: 8B] + [key_bytes]
pub const HINT_HEADER_SIZE: usize = 22;

pub fn hint_path(dir: &Path, file_id: u64) -> PathBuf {
    dir.join(format!("{:06}.hint", file_id))
}

pub fn write_hint_entry(
    writer: &mut BufWriter<File>,
    key: &str,
    entry: &KeyDirEntry,
) -> Result<()> {
    let key_bytes = key.as_bytes();
    if key_bytes.len() > u16::MAX as usize {
        return Err(KvError::KeyTooLarge(key_bytes.len()));
    }
    let mut header = [0u8; HINT_HEADER_SIZE];
    header[0..8].copy_from_slice(&entry.timestamp_ms.to_le_bytes());
    header[8..10].copy_from_slice(&(key_bytes.len() as u16).to_le_bytes());
    header[10..14].copy_from_slice(&entry.value_len.to_le_bytes());
    header[14..22].copy_from_slice(&entry.value_offset.to_le_bytes());

    writer.write_all(&header)?;
    writer.write_all(key_bytes)?;
    Ok(())
}

pub fn open_hint_writer(dir: &Path, file_id: u64) -> Result<BufWriter<File>> {
    let path = hint_path(dir, file_id);
    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    Ok(BufWriter::new(file))
}

pub fn load_hint_file(dir: &Path, file_id: u64, keydir: &mut KeyDir) -> Result<bool> {
    let path = hint_path(dir, file_id);
    if !path.exists() {
        return Ok(false);
    }

    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut header = [0u8; HINT_HEADER_SIZE];

    loop {
        let mut read_bytes = 0;
        while read_bytes < HINT_HEADER_SIZE {
            match reader.read(&mut header[read_bytes..]) {
                Ok(0) if read_bytes == 0 => return Ok(true),
                Ok(0) => return Err(KvError::UnexpectedEof { offset: 0 }),
                Ok(n) => read_bytes += n,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(KvError::Io(e)),
            }
        }

        let timestamp_ms = u64::from_le_bytes(header[0..8].try_into().unwrap());
        let key_len = u16::from_le_bytes(header[8..10].try_into().unwrap()) as usize;
        let value_len = u32::from_le_bytes(header[10..14].try_into().unwrap());
        let value_offset = u64::from_le_bytes(header[14..22].try_into().unwrap());

        let mut key_bytes = vec![0u8; key_len];
        reader.read_exact(&mut key_bytes)?;
        let key = String::from_utf8(key_bytes).map_err(|_| KvError::InvalidUtf8Key)?;

        keydir.insert(
            key,
            KeyDirEntry {
                file_id,
                value_offset,
                value_len,
                timestamp_ms,
            },
            |_| 0,
        );
    }
}
