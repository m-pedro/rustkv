# rustkv

A persistent, crash-resilient **Bitcask** log-structured key-value storage engine in Rust featuring IEEE CRC32 corruption detection, $O(1)$ single-seek reads via an in-memory `KeyDir` index, `.hint` fast-boot files, and background log compaction.

## 15-Day Incremental Engineering Roadmap (Month 2, Days 31–45)

- [x] **Day 31:** Initialize `rustkv` crate, binary frame specification, and 15-day roadmap
- [x] **Day 32:** Add unified `KvError` type and IEEE CRC32 frame integrity verifier (`src/error.rs`, `src/crc.rs`)
- [x] **Day 33:** Implement 17-byte binary log frame encoder, decoder, and tombstone flags (`src/record.rs`)
- [x] **Day 34:** Implement $O(1)$ in-memory `KeyDir` offset index and storage metrics (`src/keydir.rs`)
- [x] **Day 35:** Implement append-only WAL segment writer and positional reader (`src/segment.rs`)
- [x] **Day 36:** Implement Bitcask `open`, `put`, `get`, and tombstone `delete` operations (`src/engine.rs`)
- [x] **Day 37:** Add crash-recovery replay, torn-write truncation, and bit-rot tests (`tests/recovery_test.rs`)
- [x] **Day 38:** Implement automatic active segment rotation at configurable size threshold
- [x] **Day 39:** Implement background log compaction to reclaim dead keys and tombstones
- [x] **Day 40:** Generate and load binary `.hint` files for instant cold-start recovery (`src/hint.rs`)
- [ ] **Day 41:** Verify compaction shrinks disk footprint and preserves live keys across restarts
- [ ] **Day 42:** Build `rustkv` CLI binary supporting `set`, `get`, `rm`, `scan`, `compact`, and `stats`
- [ ] **Day 43:** Wrap engine in thread-safe `SharedKvStore` with multi-reader concurrency tests
- [ ] **Day 44:** Add Criterion throughput benchmarks for sequential/random puts and gets
- [ ] **Day 45:** Complete Bitcask architecture guide, on-disk frame diagram, and GitHub Actions CI
