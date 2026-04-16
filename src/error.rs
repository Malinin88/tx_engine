//! Error types for the transaction engine.
//!
//! Two enums cover all failure modes:
//! - [`EngineError`]: fatal I/O failures and malformed-input warnings surfaced
//!   through the engine (negative amount, duplicate tx id).
//! - [`ParseError`]: failures produced while reading and parsing the input CSV,
//!   including a distinguished `Io` variant so the CLI can tell a fatal
//!   mid-stream I/O error apart from a skip-able malformed row.

use std::error::Error;
use std::fmt;
use std::io;

/// Errors returned by the engine or the output writer.
///
/// `Io` is fatal (write failure). The remaining variants are malformed-input
/// warnings — the CLI logs them to stderr and continues processing.
#[derive(Debug)]
pub enum EngineError {
    /// A fatal I/O error, e.g. stdout write failure.
    Io(io::Error),
    /// Deposit or withdrawal amount is negative; tx id is in `tx`.
    NegativeAmount { tx: u32 },
    /// A transaction with this id was already recorded for this client.
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
/// — the CLI logs a warning and skips the offending row.
#[derive(Debug)]
pub enum ParseError {
    /// Fatal: an I/O error reading from the underlying reader.
    Io(io::Error),
    /// Fatal: a CSV decoding error (e.g. invalid UTF-8).
    Csv(csv::Error),
    /// Fatal: the CSV header did not match the expected columns.
    WrongHeader,
    /// Malformed: the `type` column contained an unrecognized value.
    UnknownKind { row: usize, raw: String },
    /// Malformed: an amount field was present when it should be absent, or vice versa.
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

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    #[test]
    fn engine_error_io_variant_displays_io_prefix() {
        let err = EngineError::Io(io::Error::other("disk full"));
        assert!(err.to_string().contains("I/O error"));
    }

    #[test]
    fn engine_error_negative_amount_mentions_tx_id() {
        let err = EngineError::NegativeAmount { tx: 42 };
        assert!(err.to_string().contains("42"));
    }

    #[test]
    fn engine_error_duplicate_tx_id_mentions_tx_id() {
        let err = EngineError::DuplicateTxId { tx: 7 };
        assert!(err.to_string().contains("7"));
    }

    #[test]
    fn engine_error_from_io_error_produces_io_variant() {
        let io_err = io::Error::other("test");
        let err = EngineError::from(io_err);
        assert!(matches!(err, EngineError::Io(_)));
    }

    #[test]
    fn parse_error_wrong_header_displays_expected_header() {
        let err = ParseError::WrongHeader;
        assert!(err.to_string().contains("type,client,tx,amount"));
    }

    #[test]
    fn parse_error_unknown_kind_mentions_row_and_raw() {
        let err = ParseError::UnknownKind {
            row: 3,
            raw: "bogus".to_string(),
        };
        let s = err.to_string();
        assert!(s.contains("3"));
        assert!(s.contains("bogus"));
    }

    #[test]
    fn parse_error_from_io_error_produces_io_variant() {
        let io_err = io::Error::other("test");
        let err = ParseError::from(io_err);
        assert!(matches!(err, ParseError::Io(_)));
    }

    #[test]
    fn parse_error_from_csv_error_produces_csv_variant() {
        let csv_err = csv::Error::from(io::Error::other("csv test"));
        let err = ParseError::from(csv_err);
        assert!(matches!(err, ParseError::Csv(_)));
    }
}
