//! `tx_engine` — streaming transaction processing engine.
//!
//! # Structure
//! - [`engine`] — [`Engine`] struct: owns the client map, dispatches transactions.
//! - [`client`] — [`Client`] struct: per-client account state machine.
//! - [`transaction`] — [`Transaction`] and [`TransactionKind`]: pure data types.
//! - [`io`] — CSV reader / writer and [`io::ClientSnapshot`] output type.
//! - [`error`] — [`EngineError`] and [`ParseError`] error enums.

pub mod client;
pub mod engine;
pub mod error;
pub mod io;
pub mod transaction;

pub use error::{EngineError, ParseError};
pub use transaction::{Transaction, TransactionKind};
