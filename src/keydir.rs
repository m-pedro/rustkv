use std::collections::HashMap;

/// Pointer stored in RAM for every live key.
/// Notice the value itself is NOT stored in memory—only its exact byte coordinate on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyDirEntry {
    pub file_id: u64,
    pub value_offset: u64,
    pub value_len: u32,
    pub timestamp_ms: u64,
}

#[derive(Debug, Default, Clone)]
pub struct KeyDir {
    entries: HashMap<String, KeyDirEntry>,
    uncompacted_bytes: u64,
}

impl KeyDir {
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts or updates a key's disk location.
    /// If the key previously existed, adds the old record's on-disk frame size to `uncompacted_bytes`.
    pub fn insert(&mut self, key: String, entry: KeyDirEntry, old_record_size_fn: impl FnOnce(&KeyDirEntry) -> u64) {
        if let Some(prev) = self.entries.insert(key, entry) {
            self.uncompacted_bytes += old_record_size_fn(&prev);
        }
    }

    /// Removes a key (when a tombstone is written) and accounts for the reclaimed live entry.
    pub fn remove(&mut self, key: &str, tombstone_size: u64, old_record_size_fn: impl FnOnce(&KeyDirEntry) -> u64) -> Option<KeyDirEntry> {
        self.uncompacted_bytes += tombstone_size;
        if let Some(prev) = self.entries.remove(key) {
            self.uncompacted_bytes += old_record_size_fn(&prev);
            Some(prev)
        } else {
            None
        }
    }

    pub fn get(&self, key: &str) -> Option<&KeyDirEntry> {
        self.entries.get(key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn uncompacted_bytes(&self) -> u64 {
        self.uncompacted_bytes
    }

    pub fn reset_uncompacted_bytes(&mut self) {
        self.uncompacted_bytes = 0;
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &KeyDirEntry)> {
        self.entries.iter()
    }

    /// Returns a sorted list of all live keys.
    pub fn keys_sorted(&self) -> Vec<String> {
        let mut keys: Vec<String> = self.entries.keys().cloned().collect();
        keys.sort();
        keys
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::record::HEADER_SIZE;

    #[test]
    fn test_keydir_tracks_overwrites_and_uncompacted_bytes() {
        let mut kd = KeyDir::new();
        let entry1 = KeyDirEntry {
            file_id: 1,
            value_offset: 21,
            value_len: 10,
            timestamp_ms: 100,
        };
        kd.insert("user:1".to_string(), entry1, |_| 0);
        assert_eq!(kd.len(), 1);
        assert_eq!(kd.uncompacted_bytes(), 0);

        // Overwrite "user:1" -> old record size (17 + 6 + 10 = 33 bytes) becomes uncompacted waste
        let entry2 = KeyDirEntry {
            file_id: 1,
            value_offset: 54,
            value_len: 12,
            timestamp_ms: 200,
        };
        kd.insert("user:1".to_string(), entry2, |prev| {
            (HEADER_SIZE as u64) + 6 + (prev.value_len as u64)
        });
        assert_eq!(kd.get("user:1").unwrap().value_offset, 54);
        assert_eq!(kd.uncompacted_bytes(), 35);
    }
}
