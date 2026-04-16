//! Transaction engine — owns the per-client state map and dispatches each
//! incoming transaction to the appropriate [`Client`] method.

use std::collections::HashMap;

use crate::client::Client;
use crate::error::EngineError;
use crate::io::ClientSnapshot;
use crate::transaction::{Transaction, TransactionKind};

#[derive(Default)]
pub struct Engine {
    clients: HashMap<u16, Client>,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Applies `tx` to the appropriate client account, creating the client if
    /// it has not been seen before.
    ///
    /// Returns `Err` only for conditions worth surfacing as warnings (negative
    /// amount, duplicate tx id). Business-rule failures (insufficient funds,
    /// wrong dispute state, locked account) are silent no-ops.
    pub fn process(&mut self, tx: Transaction) -> Result<(), EngineError> {
        let client = self.clients.entry(tx.client).or_default();

        match tx.kind {
            TransactionKind::Deposit => {
                client.deposit(tx.tx, tx.amount.unwrap_or_default())?;
            }
            TransactionKind::Withdrawal => {
                client.withdraw(tx.tx, tx.amount.unwrap_or_default())?;
            }
            TransactionKind::Dispute => {
                client.dispute(tx.tx);
            }
            TransactionKind::Resolve => {
                client.resolve(tx.tx);
            }
            TransactionKind::Chargeback => {
                client.chargeback(tx.tx);
            }
        }

        Ok(())
    }

    /// Returns an iterator of account snapshots.
    pub fn snapshots(&self) -> impl Iterator<Item = ClientSnapshot> {
        self.clients
            .iter()
            .map(|(id, client)| ClientSnapshot::from_client(*id, client))
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use crate::error::EngineError;
    use crate::transaction::{Transaction, TransactionKind};

    use super::*;

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn deposit(client: u16, tx: u32, amount: Decimal) -> Transaction {
        Transaction {
            kind: TransactionKind::Deposit,
            client,
            tx,
            amount: Some(amount),
        }
    }

    fn withdrawal(client: u16, tx: u32, amount: Decimal) -> Transaction {
        Transaction {
            kind: TransactionKind::Withdrawal,
            client,
            tx,
            amount: Some(amount),
        }
    }

    fn dispute(client: u16, tx: u32) -> Transaction {
        Transaction {
            kind: TransactionKind::Dispute,
            client,
            tx,
            amount: None,
        }
    }

    fn resolve(client: u16, tx: u32) -> Transaction {
        Transaction {
            kind: TransactionKind::Resolve,
            client,
            tx,
            amount: None,
        }
    }

    fn chargeback(client: u16, tx: u32) -> Transaction {
        Transaction {
            kind: TransactionKind::Chargeback,
            client,
            tx,
            amount: None,
        }
    }

    fn snapshot_for(engine: &Engine, client_id: u16) -> ClientSnapshot {
        engine
            .snapshots()
            .find(|s| s.client == client_id)
            .expect("client not found in snapshots")
    }

    // ── Client creation ──────────────────────────────────────────────────────

    #[test]
    fn engine_creates_new_client_on_first_deposit() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, Decimal::from(10))).unwrap();
        assert_eq!(engine.snapshots().count(), 1);
    }

    #[test]
    fn engine_creates_client_for_dispute_against_unknown_tx() {
        let mut engine = Engine::new();
        engine.process(dispute(42, 99)).unwrap();
        assert_eq!(engine.snapshots().count(), 1);
    }

    // ── Routing ──────────────────────────────────────────────────────────────

    #[test]
    fn engine_routes_deposit_to_correct_client() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, Decimal::from(10))).unwrap();
        engine.process(deposit(2, 2, Decimal::from(5))).unwrap();
        assert_eq!(snapshot_for(&engine, 1).available, Decimal::from(10));
    }

    #[test]
    fn engine_routes_withdrawal_to_correct_client() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, Decimal::from(10))).unwrap();
        engine.process(deposit(2, 2, Decimal::from(10))).unwrap();
        engine.process(withdrawal(1, 3, Decimal::from(4))).unwrap();
        assert_eq!(snapshot_for(&engine, 1).available, Decimal::from(6));
    }

    #[test]
    fn engine_routes_dispute_to_correct_client() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, Decimal::from(10))).unwrap();
        engine.process(deposit(2, 2, Decimal::from(10))).unwrap();
        engine.process(dispute(1, 1)).unwrap();
        assert_eq!(snapshot_for(&engine, 1).held, Decimal::from(10));
    }

    #[test]
    fn engine_routes_resolve_to_correct_client() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, Decimal::from(10))).unwrap();
        engine.process(dispute(1, 1)).unwrap();
        engine.process(resolve(1, 1)).unwrap();
        assert_eq!(snapshot_for(&engine, 1).held, Decimal::ZERO);
    }

    #[test]
    fn engine_routes_chargeback_to_correct_client() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, Decimal::from(10))).unwrap();
        engine.process(dispute(1, 1)).unwrap();
        engine.process(chargeback(1, 1)).unwrap();
        assert!(snapshot_for(&engine, 1).locked);
    }

    // ── Snapshots ────────────────────────────────────────────────────────────

    #[test]
    fn engine_snapshots_yields_one_per_client() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, Decimal::from(10))).unwrap();
        engine.process(deposit(2, 2, Decimal::from(5))).unwrap();
        assert_eq!(engine.snapshots().count(), 2);
    }

    #[test]
    fn engine_snapshot_total_equals_available_plus_held() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, Decimal::from(10))).unwrap();
        engine.process(dispute(1, 1)).unwrap();
        let snap = snapshot_for(&engine, 1);
        assert_eq!(snap.total, snap.available + snap.held);
    }

    // ── Error propagation ────────────────────────────────────────────────────

    #[test]
    fn engine_returns_error_for_negative_deposit() {
        let mut engine = Engine::new();
        let result = engine.process(deposit(1, 1, Decimal::from(-1)));
        assert!(matches!(result, Err(EngineError::NegativeAmount { tx: 1 })));
    }

    #[test]
    fn engine_returns_error_for_duplicate_tx_id() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, Decimal::from(10))).unwrap();
        let result = engine.process(deposit(1, 1, Decimal::from(5)));
        assert!(matches!(result, Err(EngineError::DuplicateTxId { tx: 1 })));
    }

    // ── Cross-client safety ──────────────────────────────────

    #[test]
    fn engine_dispute_for_tx_owned_by_different_client_is_noop() {
        let mut engine = Engine::new();
        engine.process(deposit(1, 1, Decimal::from(10))).unwrap();
        // Client 2 references tx 1, which belongs to client 1 — silent no-op.
        engine.process(dispute(2, 1)).unwrap();
        assert_eq!(snapshot_for(&engine, 1).held, Decimal::ZERO);
    }
}
