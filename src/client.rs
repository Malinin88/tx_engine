//! Per-client account state machine — balances, dispute history, and locked flag.

use std::collections::HashMap;

use rust_decimal::Decimal;

use crate::error::EngineError;

#[derive(Default)]
pub struct Client {
    available: Decimal,
    held: Decimal,
    locked: bool,
    history: HashMap<u32, RecordedTx>,
}

struct RecordedTx {
    amount: Decimal,
    #[allow(dead_code)]
    kind: RecordedTxKind,
    state: DisputeState,
}

enum RecordedTxKind {
    Deposit,
    Withdrawal,
}

#[derive(PartialEq)]
enum DisputeState {
    Normal,
    Disputed,
    ChargedBack,
}

impl Client {
    pub fn new() -> Self {
        Self {
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            locked: false,
            history: HashMap::new(),
        }
    }

    /// Adds the `amount` to the available balance. Returns an error for negative amounts or
    /// duplicate tx ids. Silently no-ops on a locked account.
    pub fn deposit(&mut self, tx_id: u32, amount: Decimal) -> Result<(), EngineError> {
        if self.locked {
            return Ok(());
        }

        if amount < Decimal::ZERO {
            return Err(EngineError::NegativeAmount { tx: tx_id });
        }

        if self.history.contains_key(&tx_id) {
            return Err(EngineError::DuplicateTxId { tx: tx_id });
        }

        self.available += amount;

        self.history.insert(
            tx_id,
            RecordedTx {
                amount,
                kind: RecordedTxKind::Deposit,
                state: DisputeState::Normal,
            },
        );
        Ok(())
    }

    /// Deducts the `amount` from the available balance. Returns an error for negative amounts or
    /// duplicate tx ids. Silently no-ops on insufficient funds or a locked account.
    pub fn withdraw(&mut self, tx_id: u32, amount: Decimal) -> Result<(), EngineError> {
        if self.locked {
            return Ok(());
        }

        if amount < Decimal::ZERO {
            return Err(EngineError::NegativeAmount { tx: tx_id });
        }

        if self.history.contains_key(&tx_id) {
            return Err(EngineError::DuplicateTxId { tx: tx_id });
        }

        if self.available < amount {
            return Ok(());
        }

        self.available -= amount;

        self.history.insert(
            tx_id,
            RecordedTx {
                amount,
                kind: RecordedTxKind::Withdrawal,
                state: DisputeState::Normal,
            },
        );
        Ok(())
    }

    /// Moves the referenced tx's amount from available to held and marks it `Disputed`.
    /// Silent no-op if the account is locked, the tx can't be found, or the tx is not in `Normal` state.
    pub fn dispute(&mut self, tx_id: u32) {
        if self.locked {
            return;
        }

        let Some(recorded_tx) = self.history.get_mut(&tx_id) else {
            return;
        };

        if recorded_tx.state != DisputeState::Normal {
            return;
        }

        recorded_tx.state = DisputeState::Disputed;

        self.available -= recorded_tx.amount;
        self.held += recorded_tx.amount;
    }

    /// Moves the referenced tx's amount from held back to available and returns it to `Normal`.
    /// Silent no-op if the account is locked, the tx can't be found, or tx is not `Disputed`.
    pub fn resolve(&mut self, tx_id: u32) {
        if self.locked {
            return;
        }

        let Some(recorded_tx) = self.history.get_mut(&tx_id) else {
            return;
        };

        if recorded_tx.state != DisputeState::Disputed {
            return;
        }

        recorded_tx.state = DisputeState::Normal;

        self.available += recorded_tx.amount;
        self.held -= recorded_tx.amount;
    }

    /// Removes held funds for the referenced tx, marks it `ChargedBack`, and locks the account.
    /// Silent no-op if the account is locked, the tx can't be found, or tx is not `Disputed`.
    pub fn chargeback(&mut self, tx_id: u32) {
        if self.locked {
            return;
        }

        let Some(recorded_tx) = self.history.get_mut(&tx_id) else {
            return;
        };

        if recorded_tx.state != DisputeState::Disputed {
            return;
        }

        recorded_tx.state = DisputeState::ChargedBack;

        self.held -= recorded_tx.amount;
        self.locked = true;
    }

    /// Funds available for trading or withdrawal (`total - held`).
    pub fn available(&self) -> Decimal {
        self.available
    }

    /// Funds held pending dispute resolution (`total - available`).
    pub fn held(&self) -> Decimal {
        self.held
    }

    /// Total funds (`available + held`). Computed on every call; never stored.
    pub fn total(&self) -> Decimal {
        self.available + self.held
    }

    /// Whether the account is frozen (a chargeback occurred).
    pub fn locked(&self) -> bool {
        self.locked
    }
}

#[cfg(test)]
mod tests {
    use rust_decimal::Decimal;

    use crate::error::EngineError;

    use super::*;

    // ── Deposit ──────────────────────────────────────────────────────────────

