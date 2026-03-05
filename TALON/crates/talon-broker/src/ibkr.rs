use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{NaiveDate, Utc};

use rust_decimal::Decimal;
use tokio::sync::mpsc::Sender;
use tracing;

use ibapi::accounts::{AccountSummaryResult, AccountSummaryTags, PositionUpdate};
use ibapi::contracts::Contract;
use ibapi::market_data::realtime::{BarSize, MarketDepths, WhatToShow};
use ibapi::market_data::TradingHours;
use ibapi::Client as IbClient;

use talon_types::broker::*;
use talon_types::error::BrokerError;
use talon_types::flow::{DepthOp, DepthRaw, DepthSide, TapeRaw};
use talon_types::module::ModuleId;
use talon_types::order::{OrderId, OrderIntent, OrderModify, Side};
use talon_types::position::{Position, Symbol};

use crate::traits::{BrokerCommands, BrokerStreams};

// ---------------------------------------------------------------------------
// IBKR Broker — connects to IB Gateway via TWS API (ibapi crate)
// ---------------------------------------------------------------------------

pub struct IbkrBroker {
    client: Arc<IbClient>,
    /// Maps TALON OrderId (UUID) -> IBKR order_id (i32)
    order_map: Mutex<HashMap<OrderId, i32>>,
    /// Account ID from managed_accounts()
    account_id: Mutex<String>,
}

impl IbkrBroker {
    /// Connect to IB Gateway.
    ///
    /// # Arguments
    /// * `host` — e.g. "127.0.0.1"
    /// * `port` — e.g. 4002 (IB Gateway paper)
    /// * `client_id` — unique per connection (1 = trading, 2 = vault)
    ///
    /// Connect to IB Gateway with exponential backoff retry.
    ///
    /// Retries up to `max_retries` times with delays: 1s, 2s, 4s, 8s (capped).
    pub async fn connect(host: &str, port: u16, client_id: i32) -> Result<Arc<Self>, BrokerError> {
        Self::connect_with_retries(host, port, client_id, 4).await
    }

    async fn connect_with_retries(
        host: &str,
        port: u16,
        client_id: i32,
        max_retries: u32,
    ) -> Result<Arc<Self>, BrokerError> {
        let addr = format!("{host}:{port}");
        let mut last_err = String::new();

        for attempt in 0..=max_retries {
            if attempt > 0 {
                let delay_secs = (1u64 << (attempt - 1)).min(8);
                tracing::warn!(
                    attempt,
                    delay_secs,
                    "IBKR connection failed, retrying..."
                );
                tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
            }

            tracing::info!(address = %addr, client_id, attempt, "Connecting to IB Gateway...");

            match IbClient::connect(&addr, client_id).await {
                Ok(client) => {
                    tracing::info!(
                        server_version = client.server_version(),
                        "Connected to IB Gateway"
                    );

                    let accounts = client
                        .managed_accounts()
                        .await
                        .map_err(|e| {
                            BrokerError::ConnectionLost(format!("managed_accounts failed: {e}"))
                        })?;

                    let account_id = accounts.first().cloned().unwrap_or_default();
                    tracing::info!(account_id = %account_id, "IBKR account discovered");

                    return Ok(Arc::new(Self {
                        client: Arc::new(client),
                        order_map: Mutex::new(HashMap::new()),
                        account_id: Mutex::new(account_id),
                    }));
                }
                Err(e) => {
                    last_err = e.to_string();
                    tracing::warn!(
                        attempt,
                        error = %last_err,
                        "IBKR connection attempt failed"
                    );
                }
            }
        }

        Err(BrokerError::ConnectionLost(format!(
            "IBKR connect failed after {} retries: {last_err}",
            max_retries
        )))
    }

    fn account_id(&self) -> String {
        self.account_id.lock().unwrap().clone()
    }

    fn map_ibkr_position(ib_pos: &ibapi::accounts::Position) -> Position {
        Position {
            symbol: Symbol::new(ib_pos.contract.symbol.as_str()),
            module: ModuleId::Firebird, // IBKR doesn't track modules — placeholder
            broker_id: BrokerId::Ibkr,
            qty: ib_pos.position as i64,
            avg_entry: Decimal::from_f64_retain(ib_pos.average_cost)
                .unwrap_or(Decimal::ZERO),
            current_price: Decimal::from_f64_retain(ib_pos.average_cost)
                .unwrap_or(Decimal::ZERO), // will be updated by quotes
            stop_loss: None,
            take_profit: None,
            time_stop: None,
            opened_at: Utc::now(), // IBKR doesn't provide open time via this API
            order_id: OrderId::new(),
        }
    }
}

