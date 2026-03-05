//! Pure technical indicator functions for module scanning.

use std::collections::VecDeque;

use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

// ---------------------------------------------------------------------------
// Simple Moving Average
// ---------------------------------------------------------------------------

pub fn sma(values: &VecDeque<Decimal>, period: usize) -> Option<Decimal> {
    if values.len() < period || period == 0 {
        return None;
    }
    let sum: Decimal = values.iter().rev().take(period).sum();
    Some(sum / Decimal::from(period as u64))
}

// ---------------------------------------------------------------------------
// Standard Deviation (population)
// ---------------------------------------------------------------------------

pub fn std_dev(values: &VecDeque<Decimal>, period: usize, mean: Decimal) -> Option<Decimal> {
    if values.len() < period || period == 0 {
        return None;
    }
    let variance: Decimal = values
        .iter()
        .rev()
        .take(period)
        .map(|v| {
            let diff = *v - mean;
            diff * diff
        })
        .sum::<Decimal>()
        / Decimal::from(period as u64);

    // Decimal doesn't have sqrt — convert through f64
    let var_f64 = variance.to_f64()?;
    Decimal::from_f64_retain(var_f64.sqrt())
}

// ---------------------------------------------------------------------------
// RSI (Wilder smoothing)
// ---------------------------------------------------------------------------

/// Compute RSI from a price series. Requires at least `period + 1` values.
pub fn rsi(prices: &VecDeque<Decimal>, period: usize) -> Option<Decimal> {
    if prices.len() < period + 1 || period == 0 {
        return None;
    }

    let recent: Vec<&Decimal> = prices.iter().rev().take(period + 1).collect();
    // `recent` is newest-first, reverse to get chronological order
    let chronological: Vec<Decimal> = recent.into_iter().rev().copied().collect();

    let mut gains = Decimal::ZERO;
    let mut losses = Decimal::ZERO;

    // Initial average gain/loss
    for i in 1..=period {
        let change = chronological[i] - chronological[i - 1];
        if change > Decimal::ZERO {
            gains += change;
        } else {
            losses += change.abs();
        }
    }

    let period_dec = Decimal::from(period as u64);
    let avg_gain = gains / period_dec;
    let avg_loss = losses / period_dec;

    if avg_loss.is_zero() {
        return Some(Decimal::from(100));
    }

    let rs = avg_gain / avg_loss;
    let rsi = Decimal::from(100) - (Decimal::from(100) / (Decimal::ONE + rs));
    Some(rsi)
}

// ---------------------------------------------------------------------------
// Bollinger Bands
// ---------------------------------------------------------------------------

/// Returns (upper, middle, lower) bands.
pub fn bollinger(
    prices: &VecDeque<Decimal>,
    period: usize,
    deviation: Decimal,
) -> Option<(Decimal, Decimal, Decimal)> {
    let middle = sma(prices, period)?;
    let sd = std_dev(prices, period, middle)?;

    let upper = middle + deviation * sd;
    let lower = middle - deviation * sd;

    Some((upper, middle, lower))
}

// ---------------------------------------------------------------------------
// Relative Volume
// ---------------------------------------------------------------------------

pub fn rvol(current_vol: u64, avg_vol: u64) -> Decimal {
    if avg_vol == 0 {
        return Decimal::ZERO;
    }
    Decimal::from(current_vol) / Decimal::from(avg_vol)
}

// ---------------------------------------------------------------------------
// Volume trend (declining = true if last N volumes are downtrending)
// ---------------------------------------------------------------------------

