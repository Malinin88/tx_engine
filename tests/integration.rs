//! End-to-end tests that exercise the compiled `tx_engine` binary against
//! fixture CSV pairs under `tests/fixtures/`.

use std::path::PathBuf;
use std::process::Command;

use assert_cmd::prelude::*;
use rust_decimal::Decimal;
use serde::Deserialize;

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct Row {
    client: u16,
    available: Decimal,
    held: Decimal,
    total: Decimal,
    locked: bool,
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn parse_rows(csv_bytes: &[u8]) -> Vec<Row> {
    let mut reader = csv::ReaderBuilder::new()
        .trim(csv::Trim::All)
        .from_reader(csv_bytes);

    let mut rows: Vec<Row> = reader
        .deserialize()
        .collect::<Result<_, _>>()
        .expect("failed to parse CSV rows");

    rows.sort_by_key(|row| row.client);
    rows
}

fn actual_rows(fixture_name: &str) -> Vec<Row> {
    let input_path = fixtures_dir().join(format!("{fixture_name}.csv"));

    let output = Command::cargo_bin("tx_engine")
        .expect("binary not found")
        .arg(&input_path)
        .output()
        .expect("failed to execute binary");

    assert!(
        output.status.success(),
        "binary exited non-zero for {fixture_name}: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    parse_rows(&output.stdout)
}

fn expected_rows(fixture_name: &str) -> Vec<Row> {
    let expected_path = fixtures_dir().join(format!("{fixture_name}.expected.csv"));
    let expected_bytes = std::fs::read(&expected_path).expect("failed to read expected fixture");
    parse_rows(&expected_bytes)
}

#[test]
fn basic_deposits_and_withdrawals() {
    // the client #2 will not have sufficient funds for withdrawal so the funds will not be deducted
    let fixture = "01_basic_deposits_withdrawals";
    assert_eq!(actual_rows(fixture), expected_rows(fixture));
}

#[test]
fn dispute_resolve_cycle_restores_balance() {
    let fixture = "02_dispute_resolve";
    assert_eq!(actual_rows(fixture), expected_rows(fixture));
}

#[test]
fn chargeback_locks_account_and_blocks_further_ops() {
    let fixture = "03_chargeback_locks_account";
    assert_eq!(actual_rows(fixture), expected_rows(fixture));
}

#[test]
fn malformed_rows_are_skipped() {
    // Four skipped rows:
    //   - `deposit, 1, 3, -10.0` — negative amount rejected by the engine.
    //   - `foobar,  1, 4, 5.0`   — unknown transaction kind rejected at parse time.
    //   - `dispute, 1, 1, 5.0`   — dispute rows must not carry an amount; rejected at parse time.
    //   - `deposit, 1, 1, 5.0`   — duplicate tx id (1 was used above) rejected by the engine.
    let fixture = "04_malformed_rows_skipped";
    assert_eq!(actual_rows(fixture), expected_rows(fixture));
}

#[test]
fn whitespace_and_decimal_precision_are_preserved() {
    // Every column has leading and trailing whitespace that must be trimmed.
    // Amounts use the full 4-decimal precision: 1.2345 + 2.3456 - 0.0001 = 3.58.
    let fixture = "05_whitespace_and_decimal_precision";
    assert_eq!(actual_rows(fixture), expected_rows(fixture));
}
