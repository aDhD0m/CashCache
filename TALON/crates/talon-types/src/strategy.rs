use async_trait::async_trait;

use crate::broker::QuoteEvent;
use crate::channel::ApproachingSetup;
use crate::module::{ModuleId, ModuleState};
use crate::order::OrderIntent;

// ---------------------------------------------------------------------------
// ScanResult — what modules return from on_quote/scan
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct ScanResult {
    /// Order intents ready for ExecCore evaluation.
    pub intents: Vec<OrderIntent>,
    /// Stocks approaching trigger thresholds ("almost orders").
    pub approaching: Vec<ApproachingSetup>,
}

// ---------------------------------------------------------------------------
// TradingModule trait — scan/evaluate/idle lifecycle
// ---------------------------------------------------------------------------

#[async_trait]
pub trait TradingModule: Send + Sync {
    fn id(&self) -> ModuleId;
    fn state(&self) -> ModuleState;

    /// Process a market data update. Returns intents + approaching setups.
    async fn on_quote(&mut self, quote: &QuoteEvent) -> ScanResult;

    /// Periodic scan for new opportunities.
    async fn scan(&mut self) -> ScanResult;

    /// Transition to idle state.
    fn go_idle(&mut self);

    /// Transition to scanning state.
    fn go_scanning(&mut self);

    /// Pause the module (demotion, nosedive, etc.)
    fn pause(&mut self);

    /// Number of signals generated since startup.
    fn signals_generated(&self) -> u32;
}
