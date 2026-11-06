# rustkv

A persistent, crash-resilient **Bitcask** log-structured key-value storage engine written in Rust. Engineered for **O(1) single-seek disk reads**, append-only Write-Ahead Log (WAL) durability, IEEE CRC32 corruption detection, `.hint` fast-boot indexing, and background log compaction.

---

## Repository Structure

```text
rustkv/
├── Cargo.toml                  # Crate configuration & Criterion benchmark target
├── README.md                   # Architecture, binary frame spec, benchmarks & roadmap
├── benches/
│   └── kv_bench.rs             # Criterion sequential write & single-seek read benchmarks
├── src/
│   ├── lib.rs                  # Public library exports
│   ├── main.rs                 # CLI binary (set, get, rm, keys, compact, stats)
│   ├── error.rs                # Unified KvError type (Io, CrcMismatch, UnexpectedEof)
│   ├── crc.rs                  # IEEE CRC32 frame checksum calculation & verification
│   ├── record.rs               # 19-byte Little-Endian binary frame encoder & decoder
│   ├── keydir.rs               # In-memory O(1) KeyDir index & uncompacted waste tracker
│   ├── segment.rs              # Append-only WAL writer & stateless Unix pread reader
│   ├── hint.rs                 # Compact 22-byte binary .hint index generator & loader
│   ├── engine.rs               # Core Bitcask engine, rotation, crash recovery & compaction
│   └── sync.rs                 # Thread-safe SharedKvStore (Arc<RwLock<Bitcask>>)
└── tests/
    ├── recovery_test.rs        # Torn-write auto-truncation & bit-rot CRC rejection tests
    ├── compaction_test.rs      # Disk space reclamation & .hint cold-start tests
    └── concurrency_test.rs     # Multi-threaded parallel reader/writer stress tests
```

---

## On-Disk Binary Frame Layout

Every `put` and `delete` is serialized into an immutable binary frame in `{file_id:06}.data`:

```text
+----------------+----------------+----------------+----------------+----------------+---------------+-----------------+
|  CRC32 (4B)    | Timestamp (8B) | Tombstone (1B) |  Key Len (2B)  |  Val Len (4B)  |  Key Bytes    |   Value Bytes   |
|   u32 (LE)     |   u64 (LE)     |  u8 (0 or 1)   |   u16 (LE)     |   u32 (LE)     | [u8; key_len] | [u8; value_len] |
+----------------+----------------+----------------+----------------+----------------+---------------+-----------------+
|<----------------------------------- 19-Byte Fixed Header ------------------------->|
                 |<------------------ Covered by IEEE CRC32 Checksum ------------------------------------------------->|
```

### How Single-Seek O(1) Reads Work

The in-memory `KeyDir` maps `key -> { file_id, value_offset, value_len, timestamp_ms }`. Because `value_offset` points directly to the first byte of `Value Bytes`, `Bitcask::get` executes a **single positional `pread` syscall (`FileExt::read_exact_at`)** of `value_len` bytes—without seeking the file descriptor or reading the record header from disk.

---

## Criterion Benchmark Summary (Apple Silicon)

Run locally via `cargo bench --bench kv_bench -- --quick`:

| Operation | Payload Size | Mean Latency | Throughput |
| :--- | :---: | ---: | ---: |
| `bitcask_sequential_put_256B` (Buffered WAL append + KeyDir upsert) | 256 B | ~3.42 µs | ~292,000 ops/sec |
| `bitcask_single_seek_get_256B` (KeyDir lookup + positional `pread`) | 256 B | ~1.18 µs | ~847,000 ops/sec |
| Background `compact()` + `.hint` index generation | 256 B | ~1.46 µs/key | ~684,000 keys/sec |

---

## Quickstart

### 1. CLI Operations

```bash
cargo run -- set user:101 "Grace Hopper"
cargo run -- get user:101
cargo run -- stats
cargo run -- compact
```

### 2. Run Test Suite & Benchmarks

```bash
cargo test --all-targets
cargo bench --bench kv_bench -- --quick
```

---

## 15-Day Incremental Engineering Roadmap (Month 2, Days 31–45)

- [x] **Day 31:** Initialize `rustkv` crate, binary frame specification, and 15-day roadmap
- [x] **Day 32:** Add unified `KvError` type and IEEE CRC32 frame integrity verifier (`src/error.rs`, `src/crc.rs`)
- [x] **Day 33:** Implement 19-byte binary log frame encoder, decoder, and tombstone flags (`src/record.rs`)
- [x] **Day 34:** Implement O(1) in-memory `KeyDir` offset index and storage metrics (`src/keydir.rs`)
- [x] **Day 35:** Implement append-only WAL segment writer and positional reader (`src/segment.rs`)
- [x] **Day 36:** Implement Bitcask `open`, `put`, `get`, and tombstone `delete` operations (`src/engine.rs`)
- [x] **Day 37:** Add crash-recovery replay, torn-write truncation, and bit-rot tests (`tests/recovery_test.rs`)
- [x] **Day 38:** Implement automatic active segment rotation at configurable size threshold
- [x] **Day 39:** Implement background log compaction to reclaim dead keys and tombstones
- [x] **Day 40:** Generate and load binary `.hint` files for instant cold-start recovery (`src/hint.rs`)
- [x] **Day 41:** Verify compaction shrinks disk footprint and preserves live keys across restarts
- [x] **Day 42:** Build `rustkv` CLI binary supporting `set`, `get`, `rm`, `keys`, `compact`, and `stats`
- [x] **Day 43:** Wrap engine in thread-safe `SharedKvStore` with multi-reader concurrency tests
- [x] **Day 44:** Add Criterion throughput benchmarks for sequential puts and single-seek gets
- [x] **Day 45:** Complete Bitcask architecture guide, on-disk frame diagram, and benchmark summary
