use std::path::Path;
use std::sync::{Arc, RwLock};

use crate::engine::{Bitcask, Config, EngineStats};
use crate::error::Result;

/// Thread-safe, cloneable handle to a Bitcask storage engine.
/// Concurrent `get()` calls acquire a shared read lock and execute positional `pread` syscalls in parallel.
#[derive(Clone)]
pub struct SharedKvStore {
    inner: Arc<RwLock<Bitcask>>,
}

impl SharedKvStore {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(RwLock::new(Bitcask::open(dir)?)),
        })
    }

    pub fn open_with_config(dir: impl AsRef<Path>, cfg: Config) -> Result<Self> {
        Ok(Self {
            inner: Arc::new(RwLock::new(Bitcask::open_with_config(dir, cfg)?)),
        })
    }

    pub fn put(&self, key: impl Into<String>, value: Vec<u8>) -> Result<()> {
        let mut guard = self.inner.write().expect("rwlock poisoned");
        guard.put(key, value)
    }

    pub fn get(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let guard = self.inner.read().expect("rwlock poisoned");
        guard.get(key)
    }

    pub fn delete(&self, key: &str) -> Result<bool> {
        let mut guard = self.inner.write().expect("rwlock poisoned");
        guard.delete(key)
    }

    pub fn compact(&self) -> Result<u64> {
        let mut guard = self.inner.write().expect("rwlock poisoned");
        guard.compact()
    }

    pub fn stats(&self) -> EngineStats {
        let guard = self.inner.read().expect("rwlock poisoned");
        guard.stats()
    }
}