impl BrokerCommands for IbkrBroker {
    fn submit_order(&self, order: &OrderIntent) -> Result<OrderAck, BrokerError> {
        // Build ibapi Contract
        let contract = Contract::stock(order.symbol.to_string()).build();

        // Build and submit order using tokio runtime
        let client = Arc::clone(&self.client);
        let qty = order.quantity;
        let side = order.side;
        let limit_price = match &order.order_type {
            talon_types::order::OrderType::Single(leg) => leg.limit_price,
            talon_types::order::OrderType::Spread { .. } => None,
        };

        // We need to run the async client from a sync context.
        // BrokerCommands::submit_order is called inside spawn_blocking,
        // so we use Handle::current().block_on().
        let handle = tokio::runtime::Handle::current();
        let result = handle.block_on(async {
            let builder = client.order(&contract);
            let builder = match side {
                Side::Long => builder.buy(qty as f64),
                Side::Short => builder.sell(qty as f64),
            };

            let builder = if let Some(price) = limit_price {
                let price_f64 = price.to_string().parse::<f64>().unwrap_or(0.0);
                builder.limit(price_f64)
            } else {
                builder.market()
            };

            builder.submit().await
        });

        match result {
            Ok(ib_order_id) => {
                let ib_id = ib_order_id.value();

                // Store the mapping
                self.order_map
                    .lock()
                    .unwrap()
                    .insert(order.id, ib_id);

                tracing::info!(
                    order_id = %order.id,
                    ib_order_id = ib_id,
                    symbol = %order.symbol,
                    "Order submitted to IBKR"
                );

                Ok(OrderAck {
                    broker_order_id: ib_id.to_string(),
                    order_id: order.id,
                    timestamp: Utc::now(),
                })
            }
            Err(e) => Err(BrokerError::OrderRejected(format!("IBKR rejected: {e}"))),
        }
    }

    fn cancel_order(&self, id: &OrderId) -> Result<CancelAck, BrokerError> {
        let ib_order_id = self
            .order_map
            .lock()
            .unwrap()
            .get(id)
            .copied()
            .ok_or_else(|| BrokerError::OrderRejected(format!("Unknown order: {id}")))?;

        let client = Arc::clone(&self.client);
        let handle = tokio::runtime::Handle::current();

        let _subscription = handle
            .block_on(async { client.cancel_order(ib_order_id, "").await })
            .map_err(|e| BrokerError::OrderRejected(format!("Cancel failed: {e}")))?;

        tracing::info!(order_id = %id, ib_order_id, "Order cancelled on IBKR");

        Ok(CancelAck {
            order_id: *id,
            timestamp: Utc::now(),
        })
    }

    fn modify_order(&self, _id: &OrderId, _m: &OrderModify) -> Result<ModifyAck, BrokerError> {
        Err(BrokerError::Unsupported(
            "Order modification not yet implemented for IBKR".into(),
        ))
    }

    fn positions(&self) -> Result<Vec<Position>, BrokerError> {
        let client = Arc::clone(&self.client);
        let handle = tokio::runtime::Handle::current();

        let mut subscription = handle
            .block_on(async { client.positions().await })
            .map_err(|e| BrokerError::ConnectionLost(format!("positions failed: {e}")))?;

        let mut positions = Vec::new();

        // Drain position updates until PositionEnd
        loop {
            let update = handle.block_on(async { subscription.next().await });
            match update {
                Some(Ok(PositionUpdate::Position(pos))) => {
                    positions.push(Self::map_ibkr_position(&pos));
                }
                Some(Ok(PositionUpdate::PositionEnd)) => break,
                Some(Err(e)) => {
                    tracing::warn!(error = %e, "Error reading position");
                    break;
                }
                None => break,
            }
        }

        Ok(positions)
    }

