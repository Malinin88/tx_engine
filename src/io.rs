//! CSV input reader and output writer.

use rust_decimal::Decimal;

use crate::client::Client;

/// A view of a client account, used for CSV output.
/// This view is separated from the core `Client` data structure so it could be easily tweaked
/// for the presentation formatting purposes. 
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
