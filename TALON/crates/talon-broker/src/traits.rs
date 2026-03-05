use async_trait::async_trait;
use chrono::NaiveDate;
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;

use talon_types::broker::*;
use talon_types::error::BrokerError;
use talon_types::flow::{DepthRaw, TapeRaw};
use talon_types::order::{OrderId, OrderIntent, OrderModify};
use talon_types::position::{Position, Symbol};

// ---------------------------------------------------------------------------
// BrokerCommands — synchronous. Runs on spawn_blocking. (S6.2)
// ---------------------------------------------------------------------------

pub trait BrokerCommands: Send + Sync {
    fn submit_order(&self, order: &OrderIntent) -> Result<OrderAck, BrokerError>;
    fn cancel_order(&self, id: &OrderId) -> Result<CancelAck, BrokerError>;
    fn modify_order(&self, id: &OrderId, m: &OrderModify) -> Result<ModifyAck, BrokerError>;
    fn positions(&self) -> Result<Vec<Position>, BrokerError>;
    fn account_snapshot(&self) -> Result<AccountSnapshot, BrokerError>;
    fn settled_cash_delta(&self, date: NaiveDate) -> Result<Decimal, BrokerError>;
    fn broker_id(&self) -> BrokerId;
    fn supports_short(&self, symbol: &Symbol) -> Result<ShortAvailability, BrokerError>;
    fn locate_shares(&self, symbol: &Symbol, qty: u64) -> Result<LocateResult, BrokerError>;
}

// ---------------------------------------------------------------------------
// BrokerStreams — asynchronous. Long-lived streaming connections. (S6.2)
// ---------------------------------------------------------------------------

#[async_trait]
pub trait BrokerStreams: Send + Sync {
    async fn subscribe_quotes(
        &self,
        symbols: &[Symbol],
        tx: Sender<QuoteEvent>,
    ) -> Result<StreamHandle, BrokerError>;

    async fn subscribe_fills(
        &self,
        tx: Sender<FillEvent>,
    ) -> Result<StreamHandle, BrokerError>;

    async fn subscribe_margin_events(
        &self,
        tx: Sender<MarginEvent>,
    ) -> Result<StreamHandle, BrokerError>;

    /// Subscribe to tick-by-tick T&S data for one symbol.
    async fn subscribe_tape(
        &self,
        symbol: &Symbol,
        tx: Sender<TapeRaw>,
    ) -> Result<StreamHandle, BrokerError>;

    /// Subscribe to Level 2 market depth for one symbol.
    async fn subscribe_depth(
        &self,
        symbol: &Symbol,
        rows: usize,
        tx: Sender<DepthRaw>,
    ) -> Result<StreamHandle, BrokerError>;
}