pub fn volume_declining(volumes: &VecDeque<u64>, lookback: usize) -> bool {
    if volumes.len() < lookback || lookback < 2 {
        return false;
    }
    let start = volumes.len() - lookback;
    let recent: Vec<u64> = volumes.iter().skip(start).copied().collect();
    // Check if generally declining (more decreases than increases)
    let mut declines = 0u32;
    for i in 1..recent.len() {
        if recent[i] < recent[i - 1] {
            declines += 1;
        }
    }
    declines > (lookback as u32 - 1) / 2
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn dec(v: &str) -> Decimal {
        v.parse().unwrap()
    }

    fn make_deque(vals: &[&str]) -> VecDeque<Decimal> {
        vals.iter().map(|v| dec(v)).collect()
    }

    #[test]
    fn test_sma_basic() {
        let prices = make_deque(&["10", "20", "30", "40", "50"]);
        let result = sma(&prices, 5).unwrap();
        assert_eq!(result, dec("30"));
    }

    #[test]
    fn test_sma_insufficient_data() {
        let prices = make_deque(&["10", "20"]);
        assert!(sma(&prices, 5).is_none());
    }

    #[test]
    fn test_sma_uses_latest_values() {
        let prices = make_deque(&["1", "2", "3", "4", "5", "100", "200", "300"]);
        // SMA(3) of last 3: 100, 200, 300 = 200
        let result = sma(&prices, 3).unwrap();
        assert_eq!(result, dec("200"));
    }

    #[test]
    fn test_rsi_all_gains() {
        // Monotonically increasing — RSI should be 100
        let prices = make_deque(&[
            "10", "11", "12", "13", "14", "15", "16", "17", "18", "19", "20", "21", "22", "23",
            "24",
        ]);
        let result = rsi(&prices, 14).unwrap();
        assert_eq!(result, dec("100"));
    }

    #[test]
    fn test_rsi_all_losses() {
        // Monotonically decreasing — RSI should be 0
        let prices = make_deque(&[
            "24", "23", "22", "21", "20", "19", "18", "17", "16", "15", "14", "13", "12", "11",
            "10",
        ]);
        let result = rsi(&prices, 14).unwrap();
        assert_eq!(result, dec("0"));
    }

    #[test]
    fn test_rsi_mixed() {
        // Equal gains and losses — RSI should be 50
        let prices = make_deque(&[
            "10", "12", "10", "12", "10", "12", "10", "12", "10", "12", "10", "12", "10", "12",
            "10",
        ]);
        let result = rsi(&prices, 14).unwrap();
        assert_eq!(result, dec("50"));
    }

    #[test]
    fn test_rsi_insufficient_data() {
        let prices = make_deque(&["10", "11", "12"]);
        assert!(rsi(&prices, 14).is_none());
    }

    #[test]
    fn test_bollinger_symmetric() {
        // Constant prices — std_dev = 0, bands collapse to SMA
        let prices = make_deque(&[
            "100", "100", "100", "100", "100", "100", "100", "100", "100", "100", "100", "100",
            "100", "100", "100", "100", "100", "100", "100", "100",
        ]);
        let (upper, middle, lower) = bollinger(&prices, 20, dec("2")).unwrap();
        assert_eq!(middle, dec("100"));
        assert_eq!(upper, dec("100"));
        assert_eq!(lower, dec("100"));
    }

    #[test]
    fn test_bollinger_spread() {
        // Non-constant prices — upper > middle > lower
        let prices = make_deque(&[
            "98", "99", "100", "101", "102", "98", "99", "100", "101", "102", "98", "99", "100",
            "101", "102", "98", "99", "100", "101", "102",
        ]);
        let (upper, middle, lower) = bollinger(&prices, 20, dec("2")).unwrap();
        assert!(upper > middle);
        assert!(middle > lower);
    }

    #[test]
    fn test_rvol() {
        assert_eq!(rvol(2_000_000, 1_000_000), dec("2"));
        assert_eq!(rvol(500_000, 1_000_000), dec("0.5"));
        assert_eq!(rvol(1_000_000, 0), dec("0"));
    }

    #[test]
    fn test_volume_declining() {
        let vols: VecDeque<u64> = vec![100, 90, 80, 70, 60].into();
        assert!(volume_declining(&vols, 5));

        let vols: VecDeque<u64> = vec![60, 70, 80, 90, 100].into();
        assert!(!volume_declining(&vols, 5));
    }
}
