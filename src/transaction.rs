//! Transaction data model — pure types, no parsing logic.

use rust_decimal::Decimal;

/// The type of a transaction.
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionKind {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

/// A single transaction read from the input CSV.
///
/// `amount` is `None` for dispute, resolve, and chargeback rows,
/// which do not carry an amount in the spec.
#[derive(Debug, Clone, PartialEq)]
pub struct Transaction {
    pub kind: TransactionKind,
    pub client: u16,
    pub tx: u32,
    pub amount: Option<Decimal>,
}
