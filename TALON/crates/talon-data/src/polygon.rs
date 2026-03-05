use std::time::Duration;

use chrono::{DateTime, NaiveDate, Utc};
use reqwest::Client as HttpClient;
use rust_decimal::Decimal;
use serde::Deserialize;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing;

use talon_types::broker::{CandleBar, QuoteEvent, Timeframe};
use talon_types::position::Symbol;

// ---------------------------------------------------------------------------
// Polygon.io REST client — supplementary market data
// ---------------------------------------------------------------------------

pub struct PolygonClient {
    http: HttpClient,
    api_key: String,
}

#[derive(Debug, thiserror::Error)]
pub enum PolygonError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("API error: {status} — {message}")]
    Api { status: String, message: String },
}

// --- Polygon JSON response shapes ---

#[derive(Debug, Deserialize)]
struct SnapshotResponse {
    ticker: Option<TickerSnapshot>,
}

#[derive(Debug, Deserialize)]
struct TickerSnapshot {
    day: Option<DaySnapshot>,
    #[serde(rename = "prevDay")]
    prev_day: Option<PrevDaySnapshot>,
    #[serde(rename = "lastTrade")]
    last_trade: Option<LastTrade>,
    #[serde(rename = "lastQuote")]
    last_quote: Option<LastQuote>,
}

#[derive(Debug, Deserialize)]
struct DaySnapshot {
    o: Option<f64>,
    h: Option<f64>,
    l: Option<f64>,
    v: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct PrevDaySnapshot {
    c: Option<f64>,
    v: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct LastTrade {
    p: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct LastQuote {
    #[serde(rename = "P")]
    ask_price: Option<f64>,
    p: Option<f64>, // bid price
}

impl PolygonClient {
    pub fn new(api_key: String) -> Self {
        Self {
            http: HttpClient::new(),
            api_key,
        }
    }

    /// Fetch a real-time snapshot for a single ticker.
    pub async fn snapshot(&self, symbol: &Symbol) -> Result<QuoteEvent, PolygonError> {
        let url = format!(
            "https://api.polygon.io/v2/snapshot/locale/us/markets/stocks/tickers/{}?apiKey={}",
            symbol, self.api_key
        );

        let resp: SnapshotResponse = self.http.get(&url).send().await?.json().await?;

        let ticker = resp
            .ticker
            .ok_or_else(|| PolygonError::Parse("missing ticker in response".into()))?;

        let last = ticker
            .last_trade
            .and_then(|t| t.p)
            .unwrap_or(0.0);
        let bid = ticker
            .last_quote
            .as_ref()
            .and_then(|q| q.p)
            .unwrap_or(0.0);
        let ask = ticker
            .last_quote
            .as_ref()
            .and_then(|q| q.ask_price)
            .unwrap_or(0.0);
        let volume = ticker
            .day
            .as_ref()
            .and_then(|d| d.v)
            .unwrap_or(0.0) as u64;

        let prev_close = ticker
            .prev_day
            .as_ref()
            .and_then(|pd| pd.c)
            .and_then(Decimal::from_f64_retain);
        let day_open = ticker
            .day
            .as_ref()
            .and_then(|d| d.o)
            .and_then(Decimal::from_f64_retain);
        let day_high = ticker
            .day
            .as_ref()
            .and_then(|d| d.h)
            .and_then(Decimal::from_f64_retain);
        let day_low = ticker
            .day
            .as_ref()
            .and_then(|d| d.l)
            .and_then(Decimal::from_f64_retain);
        let avg_volume = ticker
            .prev_day
            .as_ref()
            .and_then(|pd| pd.v)
            .map(|v| v as u64);

        Ok(QuoteEvent {
            symbol: symbol.clone(),
            bid: Decimal::from_f64_retain(bid).unwrap_or(Decimal::ZERO),
            ask: Decimal::from_f64_retain(ask).unwrap_or(Decimal::ZERO),
            last: Decimal::from_f64_retain(last).unwrap_or(Decimal::ZERO),
            volume,
            timestamp: Utc::now(),
            prev_close,
            day_open,
            day_high,
            day_low,
            avg_volume,
        })
    }

    /// Fetch historical OHLCV bars (aggregates) for a symbol.
    pub async fn aggregates(
        &self,
        symbol: &Symbol,
        timeframe: Timeframe,
        from: NaiveDate,
        to: NaiveDate,
    ) -> Result<Vec<CandleBar>, PolygonError> {
        let (multiplier, timespan) = timeframe.polygon_params();
        let url = format!(
            "https://api.polygon.io/v2/aggs/ticker/{}/range/{}/{}/{}/{}?adjusted=true&sort=asc&limit=50000&apiKey={}",
            symbol, multiplier, timespan, from, to, self.api_key
        );

        let resp: AggregatesResponse = self.http.get(&url).send().await?.json().await?;

        let bars = resp.results.unwrap_or_default();
        Ok(bars.into_iter().map(|agg| {
            CandleBar {
                time: DateTime::from_timestamp_millis(agg.t)
                    .unwrap_or(DateTime::UNIX_EPOCH),
                open: dec(agg.o),
                high: dec(agg.h),
                low: dec(agg.l),
                close: dec(agg.c),
                volume: agg.v as u64,
                vwap: agg.vw.map(dec),
                trade_count: agg.n.map(|n| n as u64),
            }
        }).collect())
    }

    /// Start a polling loop that fetches snapshots and sends them on quote_tx.
    pub fn start_polling(
        self,
        symbols: Vec<Symbol>,
        quote_tx: broadcast::Sender<QuoteEvent>,
        interval: Duration,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                for sym in &symbols {
                    match self.snapshot(sym).await {
                        Ok(quote) => {
                            let _ = quote_tx.send(quote);
                        }
                        Err(e) => {
                            tracing::warn!(symbol = %sym, error = %e, "Polygon snapshot failed");
                        }
                    }
                }
                tokio::time::sleep(interval).await;
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Aggregates response shapes
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct AggregatesResponse {
    results: Option<Vec<AggBar>>,
}

#[derive(Debug, Deserialize)]
struct AggBar {
    /// Open
    o: f64,
    /// High
    h: f64,
    /// Low
    l: f64,
    /// Close
    c: f64,
    /// Volume
    v: f64,
    /// VWAP
    vw: Option<f64>,
    /// Unix timestamp (milliseconds)
    t: i64,
    /// Number of trades
    n: Option<f64>,
}

fn dec(v: f64) -> Decimal {
    Decimal::from_f64_retain(v).unwrap_or(Decimal::ZERO)
}