    fn account_snapshot(&self) -> Result<AccountSnapshot, BrokerError> {
        let client = Arc::clone(&self.client);
        let handle = tokio::runtime::Handle::current();

        let group = ibapi::accounts::types::AccountGroup("All".to_string());
        let tags = &[
            AccountSummaryTags::NET_LIQUIDATION,
            AccountSummaryTags::BUYING_POWER,
            AccountSummaryTags::TOTAL_CASH_VALUE,
            AccountSummaryTags::SETTLED_CASH,
        ];

        let mut subscription = handle
            .block_on(async { client.account_summary(&group, tags).await })
            .map_err(|e| BrokerError::ConnectionLost(format!("account_summary failed: {e}")))?;

        let mut net_liq = Decimal::ZERO;
        let mut buying_power = Decimal::ZERO;
        let mut total_cash = Decimal::ZERO;
        let mut settled_cash = Decimal::ZERO;

        loop {
            let update = handle.block_on(async { subscription.next().await });
            match update {
                Some(Ok(AccountSummaryResult::Summary(summary))) => {
                    let val: Decimal = summary
                        .value
                        .parse()
                        .unwrap_or(Decimal::ZERO);
                    match summary.tag.as_str() {
                        "NetLiquidation" => net_liq = val,
                        "BuyingPower" => buying_power = val,
                        "TotalCashValue" => total_cash = val,
                        "SettledCash" => settled_cash = val,
                        _ => {}
                    }
                }
                Some(Ok(AccountSummaryResult::End)) => break,
                Some(Err(e)) => {
                    tracing::warn!(error = %e, "Error reading account summary");
                    break;
                }
                None => break,
            }
        }

        Ok(AccountSnapshot {
            broker_id: BrokerId::Ibkr,
            net_liquidation: net_liq,
            buying_power,
            cash: CashBalance {
                settled: settled_cash,
                unsettled: total_cash - settled_cash,
                pending_settlement: vec![],
            },
            timestamp: Utc::now(),
        })
    }

    fn settled_cash_delta(&self, _date: NaiveDate) -> Result<Decimal, BrokerError> {
        Err(BrokerError::Unsupported(
            "settled_cash_delta not yet implemented".into(),
        ))
    }

    fn broker_id(&self) -> BrokerId {
        BrokerId::Ibkr
    }

    fn supports_short(&self, _symbol: &Symbol) -> Result<ShortAvailability, BrokerError> {
        // Phase 0: Hatch tier, no shorts
        Ok(ShortAvailability::Unavailable)
    }

    fn locate_shares(&self, _symbol: &Symbol, _qty: u64) -> Result<LocateResult, BrokerError> {
        Err(BrokerError::Unsupported(
            "Locate shares not available in Hatch tier".into(),
        ))
    }
}

#[async_trait]
impl BrokerStreams for IbkrBroker {
    async fn subscribe_quotes(
        &self,
        symbols: &[Symbol],
        tx: Sender<QuoteEvent>,
    ) -> Result<StreamHandle, BrokerError> {
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();

        let client = Arc::clone(&self.client);
        let symbols = symbols.to_vec();

        let join = tokio::spawn(async move {
            let cancel_rx = cancel_rx;

            for symbol in &symbols {
                let contract = Contract::stock(symbol.to_string()).build();

                let subscription = client
                    .realtime_bars(
                        &contract,
                        BarSize::Sec5,
                        WhatToShow::Trades,
                        TradingHours::Extended,
                    )
                    .await;

                let mut subscription = match subscription {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(symbol = %symbol, error = %e, "Failed to subscribe to IBKR bars");
                        continue;
                    }
                };

                let tx = tx.clone();
                let sym = symbol.clone();

                tokio::spawn(async move {
                    let mut consecutive_errors = 0u32;
                    while let Some(bar_result) = subscription.next().await {
                        match bar_result {
                            Ok(bar) => {
                                consecutive_errors = 0;
                                let quote = QuoteEvent {
                                    symbol: sym.clone(),
                                    bid: Decimal::ZERO, // realtime_bars don't have bid/ask
                                    ask: Decimal::ZERO,
                                    last: Decimal::from_f64_retain(bar.close)
                                        .unwrap_or(Decimal::ZERO),
                                    volume: bar.volume as u64,
                                    timestamp: Utc::now(),
                                    prev_close: None,
                                    day_open: None,
                                    day_high: None,
                                    day_low: None,
                                    avg_volume: None,
                                };
                                if tx.send(quote).await.is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                consecutive_errors += 1;
                                if consecutive_errors <= 3 {
                                    tracing::warn!(symbol = %sym, error = %e, "IBKR bar error");
                                } else if consecutive_errors == 4 {
                                    tracing::warn!(symbol = %sym, "IBKR bar errors suppressed — persistent failure");
                                }
                            }
                        }
                    }
                });
            }

            // Keep alive until cancelled
            let _ = cancel_rx.await;
        });

