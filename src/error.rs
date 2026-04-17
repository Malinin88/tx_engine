use std::error::Error;
use std::fmt;
use std::io;

/// Errors returned by the engine or the output writer.
///
/// `Io` is fatal (write failure). The remaining variants are malformed-input
/// warnings — the CLI logs them to stderr and continues processing.
#[derive(Debug)]
pub enum EngineError {
    Io(io::Error),
    NegativeAmount { tx: u32 },
    DuplicateTxId { tx: u32 },
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::NegativeAmount { tx } => write!(f, "tx {tx}: amount must not be negative"),
            Self::DuplicateTxId { tx } => write!(f, "tx {tx}: duplicate transaction id"),
        }
    }
}

impl Error for EngineError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Self::Io(e) = self {
            Some(e)
        } else {
            None
        }
    }
}

impl From<io::Error> for EngineError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<csv::Error> for EngineError {
    fn from(e: csv::Error) -> Self {
        Self::Io(io::Error::other(e))
    }
}

/// Errors produced while reading and parsing the input CSV.
///
/// `Io` and `Csv` are fatal — the CLI exits immediately.
/// `WrongHeader`, `UnknownKind`, and `AmountMismatch` are malformed-row errors
/// — the CLI logs a warning and skips the problematic row.
#[derive(Debug)]
pub enum ParseError {
    Io(io::Error),
    Csv(csv::Error),
    WrongHeader,
    UnknownKind { row: usize, raw: String },
    AmountMismatch { row: usize, kind: String },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {e}"),
            Self::Csv(e) => write!(f, "CSV error: {e}"),
            Self::WrongHeader => write!(f, "wrong CSV header — expected: type,client,tx,amount"),
            Self::UnknownKind { row, raw } => {
                write!(f, "row {row}: unknown transaction type '{raw}'")
            }
            Self::AmountMismatch { row, kind } => {
                write!(f, "row {row}: unexpected amount field for '{kind}'")
            }
        }
    }
}

impl Error for ParseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Csv(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for ParseError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<csv::Error> for ParseError {
    fn from(e: csv::Error) -> Self {
        Self::Csv(e)
    }
}
