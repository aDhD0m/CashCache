//! FlowManager — manages L2 subscription lifecycle and aggregates data
//! for the Flow tab.
//!
//! Owns the DOM book, tape ring buffer, volume profile, and delta sparkline.
//! Receives commands from the TUI via `mpsc<FlowCmd>` and publishes
//! `FlowSnapshot` into `AppState` via the `watch::Sender`.

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use chrono::{NaiveDate, Utc};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;
use tokio::sync::{mpsc, watch};

use talon_broker::ibkr::IbkrBroker;
use talon_broker::traits::BrokerStreams;
use talon_types::broker::StreamHandle;
use talon_types::channel::AppState;
use talon_types::flow::*;
use talon_types::position::Symbol;

pub struct FlowManager {
    broker: Arc<IbkrBroker>,
    cmd_rx: mpsc::Receiver<FlowCmd>,
    state_tx: watch::Sender<AppState>,

    // Subscription state
    active_symbol: Option<Symbol>,
    tape_handle: Option<StreamHandle>,
    depth_handle: Option<StreamHandle>,
    tape_rx: Option<mpsc::Receiver<TapeRaw>>,
    depth_rx: Option<mpsc::Receiver<DepthRaw>>,

    // Aggregated data
    book: OrderBook,
    tape: VecDeque<TapeEntry>,
    volume_profile: BTreeMap<Decimal, VolumeBucket>,
    delta_buckets: VecDeque<DeltaBucket>,
    bucket_buy: Decimal,
    bucket_sell: Decimal,
    cumulative_delta: Decimal,
    session_date: Option<NaiveDate>,
}

impl FlowManager {
    pub fn new(
        broker: Arc<IbkrBroker>,
        cmd_rx: mpsc::Receiver<FlowCmd>,
        state_tx: watch::Sender<AppState>,
    ) -> Self {
        Self {
            broker,
            cmd_rx,
            state_tx,
            active_symbol: None,
            tape_handle: None,
            depth_handle: None,
            tape_rx: None,
            depth_rx: None,
            book: OrderBook::default(),
            tape: VecDeque::with_capacity(TAPE_CAP),
            volume_profile: BTreeMap::new(),
            delta_buckets: VecDeque::with_capacity(DELTA_BUCKETS),
            bucket_buy: Decimal::ZERO,
            bucket_sell: Decimal::ZERO,
            cumulative_delta: Decimal::ZERO,
            session_date: None,
        }
    }

    pub async fn run(mut self) {
        let mut delta_ticker = tokio::time::interval(Duration::from_secs(60));

        loop {
            tokio::select! {
                cmd = self.cmd_rx.recv() => {
                    match cmd {
                        Some(FlowCmd::SelectSymbol(sym)) => {
                            self.subscribe_to(sym).await;
                        }
                        Some(FlowCmd::ClearSymbol) => {
                            self.unsubscribe();
                            self.publish_snapshot();
                        }
                        Some(FlowCmd::ResetAccumulators) => {
                            self.reset_accumulators();
                            self.publish_snapshot();
                        }
                        None => break,
                    }
                }

                raw = recv_opt(&mut self.tape_rx) => {
                    if let Some(raw) = raw {
                        self.check_session_reset();
                        self.process_tape(raw);
                        self.publish_snapshot();
                    }
                }

                raw = recv_opt(&mut self.depth_rx) => {
                    if let Some(raw) = raw {
                        self.process_depth(raw);
                        self.publish_snapshot();
                    }
                }

                _ = delta_ticker.tick() => {
                    if self.active_symbol.is_some() {
                        self.rotate_delta_bucket();
                        self.publish_snapshot();
                    }
                }
            }
        }

        tracing::info!("FlowManager shutting down");
    }

    // -----------------------------------------------------------------------
    // Subscription lifecycle
    // -----------------------------------------------------------------------