    #[test]
    fn deposit_increases_available() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        assert_eq!(client.available(), Decimal::from(10));
    }

    #[test]
    fn deposit_increases_total() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        assert_eq!(client.total(), Decimal::from(10));
    }

    #[test]
    fn deposit_records_tx_in_history() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        let failing_result = client.deposit(1, Decimal::from(5));
        // A second deposit with the same tx_id must fail, proving the first was recorded.
        assert!(matches!(
            failing_result,
            Err(EngineError::DuplicateTxId { tx: 1 })
        ));
    }

    #[test]
    fn deposit_with_negative_amount_returns_error() {
        let mut client = Client::new();
        let result = client.deposit(1, Decimal::from(-1));
        assert!(matches!(result, Err(EngineError::NegativeAmount { tx: 1 })));
    }

    #[test]
    fn deposit_with_zero_amount_records_tx() {
        let mut client = Client::new();
        // A zero-amount deposit is accepted and records the tx id.
        client.deposit(1, Decimal::ZERO).unwrap();
        let result = client.deposit(1, Decimal::from(5));
        assert!(matches!(result, Err(EngineError::DuplicateTxId { tx: 1 })));
    }

    #[test]
    fn deposit_with_zero_amount_does_not_change_balance() {
        let mut client = Client::new();
        client.deposit(1, Decimal::ZERO).unwrap();
        assert_eq!(client.available(), Decimal::ZERO);
    }

    #[test]
    fn deposit_on_locked_account_is_noop() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.dispute(1);
        client.chargeback(1); // account locked; available=0
        client.deposit(2, Decimal::from(5)).unwrap();
        assert_eq!(client.available(), Decimal::ZERO);
    }

    // ── Withdrawal ────────────────────────────────────────────────────────────

    #[test]
    fn withdrawal_decreases_available() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.withdraw(2, Decimal::from(4)).unwrap();
        assert_eq!(client.available(), Decimal::from(6));
    }

    #[test]
    fn withdrawal_with_insufficient_funds_is_noop() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.withdraw(2, Decimal::from(20)).unwrap();
        assert_eq!(client.available(), Decimal::from(10));
    }

    #[test]
    fn withdrawal_with_insufficient_funds_does_not_change_total() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.withdraw(2, Decimal::from(20)).unwrap();
        assert_eq!(client.total(), Decimal::from(10));
    }

    #[test]
    fn withdrawal_on_locked_account_is_noop() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.dispute(1);
        client.chargeback(1); // locked; available=0
        client.withdraw(2, Decimal::from(5)).unwrap();
        assert_eq!(client.available(), Decimal::ZERO);
    }

    // ── Dispute ──────────────────────────────────────────────────────────────

    #[test]
    fn dispute_decreases_available() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.dispute(1);
        assert_eq!(client.available(), Decimal::ZERO);
    }

    #[test]
    fn dispute_increases_held() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.dispute(1);
        assert_eq!(client.held(), Decimal::from(10));
    }

    #[test]
    fn dispute_does_not_change_total() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.dispute(1);
        assert_eq!(client.total(), Decimal::from(10));
    }

    #[test]
    fn dispute_on_missing_tx_is_noop() {
        let mut client = Client::new();
        client.dispute(99);
        assert_eq!(client.held(), Decimal::ZERO);
    }

    #[test]
    fn dispute_on_already_disputed_is_noop() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.dispute(1);
        client.dispute(1); // second dispute on same tx
        assert_eq!(client.held(), Decimal::from(10));
    }

    #[test]
    fn dispute_on_charged_back_is_noop() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.dispute(1);
        client.chargeback(1); // tx=ChargedBack, account=locked, held=0
        client.dispute(1);
        assert_eq!(client.held(), Decimal::ZERO);
    }

    #[test]
    fn dispute_on_locked_account_is_noop() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.deposit(2, Decimal::from(5)).unwrap();
        client.dispute(1);
        client.chargeback(1); // account locked; tx2 still Normal
        client.dispute(2);
        assert_eq!(client.held(), Decimal::ZERO);
    }

    #[test]
    fn dispute_can_make_available_negative() {
        // Assumption: dispute on a withdrawal still applies
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.withdraw(2, Decimal::from(8)).unwrap(); // available=2
        client.dispute(2); // available becomes -6
        assert!(client.available() < Decimal::ZERO);
    }

    #[test]
    fn re_dispute_after_resolve_is_allowed() {
        // Assumption: resolve returns tx state to Normal, enabling re-dispute.
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.dispute(1);
        client.resolve(1); // back to Normal
        client.dispute(1); // re-dispute
        assert_eq!(client.held(), Decimal::from(10));
    }

    // ── Resolve ──────────────────────────────────────────────────────────────

    #[test]
    fn resolve_increases_available() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.dispute(1);
        client.resolve(1);
        assert_eq!(client.available(), Decimal::from(10));
    }

    #[test]
    fn resolve_decreases_held() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.dispute(1);
        client.resolve(1);
        assert_eq!(client.held(), Decimal::ZERO);
    }

    #[test]
    fn resolve_on_non_disputed_tx_is_noop() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap(); // tx is Normal, not Disputed
        client.resolve(1);
        assert_eq!(client.available(), Decimal::from(10));
    }

    #[test]
    fn resolve_on_locked_account_is_noop() {
        // tx2 remains Disputed after chargeback(1) locks the account.
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.deposit(2, Decimal::from(5)).unwrap();
        client.dispute(1);
        client.dispute(2);
        client.chargeback(1); // locks account; held = 5 (tx2 still Disputed)
        client.resolve(2);
        assert_eq!(client.held(), Decimal::from(5));
    }

    // ── Chargeback ───────────────────────────────────────────────────────────

    #[test]
    fn chargeback_decreases_held() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.dispute(1);
        client.chargeback(1);
        assert_eq!(client.held(), Decimal::ZERO);
    }

    #[test]
    fn chargeback_decreases_total() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.dispute(1);
        client.chargeback(1);
        assert_eq!(client.total(), Decimal::ZERO);
    }

    #[test]
    fn chargeback_locks_account() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.dispute(1);
        client.chargeback(1);
        assert!(client.locked());
    }

    #[test]
    fn chargeback_makes_further_ops_noop() {
        let mut client = Client::new();
        client.deposit(1, Decimal::from(10)).unwrap();
        client.dispute(1);
        client.chargeback(1); // total=0, locked=true
        client.deposit(2, Decimal::from(100)).unwrap();
        assert_eq!(client.total(), Decimal::ZERO);
    }
}
