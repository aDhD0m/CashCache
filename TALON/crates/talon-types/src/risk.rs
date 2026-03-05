use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Risk parameters per tier (S7.2)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierRiskParams {
    pub max_single_position_risk_pct: Decimal,
    pub max_total_exposure_pct: Decimal,
    pub max_concurrent_positions: u32,
    pub drawdown_circuit_breaker_pct: Decimal,
    pub daily_loss_limit_pct: Decimal,
}

// ---------------------------------------------------------------------------
// Stress multiplier tiers (S7.3)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StressTier {
    /// 0-3% drawdown. Normal operation.
    Normal,
    /// 3-5% drawdown. All limits reduced 25%.
    Tier1,
    /// 5-8% drawdown. All limits reduced 50%.
    Tier2,
    /// 8% to circuit breaker. FLAMEOUT.
    Flameout,
    /// Beyond circuit breaker. NOSEDIVE.
    Nosedive,
}

impl StressTier {
    pub fn multiplier(&self) -> Decimal {
        match self {
            Self::Normal => Decimal::ONE,
            Self::Tier1 => Decimal::new(75, 2),   // 0.75
            Self::Tier2 => Decimal::new(50, 2),   // 0.50
            Self::Flameout => Decimal::new(25, 2), // 0.25
            Self::Nosedive => Decimal::ZERO,
        }
    }

    pub fn from_drawdown_pct(dd: Decimal, params: &StressParams) -> Self {
        let dd_abs = dd.abs();
        if dd_abs >= params.circuit_breaker_pct {
            Self::Nosedive
        } else if dd_abs >= params.tier_3_threshold_pct {
            Self::Flameout
        } else if dd_abs >= params.tier_2_threshold_pct {
            Self::Tier2
        } else if dd_abs >= params.tier_0_threshold_pct {
            Self::Tier1
        } else {
            Self::Normal
        }
    }
}

impl std::fmt::Display for StressTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "x1.0"),
            Self::Tier1 => write!(f, "x0.75"),
            Self::Tier2 => write!(f, "x0.50"),
            Self::Flameout => write!(f, "FLAMEOUT x0.25"),
            Self::Nosedive => write!(f, "NOSEDIVE x0.0"),
        }
    }
}

// ---------------------------------------------------------------------------
// Stress config params
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StressParams {
    pub tier_0_threshold_pct: Decimal,
    pub tier_2_threshold_pct: Decimal,
    pub tier_3_threshold_pct: Decimal,
    pub circuit_breaker_pct: Decimal,
    pub override_cooldown_mins: u32,
}

// ---------------------------------------------------------------------------
// Risk mesh decision
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum RiskDecision {
    Approved,
    Rejected { reason: String },
    ReducedSize { original: u64, approved: u64, reason: String },
}

// ---------------------------------------------------------------------------
// Intelligence ports (S10)
// ---------------------------------------------------------------------------

pub trait IntelligencePort<S: Send + Sync>: Send + Sync {
    fn latest(&self) -> Option<&S>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RegimeState {
    Trending,
    Reverting,
    Crisis,
    LowLiquidity,
    Standalone,
}

impl std::fmt::Display for RegimeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trending => write!(f, "TRENDING"),
            Self::Reverting => write!(f, "REVERTING"),
            Self::Crisis => write!(f, "CRISIS"),
            Self::LowLiquidity => write!(f, "LOW LIQ"),
            Self::Standalone => write!(f, "STANDALONE"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SentimentState {
    Bullish,
    Bearish,
    Neutral,
}

// ---------------------------------------------------------------------------
// Flameout config (S7.4)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlameoutConfig {
    pub trigger_multiplier: Decimal,
    pub profitable_action: FlameoutAction,
    pub losing_action: FlameoutAction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FlameoutAction {
    TrailToBreakeven,
    TightenStop50Pct,
}

// ---------------------------------------------------------------------------
// Forced cover config (S7.7)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForcedCoverConfig {
    pub trigger_pct: Decimal,
    pub retry_delay_secs: u32,
    pub halt_escalation_mins: u32,
}

// ---------------------------------------------------------------------------
// Module risk allocation (S7.6)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ModuleRiskAllocation {
    pub allocations: std::collections::HashMap<crate::module::ModuleId, Decimal>,
}

impl ModuleRiskAllocation {
    pub fn get(&self, module: &crate::module::ModuleId) -> Decimal {
        self.allocations
            .get(module)
            .copied()
            .unwrap_or(Decimal::ZERO)
    }
}