    async fn subscribe_to(&mut self, symbol: Symbol) {
        // If same symbol, don't re-subscribe.
        if self.active_symbol.as_ref() == Some(&symbol) {
            return;
        }

        // Drop existing subscriptions (cancel via StreamHandle drop).
        self.tape_handle = None;
        self.depth_handle = None;
        self.tape_rx = None;
        self.depth_rx = None;

        // Clear stale data from previous symbol.
        self.reset_accumulators();
        self.active_symbol = Some(symbol.clone());

        // Subscribe to tape (T&S).
        let (tape_tx, tape_rx) = mpsc::channel(1024);
        match self.broker.subscribe_tape(&symbol, tape_tx).await {
            Ok(handle) => {
                self.tape_handle = Some(handle);
                self.tape_rx = Some(tape_rx);
            }
            Err(e) => {
                tracing::warn!(symbol = %symbol, error = %e, "tape subscription failed");
                self.state_tx.send_modify(|s| {
                    s.flow.error = Some(format!("Tape: {e}"));
                });
            }
        }

        // Subscribe to depth (DOM).
        let (depth_tx, depth_rx) = mpsc::channel(1024);
        match self.broker.subscribe_depth(&symbol, DOM_DEPTH, depth_tx).await {
            Ok(handle) => {
                self.depth_handle = Some(handle);
                self.depth_rx = Some(depth_rx);
            }
            Err(e) => {
                tracing::warn!(symbol = %symbol, error = %e, "depth subscription failed");
                self.state_tx.send_modify(|s| {
                    s.flow.error = Some(format!("Depth: {e}"));
                });
            }
        }

        tracing::info!(symbol = %symbol, "Flow subscriptions active");
        self.publish_snapshot();
    }

    fn unsubscribe(&mut self) {
        self.tape_handle = None;
        self.depth_handle = None;
        self.tape_rx = None;
        self.depth_rx = None;
        self.active_symbol = None;
        self.reset_accumulators();
    }

    fn reset_accumulators(&mut self) {
        self.book = OrderBook::default();
        self.tape.clear();
        self.volume_profile.clear();
        self.delta_buckets.clear();
        self.bucket_buy = Decimal::ZERO;
        self.bucket_sell = Decimal::ZERO;
        self.cumulative_delta = Decimal::ZERO;
    }

    // -----------------------------------------------------------------------
    // Tape processing
    // -----------------------------------------------------------------------

    fn process_tape(&mut self, raw: TapeRaw) {
        let price = match Decimal::from_f64(raw.price) {
            Some(p) => p,
            None => {
                tracing::warn!(price = raw.price, "tape: invalid f64 price");
                return;
            }
        };
        let size = Decimal::from_f64(raw.size).unwrap_or(Decimal::ZERO);

        // Classify trade side via Lee-Ready: compare to current DOM.
        let side = match (self.book.best_bid(), self.book.best_ask()) {
            (Some(bid), Some(ask)) => {
                if price >= ask {
                    TradeSide::Buy
                } else if price <= bid {
                    TradeSide::Sell
                } else {
                    TradeSide::Unknown
                }
            }
            _ => TradeSide::Unknown,
        };

        let entry = TapeEntry {
            time: raw.time,
            price,
            size,
            side,
            exchange: raw.exchange,
            conditions: raw.conditions,
        };

        // Ring buffer: newest at front.
        self.tape.push_front(entry);
        if self.tape.len() > TAPE_CAP {
            self.tape.pop_back();
        }

        // Volume profile: bucket by cent.
        let bucket_price = round_to_cent(price);
        let bucket = self.volume_profile.entry(bucket_price).or_insert(VolumeBucket {
            price: bucket_price,
            buy_volume: Decimal::ZERO,
            sell_volume: Decimal::ZERO,
        });
        match side {
            TradeSide::Buy => bucket.buy_volume += size,
            TradeSide::Sell => bucket.sell_volume += size,
            TradeSide::Unknown => {
                // Split evenly for unknown classification.
                let half = size / Decimal::TWO;
                bucket.buy_volume += half;
                bucket.sell_volume += half;
            }
        }

        // Delta accumulator.
        match side {
            TradeSide::Buy => {
                self.bucket_buy += size;
                self.cumulative_delta += size;
            }
            TradeSide::Sell => {
                self.bucket_sell += size;
                self.cumulative_delta -= size;
            }
            TradeSide::Unknown => {}
        }
    }