        Ok(StreamHandle {
            _cancel: cancel_tx,
            join,
        })
    }

    async fn subscribe_fills(
        &self,
        tx: Sender<FillEvent>,
    ) -> Result<StreamHandle, BrokerError> {
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
        let client = Arc::clone(&self.client);
        let order_map = self.order_map.lock().unwrap().clone();

        // Build a reverse map: ib_order_id -> TALON OrderId
        let reverse_map: HashMap<i32, OrderId> = order_map
            .iter()
            .map(|(talon_id, &ib_id)| (ib_id, *talon_id))
            .collect();

        let join = tokio::spawn(async move {
            let mut cancel_rx = cancel_rx;

            let filter = ibapi::orders::ExecutionFilter::default();
            let subscription = client.executions(filter).await;

            let mut subscription = match subscription {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to subscribe to IBKR executions");
                    let _ = cancel_rx.await;
                    return;
                }
            };

            loop {
                tokio::select! {
                    exec = subscription.next() => {
                        match exec {
                            Some(Ok(ibapi::orders::Executions::ExecutionData(data))) => {
                                let ib_order_id = data.execution.order_id;
                                let talon_order_id = reverse_map
                                    .get(&ib_order_id)
                                    .cloned()
                                    .unwrap_or_else(OrderId::new);

                                let fill = FillEvent {
                                    order_id: talon_order_id,
                                    symbol: Symbol::new(data.contract.symbol.as_str()),
                                    qty: data.execution.shares as i64,
                                    price: Decimal::from_f64_retain(data.execution.price)
                                        .unwrap_or(Decimal::ZERO),
                                    commission: Decimal::ZERO, // commission comes separately
                                    timestamp: Utc::now(),
                                    broker_id: BrokerId::Ibkr,
                                };
                                if tx.send(fill).await.is_err() {
                                    break;
                                }
                            }
                            Some(Ok(_)) => {} // ExecutionEnd, CommissionReport, etc.
                            Some(Err(e)) => {
                                tracing::warn!(error = %e, "IBKR execution error");
                            }
                            None => break,
                        }
                    }
                    _ = &mut cancel_rx => break,
                }
            }
        });

        Ok(StreamHandle {
            _cancel: cancel_tx,
            join,
        })
    }

    async fn subscribe_margin_events(
        &self,
        tx: Sender<MarginEvent>,
    ) -> Result<StreamHandle, BrokerError> {
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
        let client = Arc::clone(&self.client);
        let acct = ibapi::accounts::types::AccountId(self.account_id());

        let join = tokio::spawn(async move {
            let mut cancel_rx = cancel_rx;

            let subscription = client.account_updates(&acct).await;

            let mut subscription = match subscription {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to subscribe to IBKR account updates");
                    let _ = cancel_rx.await;
                    return;
                }
            };

            loop {
                tokio::select! {
                    update = subscription.next() => {
                        match update {
                            Some(Ok(ibapi::accounts::AccountUpdate::AccountValue(val))) => {
                                // Filter for margin-related keys
                                match val.key.as_str() {
                                    "MaintMarginReq" | "ExcessLiquidity" => {
                                        // Try to parse maintenance margin and excess liquidity
                                        let amount: Decimal = val.value.parse().unwrap_or(Decimal::ZERO);
                                        if val.key == "ExcessLiquidity" {
                                            let margin_event = MarginEvent {
                                                maintenance_margin: Decimal::ZERO,
                                                excess_liquidity: amount,
                                                timestamp: Utc::now(),
                                            };
                                            if tx.send(margin_event).await.is_err() {
                                                break;
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            Some(Ok(_)) => {} // Portfolio, Time, End markers
                            Some(Err(e)) => {
                                tracing::warn!(error = %e, "IBKR account update error");
                            }
                            None => break,
                        }
                    }
                    _ = &mut cancel_rx => break,
                }
            }
        });

        Ok(StreamHandle {
            _cancel: cancel_tx,
            join,
        })
    }

    async fn subscribe_tape(
        &self,
        symbol: &Symbol,
        tx: Sender<TapeRaw>,
    ) -> Result<StreamHandle, BrokerError> {
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
        let client = Arc::clone(&self.client);
        let contract = Contract::stock(symbol.to_string()).build();
        let sym_label = symbol.to_string();

        let join = tokio::spawn(async move {
            let mut cancel_rx = cancel_rx;

            let mut subscription = match client
                .tick_by_tick_all_last(&contract, 0, false)
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(symbol = %sym_label, error = %e, "tick_by_tick_all_last failed");
                    let _ = cancel_rx.await;
                    return;
                }
            };

            loop {
                tokio::select! {
                    tick = subscription.next() => {
                        match tick {
                            Some(Ok(trade)) => {
                                let ts = chrono::TimeZone::timestamp_opt(
                                    &Utc,
                                    trade.time.unix_timestamp(),
                                    trade.time.nanosecond(),
                                )
                                .single()
                                .unwrap_or_else(Utc::now);

                                let raw = TapeRaw {
                                    time: ts,
                                    price: trade.price,
                                    size: trade.size,
                                    exchange: trade.exchange,
                                    conditions: trade.special_conditions,
                                };
                                if tx.send(raw).await.is_err() {
                                    break;
                                }
                            }
                            Some(Err(e)) => {
                                tracing::warn!(symbol = %sym_label, error = %e, "tape tick error");
                            }
                            None => break,
                        }
                    }
                    _ = &mut cancel_rx => break,
                }
            }
        });

        Ok(StreamHandle {
            _cancel: cancel_tx,
            join,
        })
    }

    async fn subscribe_depth(
        &self,
        symbol: &Symbol,
        rows: usize,
        tx: Sender<DepthRaw>,
    ) -> Result<StreamHandle, BrokerError> {
        let (cancel_tx, cancel_rx) = tokio::sync::oneshot::channel::<()>();
        let client = Arc::clone(&self.client);
        let contract = Contract::stock(symbol.to_string()).build();
        let sym_label = symbol.to_string();

        let join = tokio::spawn(async move {
            let mut cancel_rx = cancel_rx;

            let mut subscription = match client
                .market_depth(&contract, rows as i32, false)
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(symbol = %sym_label, error = %e, "market_depth failed");
                    let _ = cancel_rx.await;
                    return;
                }
            };

            loop {
                tokio::select! {
                    update = subscription.next() => {
                        let raw = match update {
                            Some(Ok(MarketDepths::MarketDepth(d))) => DepthRaw {
                                position: d.position as usize,
                                operation: match d.operation {
                                    0 => DepthOp::Insert,
                                    1 => DepthOp::Update,
                                    _ => DepthOp::Delete,
                                },
                                side: if d.side == 1 { DepthSide::Bid } else { DepthSide::Ask },
                                price: d.price,
                                size: d.size,
                                market_maker: None,
                            },
                            Some(Ok(MarketDepths::MarketDepthL2(d))) => DepthRaw {
                                position: d.position as usize,
                                operation: match d.operation {
                                    0 => DepthOp::Insert,
                                    1 => DepthOp::Update,
                                    _ => DepthOp::Delete,
                                },
                                side: if d.side == 1 { DepthSide::Bid } else { DepthSide::Ask },
                                price: d.price,
                                size: d.size,
                                market_maker: Some(d.market_maker),
                            },
                            Some(Ok(MarketDepths::Notice(n))) => {
                                tracing::info!(code = n.code, msg = %n.message, "depth notice");
                                continue;
                            }
                            Some(Err(e)) => {
                                tracing::warn!(symbol = %sym_label, error = %e, "depth error");
                                continue;
                            }
                            None => break,
                        };
                        if tx.send(raw).await.is_err() {
                            break;
                        }
                    }
                    _ = &mut cancel_rx => break,
                }
            }
        });

        Ok(StreamHandle {
            _cancel: cancel_tx,
            join,
        })
    }
}
