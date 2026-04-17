use std::io;

use csv::{ReaderBuilder, StringRecord, StringRecordsIntoIter, Trim, WriterBuilder};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::client::Client;
use crate::error::{EngineError, ParseError};
use crate::transaction::{Transaction, TransactionKind};

/// A view of a client account, used for CSV output.
/// This view is separated from the core `Client` data structure so it could be easily tweaked
/// for the presentation formatting purposes.
#[derive(Serialize)]
pub struct ClientSnapshot {
    pub client: u16,
    pub available: Decimal,
    pub held: Decimal,
    pub total: Decimal,
    pub locked: bool,
}

impl ClientSnapshot {
    pub fn from_client(id: u16, client: &Client) -> Self {
        Self {
            client: id,
            available: client.available(),
            held: client.held(),
            total: client.total(),
            locked: client.locked(),
        }
    }
}

#[derive(Deserialize)]
struct RawRecord {
    #[serde(rename = "type")]
    kind: String,
    client: u16,
    tx: u32,
    amount: Option<Decimal>,
}

const EXPECTED_HEADER: [&str; 4] = ["type", "client", "tx", "amount"];

struct TransactionReader<R> {
    inner: StringRecordsIntoIter<R>,
    headers: StringRecord,
    row: usize,
    pending_error: Option<ParseError>,
}

impl<R: io::Read> TransactionReader<R> {
    fn new(reader: R) -> Self {
        let mut csv_reader = ReaderBuilder::new().trim(Trim::All).from_reader(reader);

        let (headers, pending_error) = match csv_reader.headers() {
            Err(e) => (StringRecord::new(), Some(ParseError::from(e))),
            Ok(header_record) => {
                let actual: Vec<&str> = header_record.iter().collect();
                if actual != EXPECTED_HEADER {
                    (header_record.clone(), Some(ParseError::WrongHeader))
                } else {
                    (header_record.clone(), None)
                }
            }
        };

        Self {
            inner: csv_reader.into_records(),
            headers,
            row: 1,
            pending_error,
        }
    }
}

impl<R: io::Read> Iterator for TransactionReader<R> {
    type Item = Result<(Transaction, usize), ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(err) = self.pending_error.take() {
            return Some(Err(err));
        }

        let csv_result = self.inner.next()?;
        let row = self.row;
        self.row += 1;

        let record = match csv_result {
            Err(e) => return Some(Err(ParseError::from(e))),
            Ok(rec) => rec,
        };

        let raw: RawRecord = match record.deserialize(Some(&self.headers)) {
            Err(e) => return Some(Err(ParseError::from(e))),
            Ok(raw) => raw,
        };

        let kind = match raw.kind.to_lowercase().as_ref() {
            "deposit" => TransactionKind::Deposit,
            "withdrawal" => TransactionKind::Withdrawal,
            "dispute" => TransactionKind::Dispute,
            "resolve" => TransactionKind::Resolve,
            "chargeback" => TransactionKind::Chargeback,
            other => {
                return Some(Err(ParseError::UnknownKind {
                    row,
                    raw: other.to_string(),
                }));
            }
        };

        let requires_amount =
            matches!(kind, TransactionKind::Deposit | TransactionKind::Withdrawal);

        if requires_amount && raw.amount.is_none() {
            return Some(Err(ParseError::AmountMismatch {
                row,
                kind: raw.kind,
            }));
        }

        if !requires_amount && raw.amount.is_some() {
            return Some(Err(ParseError::AmountMismatch {
                row,
                kind: raw.kind,
            }));
        }

        Some(Ok((
            Transaction {
                kind,
                client: raw.client,
                tx: raw.tx,
                amount: raw.amount,
            },
            row,
        )))
    }
}

/// Returns an iterator that reads and parses transactions from `reader`.
///
/// The first item is `Err(ParseError::WrongHeader)` if the CSV header does not match
/// `type,client,tx,amount`. Subsequent items are `Ok((transaction, row))` or
/// `Err(parse_error)` for malformed rows, where `row` is 1-indexed from the first data row.
pub fn read_transactions<R: io::Read>(
    reader: R,
) -> impl Iterator<Item = Result<(Transaction, usize), ParseError>> {
    TransactionReader::new(reader)
}