    // -----------------------------------------------------------------------
    // Depth processing
    // -----------------------------------------------------------------------

    fn process_depth(&mut self, raw: DepthRaw) {
        let price = match Decimal::from_f64(raw.price) {
            Some(p) => p,
            None => return,
        };
        let size = Decimal::from_f64(raw.size).unwrap_or(Decimal::ZERO);

        let levels = match raw.side {
            DepthSide::Bid => &mut self.book.bids,
            DepthSide::Ask => &mut self.book.asks,
        };

        let level = DomLevel {
            price,
            size,
            market_maker: raw.market_maker,
        };

        match raw.operation {
            DepthOp::Insert => {
                if raw.position <= levels.len() {
                    levels.insert(raw.position, level);
                } else {
                    levels.push(level);
                }
            }
            DepthOp::Update => {
                if raw.position < levels.len() {
                    levels[raw.position] = level;
                }
            }
            DepthOp::Delete => {
                if raw.position < levels.len() {
                    levels.remove(raw.position);
                }
            }
        }

        // Re-sort and truncate.
        self.book.bids.sort_by(|a, b| b.price.cmp(&a.price)); // descending
        self.book.asks.sort_by(|a, b| a.price.cmp(&b.price)); // ascending
        self.book.bids.truncate(DOM_DEPTH);
        self.book.asks.truncate(DOM_DEPTH);
    }

    // -----------------------------------------------------------------------
    // Delta bucket rotation
    // -----------------------------------------------------------------------

    fn rotate_delta_bucket(&mut self) {
        let delta = self.bucket_buy - self.bucket_sell;
        self.delta_buckets.push_front(DeltaBucket {
            time: Utc::now(),
            delta,
        });
        if self.delta_buckets.len() > DELTA_BUCKETS {
            self.delta_buckets.pop_back();
        }
        self.bucket_buy = Decimal::ZERO;
        self.bucket_sell = Decimal::ZERO;
    }

    // -----------------------------------------------------------------------
    // Session reset (new trading day)
    // -----------------------------------------------------------------------

    fn check_session_reset(&mut self) {
        let today = Utc::now().date_naive();
        if self.session_date.is_some() && self.session_date != Some(today) {
            tracing::info!("New trading day — resetting Flow accumulators");
            self.reset_accumulators();
        }
        self.session_date = Some(today);
    }

    // -----------------------------------------------------------------------
    // Publish to AppState
    // -----------------------------------------------------------------------

    fn publish_snapshot(&self) {
        let snapshot = FlowSnapshot {
            symbol: self.active_symbol.clone(),
            book: self.book.clone(),
            tape: Vec::from(self.tape.clone()),
            volume_profile: self.volume_profile.values().cloned().collect(),
            delta_sparkline: Vec::from(self.delta_buckets.clone()),
            cumulative_delta: self.cumulative_delta,
            is_live: self.tape_handle.is_some(),
            error: None,
        };

        self.state_tx.send_modify(|s| {
            s.flow = snapshot;
        });
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Receive from an `Option<mpsc::Receiver<T>>`.
/// Returns `pending` when the receiver is `None`.
async fn recv_opt<T>(rx: &mut Option<mpsc::Receiver<T>>) -> Option<T> {
    match rx {
        Some(rx) => rx.recv().await,
        None => {
            // No active receiver — park forever (won't wake select).
            std::future::pending().await
        }
    }
}

/// Round a Decimal price to the nearest cent.
fn round_to_cent(price: Decimal) -> Decimal {
    price.round_dp(2)
}
