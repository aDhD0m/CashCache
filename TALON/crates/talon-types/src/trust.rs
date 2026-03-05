use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::module::ModuleId;
use crate::risk::RegimeState;

// ---------------------------------------------------------------------------
// Trust calibration (S8)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ActionClass {
    Entry,
    Exit,
    StopAdjust,
    PositionScale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum VixBucket {
    Low,       // <15
    Normal,    // 15-20
    Elevated,  // 20-30
    High,      // 30+
}

impl VixBucket {
    pub fn from_vix(vix: f64) -> Self {
        if vix < 15.0 {
            Self::Low
        } else if vix < 20.0 {
            Self::Normal
        } else if vix < 30.0 {
            Self::Elevated
        } else {
            Self::High
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TimeBucket {
    Open,        // 9:30-10:00
    MidMorning,  // 10:00-11:30
    Lunch,       // 11:30-13:00
    Afternoon,   // 13:00-15:00
    Close,       // 15:00-16:00
}

// ---------------------------------------------------------------------------
// Trust key — the unit of trust (S8.1)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TrustKey {
    pub module: ModuleId,
    pub regime: RegimeState,
    pub action_class: ActionClass,
}

// ---------------------------------------------------------------------------
// Trust entry (S8.2)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrustEntry {
    pub approvals: u32,
    pub rejections: u32,
    pub unique_days_of_week: HashSet<chrono::Weekday>,
    pub vix_buckets_seen: HashSet<VixBucket>,
    pub time_buckets_seen: HashSet<TimeBucket>,
}

impl TrustEntry {
    pub fn new() -> Self {
        Self {
            approvals: 0,
            rejections: 0,
            unique_days_of_week: HashSet::new(),
            vix_buckets_seen: HashSet::new(),
            time_buckets_seen: HashSet::new(),
        }
    }

    /// S8.3 — Diversity requirements for auto-trust qualification.
    pub fn qualifies_for_auto_trust(&self) -> bool {
        self.approvals >= 100
            && self.rejections == 0
            && self.unique_days_of_week.len() >= 4
            && self.vix_buckets_seen.len() >= 2
            && self.time_buckets_seen.len() >= 2
    }
}

impl Default for TrustEntry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Trust ledger
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct TrustLedger {
    pub entries: std::collections::HashMap<TrustKey, TrustEntry>,
}

impl TrustLedger {
    pub fn record_approval(
        &mut self,
        key: TrustKey,
        weekday: chrono::Weekday,
        vix: VixBucket,
        time: TimeBucket,
    ) {
        let entry = self.entries.entry(key).or_default();
        entry.approvals += 1;
        entry.unique_days_of_week.insert(weekday);
        entry.vix_buckets_seen.insert(vix);
        entry.time_buckets_seen.insert(time);
    }

    /// S8.5 — Single rejection = immediate revocation. Counter resets.
    pub fn record_rejection(&mut self, key: TrustKey) {
        let entry = self.entries.entry(key).or_default();
        entry.rejections += 1;
        entry.approvals = 0;
        entry.unique_days_of_week.clear();
        entry.vix_buckets_seen.clear();
        entry.time_buckets_seen.clear();
    }

    pub fn has_auto_trust(&self, key: &TrustKey) -> bool {
        self.entries
            .get(key)
            .is_some_and(|e| e.qualifies_for_auto_trust())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_requires_diversity() {
        let mut entry = TrustEntry::new();
        // 100 approvals all on one day, one vix bucket, one time bucket
        for _ in 0..100 {
            entry.approvals += 1;
            entry
                .unique_days_of_week
                .insert(chrono::Weekday::Mon);
            entry.vix_buckets_seen.insert(VixBucket::Low);
            entry.time_buckets_seen.insert(TimeBucket::Open);
        }
        assert!(!entry.qualifies_for_auto_trust()); // fails diversity

        // Add diversity
        entry.unique_days_of_week.insert(chrono::Weekday::Tue);
        entry.unique_days_of_week.insert(chrono::Weekday::Wed);
        entry.unique_days_of_week.insert(chrono::Weekday::Thu);
        entry.vix_buckets_seen.insert(VixBucket::Elevated);
        entry.time_buckets_seen.insert(TimeBucket::Afternoon);
        assert!(entry.qualifies_for_auto_trust());
    }

    #[test]
    fn single_rejection_revokes_trust() {
        let mut ledger = TrustLedger::default();
        let key = TrustKey {
            module: ModuleId::Climb,
            regime: RegimeState::Trending,
            action_class: ActionClass::Entry,
        };

        for _ in 0..100 {
            ledger.record_approval(
                key.clone(),
                chrono::Weekday::Mon,
                VixBucket::Low,
                TimeBucket::Open,
            );
        }
        // Not diverse enough yet, but approvals are there
        // Add diversity via record_approval with different params
        for wd in [
            chrono::Weekday::Tue,
            chrono::Weekday::Wed,
            chrono::Weekday::Thu,
        ] {
            ledger.record_approval(key.clone(), wd, VixBucket::Elevated, TimeBucket::Afternoon);
        }

        // Now reject — everything resets
        ledger.record_rejection(key.clone());
        let entry = ledger.entries.get(&key).unwrap();
        assert_eq!(entry.approvals, 0);
        assert!(entry.unique_days_of_week.is_empty());
    }
}
