use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;

use talon_types::broker::*;
use talon_types::error::BrokerError;
use talon_types::flow::{DepthRaw, TapeRaw};
use talon_types::order::{OrderId, OrderIntent, OrderModify};
use talon_types::position::{Position, Symbol};

use crate::traits::{BrokerCommands, BrokerStreams};

// ---------------------------------------------------------------------------
// MockBroker — paper trading / testing
// ---------------------------------------------------------------------------

pub struct MockBroker {
    positions: Mutex<Vec<Position>>,
    balance: Mutex<Decimal>,
}

impl MockBroker {
    pub fn new(starting_balance: Decimal) -> Arc<Self> {
        Arc::new(Self {
            positions: Mutex::new(Vec::new()),
            balance: Mutex::new(starting_balance),
        })
    }
}

impl BrokerCommands for MockBroker {
    fn submit_order(&self, order: &OrderIntent) -> Result<OrderAck, BrokerError> {
        tracing::info!(
            module = %order.module,
            symbol = %order.symbol,
            qty = order.quantity,
            "MockBroker: order submitted"
        );

        let now = Utc::now();
        let pos = Position {
            symbol: order.symbol.clone(),
            module: order.module,
            broker_id: BrokerId::Mock,
            qty: order.quantity as i64,
            avg_entry: Decimal::new(10000, 2), // mock $100.00
            current_price: Decimal::new(10000, 2),
            stop_loss: order.stop_loss,
            take_profit: order.take_profit,
            time_stop: order.time_stop,
            opened_at: now,
            order_id: order.id,
        };
        self.positions.lock().unwrap().push(pos);

        Ok(OrderAck {
            broker_order_id: format!("MOCK-{}", order.id),
            order_id: order.id,
            timestamp: now,
        })
    }

    fn cancel_order(&self, id: &OrderId) -> Result<CancelAck, BrokerError> {
        tracing::info!(order_id = %id, "MockBroker: order cancelled");
        Ok(CancelAck {
            order_id: *id,
            timestamp: Utc::now(),
        })
    }

    fn modify_order(&self, id: &OrderId, _m: &OrderModify) -> Result<ModifyAck, BrokerError> {
        tracing::info!(order_id = %id, "MockBroker: order modified");
        Ok(ModifyAck {
            order_id: *id,
            timestamp: Utc::now(),
        })
    }

    fn positions(&self) -> Result<Vec<Position>, BrokerError> {
        Ok(self.positions.lock().unwrap().clone())
    }

    fn account_snapshot(&self) -> Result<AccountSnapshot, BrokerError> {
        let bal = *self.balance.lock().unwrap();
        Ok(AccountSnapshot {
            broker_id: BrokerId::Mock,
            net_liquidation: bal,
            buying_power: bal,
            cash: CashBalance {
                settled: bal,
                unsettled: Decimal::ZERO,
                pending_settlement: Vec::new(),
            },
            timestamp: Utc::now(),
        })
    }

    fn settled_cash_delta(&self, _date: NaiveDate) -> Result<Decimal, BrokerError> {
        Ok(Decimal::ZERO)
    }

    fn broker_id(&self) -> BrokerId {
        BrokerId::Mock
    }

    fn supports_short(&self, _symbol: &Symbol) -> Result<ShortAvailability, BrokerError> {
        Ok(ShortAvailability::Unavailable)
    }

    fn locate_shares(&self, symbol: &Symbol, _qty: u64) -> Result<LocateResult, BrokerError> {
        Err(BrokerError::LocateFailed {
            symbol: symbol.0.clone(),
        })
    }
}

#[async_trait]
impl BrokerStreams for MockBroker {
    async fn subscribe_quotes(
        &self,
        _symbols: &[Symbol],
        _tx: Sender<QuoteEvent>,
    ) -> Result<StreamHandle, BrokerError> {
        let (cancel_tx, _cancel_rx) = tokio::sync::oneshot::channel();
        let join = tokio::spawn(async {});
        Ok(StreamHandle {
            _cancel: cancel_tx,
            join,
        })
    }

    async fn subscribe_fills(
        &self,
        _tx: Sender<FillEvent>,
    ) -> Result<StreamHandle, BrokerError> {
        let (cancel_tx, _cancel_rx) = tokio::sync::oneshot::channel();
        let join = tokio::spawn(async {});
        Ok(StreamHandle {
            _cancel: cancel_tx,
            join,
        })
    }

    async fn subscribe_margin_events(
        &self,
        _tx: Sender<MarginEvent>,
    ) -> Result<StreamHandle, BrokerError> {
        let (cancel_tx, _cancel_rx) = tokio::sync::oneshot::channel();
        let join = tokio::spawn(async {});
        Ok(StreamHandle {
            _cancel: cancel_tx,
            join,
        })
    }

    async fn subscribe_tape(
        &self,
        _symbol: &Symbol,
        _tx: Sender<TapeRaw>,
    ) -> Result<StreamHandle, BrokerError> {
        let (cancel_tx, _cancel_rx) = tokio::sync::oneshot::channel();
        let join = tokio::spawn(async {});
        Ok(StreamHandle {
            _cancel: cancel_tx,
            join,
        })
    }

    async fn subscribe_depth(
        &self,
        _symbol: &Symbol,
        _rows: usize,
        _tx: Sender<DepthRaw>,
    ) -> Result<StreamHandle, BrokerError> {
        let (cancel_tx, _cancel_rx) = tokio::sync::oneshot::channel();
        let join = tokio::spawn(async {});
        Ok(StreamHandle {
            _cancel: cancel_tx,
            join,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talon_types::module::ModuleId;
    use talon_types::order::{InstrumentType, LegOrder, OrderType, Side};

    #[test]
    fn mock_submit_and_positions() {
        let broker = MockBroker::new(Decimal::new(1000000, 2)); // $10,000

        let intent = OrderIntent {
            id: OrderId::new(),
            module: ModuleId::Firebird,
            symbol: Symbol::new("AAPL"),
            side: Side::Long,
            order_type: OrderType::Single(LegOrder {
                symbol: Symbol::new("AAPL"),
                side: Side::Long,
                qty: 10,
                limit_price: Some(Decimal::new(15000, 2)),
                instrument: InstrumentType::Equity,
            }),
            quantity: 10,
            stop_loss: Some(Decimal::new(14500, 2)),
            take_profit: Some(Decimal::new(16000, 2)),
            time_stop: None,
            confidence: 0.85,
            created_at: Utc::now(),
        };

        let ack = broker.submit_order(&intent).unwrap();
        assert!(ack.broker_order_id.starts_with("MOCK-"));

        let positions = broker.positions().unwrap();
        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].symbol.0, "AAPL");
    }

    #[test]
    fn mock_account_snapshot() {
        let broker = MockBroker::new(Decimal::new(500000, 2)); // $5,000
        let snap = broker.account_snapshot().unwrap();
        assert_eq!(snap.net_liquidation, Decimal::new(500000, 2));
        assert_eq!(snap.broker_id, BrokerId::Mock);
    }
}