/// Writes client snapshots as CSV to `writer`.
///
/// The header row (`client,available,held,total,locked`) is written automatically before
/// the first data row.
pub fn write_clients<W: io::Write>(
    writer: W,
    snapshots: impl IntoIterator<Item = ClientSnapshot>,
) -> Result<(), EngineError> {
    let mut csv_writer = WriterBuilder::new().from_writer(writer);
    for snapshot in snapshots {
        csv_writer.serialize(snapshot)?;
    }
    csv_writer.flush()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io;

    use rust_decimal::Decimal;

    use crate::error::ParseError;
    use crate::transaction::TransactionKind;

    use super::*;

    fn csv_cursor(csv_str: &str) -> io::Cursor<Vec<u8>> {
        io::Cursor::new(csv_str.as_bytes().to_vec())
    }

    fn make_snapshot(available: Decimal, held: Decimal, locked: bool) -> ClientSnapshot {
        ClientSnapshot {
            client: 1,
            available,
            held,
            total: available + held,
            locked,
        }
    }

    // ── read_transactions ─────────────────────────────────────────────────────

    #[test]
    fn read_tolerates_whitespace_around_columns() {
        let csv = " type , client , tx , amount \n deposit , 1 , 1 , 10.0 \n";
        let mut iter = read_transactions(csv_cursor(csv));
        let (tx, _) = iter.next().unwrap().unwrap();
        assert_eq!(tx.kind, TransactionKind::Deposit);
    }

    #[test]
    fn read_parses_four_decimal_places() {
        let csv = "type,client,tx,amount\ndeposit,1,1,1.2345\n";
        let mut iter = read_transactions(csv_cursor(csv));
        let (tx, _) = iter.next().unwrap().unwrap();
        assert_eq!(tx.amount, Some("1.2345".parse::<Decimal>().unwrap()));
    }

    #[test]
    fn read_rejects_deposit_without_amount() {
        let csv = "type,client,tx,amount\ndeposit,1,1,\n";
        let mut iter = read_transactions(csv_cursor(csv));
        let result = iter.next().unwrap();
        assert!(matches!(result, Err(ParseError::AmountMismatch { .. })));
    }

    #[test]
    fn read_rejects_dispute_with_amount() {
        let csv = "type,client,tx,amount\ndispute,1,1,5.0\n";
        let mut iter = read_transactions(csv_cursor(csv));
        let result = iter.next().unwrap();
        assert!(matches!(result, Err(ParseError::AmountMismatch { .. })));
    }

    #[test]
    fn read_rejects_unknown_kind() {
        let csv = "type,client,tx,amount\nfoo,1,1,10.0\n";
        let mut iter = read_transactions(csv_cursor(csv));
        let result = iter.next().unwrap();
        assert!(matches!(result, Err(ParseError::UnknownKind { .. })));
    }

    #[test]
    fn read_rejects_wrong_header() {
        let csv = "kind,client,tx,amount\ndeposit,1,1,10.0\n";
        let mut iter = read_transactions(csv_cursor(csv));
        let result = iter.next().unwrap();
        assert!(matches!(result, Err(ParseError::WrongHeader)));
    }

    #[test]
    fn read_yields_row_numbers_starting_at_one() {
        let csv = "type,client,tx,amount\ndeposit,1,1,10.0\n";
        let mut iter = read_transactions(csv_cursor(csv));
        let (_, row) = iter.next().unwrap().unwrap();
        assert_eq!(row, 1);
    }

    // ── write_clients ─────────────────────────────────────────────────────────

    #[test]
    fn write_emits_expected_header() {
        let snapshot = make_snapshot(Decimal::from(5), Decimal::ZERO, false);
        let mut buf = Vec::new();
        write_clients(&mut buf, vec![snapshot]).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.starts_with("client,available,held,total,locked\n"));
    }

    #[test]
    fn write_emits_lowercase_bool_for_locked() {
        let snapshot = make_snapshot(Decimal::ZERO, Decimal::ZERO, true);
        let mut buf = Vec::new();
        write_clients(&mut buf, vec![snapshot]).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains(",true\n"));
    }

    #[test]
    fn write_emits_decimal_with_natural_precision() {
        let snapshot = make_snapshot(Decimal::from(5), Decimal::ZERO, false);
        let mut buf = Vec::new();
        write_clients(&mut buf, vec![snapshot]).unwrap();
        let output = String::from_utf8(buf).unwrap();
        // "5" not "5.0000"
        assert!(output.contains(",5,"));
    }

    // ── ClientSnapshot ────────────────────────────────────────────────────────

    #[test]
    fn client_snapshot_total_equals_available_plus_held() {
        let available = Decimal::from(7);
        let held = Decimal::from(3);
        let snapshot = ClientSnapshot {
            client: 1,
            available,
            held,
            total: available + held,
            locked: false,
        };
        assert_eq!(snapshot.total, snapshot.available + snapshot.held);
    }
}
