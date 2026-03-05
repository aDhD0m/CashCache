use serde::Deserialize;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// System config (system.toml)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SystemConfig {
    pub system: SystemSection,
    pub tiers: TiersSection,
    pub regulatory: RegulatorySection,
    pub observability: ObservabilitySection,
    pub watchdog: WatchdogSection,
}

#[derive(Debug, Deserialize)]
pub struct SystemSection {
    pub name: String,
    pub version: String,
    pub log_level: String,
    pub paths: PathsSection,
    pub modes: ModesSection,
}

#[derive(Debug, Deserialize)]
pub struct PathsSection {
    pub config_dir: String,
    pub data_dir: String,
    pub backup_dir: String,
    pub scripts_dir: String,
}

#[derive(Debug, Deserialize)]
pub struct ModesSection {
    pub cruising_altitude_eligible_modules: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct TiersSection {
    pub hatch: TierConfig,
    pub takeoff: TierConfig,
    pub turbo: TierConfig,
}

#[derive(Debug, Deserialize)]
pub struct TierConfig {
    pub name: String,
    pub min_capital: u64,
    #[serde(default)]
    pub max_capital: Option<u64>,
    pub account_type: String,
    #[serde(default)]
    pub margin_upgrade_threshold: Option<u64>,
    pub supervision: String,
    pub primary_broker: String,
}

#[derive(Debug, Deserialize)]
pub struct RegulatorySection {
    pub pdt_rule_active: bool,
    pub min_day_trading_equity: u64,
    pub margin_framework: String,
}

#[derive(Debug, Deserialize)]
pub struct ObservabilitySection {
    pub tracing_framework: String,
    pub log_file: String,
    pub log_rotation: String,
    pub span_correlation: bool,
}

#[derive(Debug, Deserialize)]
pub struct WatchdogSection {
    pub notify_interval_secs: u32,
    pub watchdog_sec: u32,
    pub panic_script: String,
}

// ---------------------------------------------------------------------------
// Risk config (risk.toml)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RiskConfig {
    pub risk: RiskSection,
}

#[derive(Debug, Deserialize)]
pub struct RiskSection {
    pub hatch: TierRiskConfig,
    pub takeoff: TierRiskConfig,
    pub turbo: TierRiskConfig,
    pub stress: StressConfig,
    pub flameout: FlameoutConfig,
    pub forced_cover: ForcedCoverConfig,
    pub timeout_defaults: TimeoutDefaultsConfig,
    pub module_allocation: HashMap<String, f64>,
}

#[derive(Debug, Deserialize)]
pub struct TierRiskConfig {
    pub max_single_position_risk_pct: f64,
    pub max_total_exposure_pct: f64,
    pub max_concurrent_positions: u32,
    pub drawdown_circuit_breaker_pct: f64,
    pub daily_loss_limit_pct: f64,
}

#[derive(Debug, Deserialize)]
pub struct StressConfig {
    pub tier_0_threshold_pct: f64,
    pub tier_1_multiplier: f64,
    pub tier_2_threshold_pct: f64,
    pub tier_2_multiplier: f64,
    pub tier_3_threshold_pct: f64,
    pub tier_3_multiplier: f64,
    pub override_cooldown_mins: u32,
}

#[derive(Debug, Deserialize)]
pub struct FlameoutConfig {
    pub trigger_multiplier: f64,
    pub profitable_positions: String,
    pub losing_positions: String,
}

#[derive(Debug, Deserialize)]
pub struct ForcedCoverConfig {
    pub trigger_pct: f64,
    pub order_type: String,
    pub retry_delay_secs: u32,
    pub halt_escalation_mins: u32,
}

#[derive(Debug, Deserialize)]
pub struct TimeoutDefaultsConfig {
    pub new_position_entry: String,
    pub stop_loss_exit: String,
    pub forced_cover: String,
    pub zero_dte_hard_close: String,
    pub cashcache_harvest: String,
}

// ---------------------------------------------------------------------------
// Graduation config (graduation.toml)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct GraduationConfig {
    pub graduation: GraduationSection,
}

#[derive(Debug, Deserialize)]
pub struct GraduationSection {
    #[serde(default)]
    pub gates: Vec<GateConfig>,
}

#[derive(Debug, Deserialize)]
pub struct GateConfig {
    pub from: String,
    pub to: String,
    pub min_capital: u64,
    pub account_type: String,
    #[serde(default)]
    pub min_trades: Option<u32>,
    #[serde(default)]
    pub min_win_rate: Option<f64>,
    #[serde(default)]
    pub max_drawdown: Option<f64>,
}

// ---------------------------------------------------------------------------
// Module config (per-module TOML)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ModuleConfig {
    pub module: ModuleSection,
}

#[derive(Debug, Deserialize)]
pub struct ModuleSection {
    pub name: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub params: HashMap<String, toml::Value>,
}

// ---------------------------------------------------------------------------
// Config loader
// ---------------------------------------------------------------------------

impl SystemConfig {
    pub fn load(path: &str) -> Result<Self, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;
        toml::from_str(&content).map_err(|e| format!("failed to parse {path}: {e}"))
    }
}

impl RiskConfig {
    pub fn load(path: &str) -> Result<Self, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("failed to read {path}: {e}"))?;
        toml::from_str(&content).map_err(|e| format!("failed to parse {path}: {e}"))
    }
}
