use rust_decimal::Decimal;

use talon_types::position::Position;
use talon_types::risk::{StressParams, StressTier};

// ---------------------------------------------------------------------------
// StressEngine — manages the stress multiplier state (S7.3)
// ---------------------------------------------------------------------------

pub struct StressEngine {
    params: StressParams,
    current_tier: StressTier,
    peak_equity: Decimal,
    last_override: Option<std::time::Instant>,
}

impl StressEngine {
    pub fn new(params: StressParams, initial_equity: Decimal) -> Self {
        Self {
            params,
            current_tier: StressTier::Normal,
            peak_equity: initial_equity,
            last_override: None,
        }
    }

    pub fn current_tier(&self) -> StressTier {
        self.current_tier
    }

    pub fn multiplier(&self) -> Decimal {
        self.current_tier.multiplier()
    }

    pub fn peak_equity(&self) -> Decimal {
        self.peak_equity
    }

    /// Update with latest equity. Returns Some(new_tier) if tier changed.
    pub fn update(&mut self, current_equity: Decimal) -> Option<StressTier> {
        // Update high-water mark
        if current_equity > self.peak_equity {
            self.peak_equity = current_equity;
        }

        let drawdown_pct = if self.peak_equity.is_zero() {
            Decimal::ZERO
        } else {
            ((self.peak_equity - current_equity) / self.peak_equity) * Decimal::from(100)
        };

        let new_tier = StressTier::from_drawdown_pct(drawdown_pct, &self.params);

        if new_tier != self.current_tier {
            self.current_tier = new_tier;
            Some(new_tier)
        } else {
            None
        }
    }

    /// Operator override: move UP one tier (toward Normal). Returns Ok(new_tier)
    /// or Err if cooldown not elapsed.
    pub fn override_up(&mut self) -> Result<StressTier, StressOverrideError> {
        let cooldown = std::time::Duration::from_secs(self.params.override_cooldown_mins as u64 * 60);

        if let Some(last) = self.last_override
            && last.elapsed() < cooldown {
                let remaining = cooldown - last.elapsed();
                return Err(StressOverrideError::CooldownActive {
                    remaining_secs: remaining.as_secs(),
                });
            }

        let new_tier = match self.current_tier {
            StressTier::Nosedive => StressTier::Flameout,
            StressTier::Flameout => StressTier::Tier2,
            StressTier::Tier2 => StressTier::Tier1,
            StressTier::Tier1 => StressTier::Normal,
            StressTier::Normal => return Err(StressOverrideError::AlreadyNormal),
        };

        self.current_tier = new_tier;
        self.last_override = Some(std::time::Instant::now());
        Ok(new_tier)
    }

    /// Apply stress multiplier to a numeric limit.
    pub fn apply(&self, limit: Decimal) -> Decimal {
        limit * self.multiplier()
    }

    /// Check if the system is in Nosedive (circuit breaker tripped).
    pub fn is_nosedive(&self) -> bool {
        self.current_tier == StressTier::Nosedive
    }

    /// Check if Flameout protocol should engage.
    pub fn is_flameout(&self) -> bool {
        self.current_tier == StressTier::Flameout
    }

    /// Apply flameout stop tightening to positions (S7.4).
    pub fn flameout_tighten_stops(positions: &mut [Position]) -> Vec<(String, Decimal, Decimal)> {
        let mut changes = Vec::new();
        for pos in positions {
            if let Some(stop) = pos.stop_loss {
                let new_stop = if pos.is_profitable() {
                    // Trail to breakeven
                    pos.avg_entry
                } else {
                    // Tighten to 50% of original stop distance
                    let distance = (pos.avg_entry - stop).abs();
                    let half_distance = distance / Decimal::from(2);
                    if pos.qty > 0 {
                        pos.current_price - half_distance
                    } else {
                        pos.current_price + half_distance
                    }
                };

                if new_stop != stop {
                    changes.push((pos.symbol.0.clone(), stop, new_stop));
                    pos.stop_loss = Some(new_stop);
                }
            }
        }
        changes
    }
}

#[derive(Debug, thiserror::Error)]
pub enum StressOverrideError {
    #[error("cooldown active, {remaining_secs}s remaining")]
    CooldownActive { remaining_secs: u64 },
    #[error("already at normal stress level")]
    AlreadyNormal,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_params() -> StressParams {
        StressParams {
            tier_0_threshold_pct: Decimal::new(30, 1),  // 3.0%
            tier_2_threshold_pct: Decimal::new(50, 1),  // 5.0%
            tier_3_threshold_pct: Decimal::new(80, 1),  // 8.0%
            circuit_breaker_pct: Decimal::new(150, 1),  // 15.0%
            override_cooldown_mins: 0,                   // no cooldown for tests
        }
    }

    #[test]
    fn normal_at_no_drawdown() {
        let engine = StressEngine::new(test_params(), Decimal::from(10_000));
        assert_eq!(engine.current_tier(), StressTier::Normal);
        assert_eq!(engine.multiplier(), Decimal::ONE);
    }

    #[test]
    fn tier1_at_4pct_drawdown() {
        let mut engine = StressEngine::new(test_params(), Decimal::from(10_000));
        let change = engine.update(Decimal::from(9_600)); // 4% DD
        assert_eq!(change, Some(StressTier::Tier1));
        assert_eq!(engine.multiplier(), Decimal::new(75, 2));
    }

    #[test]
    fn flameout_at_9pct_drawdown() {
        let mut engine = StressEngine::new(test_params(), Decimal::from(10_000));
        engine.update(Decimal::from(9_100)); // 9% DD
        assert_eq!(engine.current_tier(), StressTier::Flameout);
    }

    #[test]
    fn nosedive_at_circuit_breaker() {
        let mut engine = StressEngine::new(test_params(), Decimal::from(10_000));
        engine.update(Decimal::from(8_400)); // 16% DD > 15% breaker
        assert!(engine.is_nosedive());
        assert_eq!(engine.multiplier(), Decimal::ZERO);
    }

    #[test]
    fn override_up_one_tier() {
        let mut engine = StressEngine::new(test_params(), Decimal::from(10_000));
        engine.update(Decimal::from(9_100)); // Flameout
        assert_eq!(engine.current_tier(), StressTier::Flameout);

        let result = engine.override_up().unwrap();
        assert_eq!(result, StressTier::Tier2);
    }

    #[test]
    fn apply_stress_to_limit() {
        let mut engine = StressEngine::new(test_params(), Decimal::from(10_000));
        engine.update(Decimal::from(9_600)); // 4% DD = Tier1 (0.75x)

        let max_positions = Decimal::from(10);
        let effective = engine.apply(max_positions);
        assert_eq!(effective, Decimal::new(750, 2)); // 7.5
    }

    #[test]
    fn peak_updates_on_new_high() {
        let mut engine = StressEngine::new(test_params(), Decimal::from(10_000));
        engine.update(Decimal::from(11_000));
        assert_eq!(engine.peak_equity(), Decimal::from(11_000));
        // Now 4% from new peak
        engine.update(Decimal::from(10_560));
        assert_eq!(engine.current_tier(), StressTier::Tier1);
    }
}
