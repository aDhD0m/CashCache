use std::collections::VecDeque;
use std::time::Instant;

// ---------------------------------------------------------------------------
// RateLimiter — sliding window rate limiter for IBKR message flow
// ---------------------------------------------------------------------------

pub struct RateLimiter {
    /// Max messages per second.
    max_per_sec: u32,
    /// Timestamps of recent messages (sliding window).
    window: VecDeque<Instant>,
}

impl RateLimiter {
    pub fn new(max_per_sec: u32) -> Self {
        Self {
            max_per_sec,
            window: VecDeque::with_capacity(max_per_sec as usize + 1),
        }
    }

    /// Try to acquire a slot. Returns `true` if allowed, `false` if rate limited.
    pub fn try_acquire(&mut self) -> bool {
        let now = Instant::now();
        let one_sec_ago = now - std::time::Duration::from_secs(1);

        // Evict timestamps older than 1 second
        while self.window.front().is_some_and(|&t| t < one_sec_ago) {
            self.window.pop_front();
        }

        if self.window.len() < self.max_per_sec as usize {
            self.window.push_back(now);
            true
        } else {
            false
        }
    }

    /// Current message rate (messages in the last second).
    pub fn current_rate(&self) -> u32 {
        let now = Instant::now();
        let one_sec_ago = now - std::time::Duration::from_secs(1);
        self.window.iter().filter(|&&t| t >= one_sec_ago).count() as u32
    }

    /// Number of remaining slots in the current window.
    pub fn remaining(&self) -> u32 {
        self.max_per_sec.saturating_sub(self.current_rate())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_within_limit() {
        let mut rl = RateLimiter::new(5);
        for _ in 0..5 {
            assert!(rl.try_acquire());
        }
    }

    #[test]
    fn blocks_at_limit() {
        let mut rl = RateLimiter::new(3);
        assert!(rl.try_acquire());
        assert!(rl.try_acquire());
        assert!(rl.try_acquire());
        assert!(!rl.try_acquire());
    }

    #[test]
    fn remaining_tracks_usage() {
        let mut rl = RateLimiter::new(10);
        assert_eq!(rl.remaining(), 10);
        rl.try_acquire();
        rl.try_acquire();
        assert_eq!(rl.remaining(), 8);
    }
}
