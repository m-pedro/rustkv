use std::fmt;
use std::io;

#[derive(Debug)]
pub enum KvError {
    Io(io::Error),
    CrcMismatch {
        offset: u64,
        expected: u32,
        actual: u32,
    },
    UnexpectedEof {
        offset: u64,
    },
    KeyTooLarge(usize),
    ValueTooLarge(usize),
    InvalidUtf8Key,
    SegmentNotFound(u64),
}

impl fmt::Display for KvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KvError::Io(err) => write!(f, "I/O error: {}", err),
            KvError::CrcMismatch {
                offset,
                expected,
                actual,
            } => write!(
                f,
                "CRC32 corruption at offset {}: expected {:#010x}, got {:#010x}",
                offset, expected, actual
            ),
            KvError::UnexpectedEof { offset } => {
                write!(f, "truncated record (unexpected EOF) at offset {}", offset)
            }
            KvError::KeyTooLarge(len) => write!(f, "key length {} exceeds u16::MAX", len),
            KvError::ValueTooLarge(len) => write!(f, "value length {} exceeds u32::MAX", len),
            KvError::InvalidUtf8Key => write!(f, "record key is not valid UTF-8"),
            KvError::SegmentNotFound(id) => write!(f, "data segment {} not found", id),
        }
    }
}

impl std::error::Error for KvError {}

impl From<io::Error> for KvError {
    fn from(err: io::Error) -> Self {
        KvError::Io(err)
    }
}

pub type Result<T> = std::result::Result<T, KvError>;
