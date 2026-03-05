use std::time::Duration;

use chrono::Utc;
use rust_decimal::Decimal;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;
use tracing;
use yahoo_finance_api as yahoo;

use talon_types::broker::QuoteEvent;
use talon_types::position::Symbol;

// ---------------------------------------------------------------------------
// Yahoo Finance client — free fallback data source
// ---------------------------------------------------------------------------

pub struct YahooClient {
    connector: yahoo::YahooConnector,
}

#[derive(Debug, thiserror::Error)]
pub enum YahooError {
    #[error("Yahoo API error: {0}")]
    Api(String),
}

impl YahooClient {
    pub fn new() -> Result<Self, YahooError> {
        let connector =
            yahoo::YahooConnector::new().map_err(|e| YahooError::Api(e.to_string()))?;
        Ok(Self { connector })
    }

    /// Get the latest quote for a symbol.
    pub async fn latest_quote(&self, symbol: &Symbol) -> Result<QuoteEvent, YahooError> {
        let resp = self
            .connector
            .get_latest_quotes(&symbol.to_string(), "1d")
            .await
            .map_err(|e| YahooError::Api(e.to_string()))?;

        let quote = resp
            .last_quote()
            .map_err(|e| YahooError::Api(e.to_string()))?;

        Ok(QuoteEvent {
            symbol: symbol.clone(),
            bid: Decimal::ZERO, // Yahoo doesn't provide bid/ask in this endpoint
            ask: Decimal::ZERO,
            last: Decimal::from_f64_retain(quote.close).unwrap_or(Decimal::ZERO),
            volume: quote.volume,
            timestamp: Utc::now(),
            prev_close: None,
            day_open: None,
            day_high: None,
            day_low: None,
            avg_volume: None,
        })
    }

    /// Start a fallback polling loop (lower frequency than Polygon).
    pub fn start_fallback_polling(
        self,
        symbols: Vec<Symbol>,
        quote_tx: broadcast::Sender<QuoteEvent>,
        interval: Duration,
    ) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                for sym in &symbols {
                    match self.latest_quote(sym).await {
                        Ok(quote) => {
                            let _ = quote_tx.send(quote);
                        }
                        Err(e) => {
                            tracing::warn!(symbol = %sym, error = %e, "Yahoo quote failed");
                        }
                    }
                }
                tokio::time::sleep(interval).await;
            }
        })
    }
}
