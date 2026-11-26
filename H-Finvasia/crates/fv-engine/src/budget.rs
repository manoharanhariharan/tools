//! Token bucket for venue-imposed request limits.
//!
//! Documented limits (shoonya.com/api-documentation/rate-limits):
//! * order place/modify/cancel — **~10 req/sec** per user, burst-limited;
//! * market-data REST — ~1 req/sec **per instrument** (continuous data must come
//!   from the websocket instead);
//! * websocket — **1 connection per session**.
//!
//! Modelling these explicitly means the app self-limits instead of collecting
//! `Rate_Limited` responses and retrying into a storm.

use std::time::Instant;

/// Default order-request budget.
pub const DEFAULT_ORDER_RATE_PER_SEC: f64 = 10.0;

#[derive(Debug, Clone)]
pub struct TokenBucket {
    capacity: f64,
    refill_per_sec: f64,
    tokens: f64,
    last: Instant,
}

impl TokenBucket {
    pub fn new(refill_per_sec: f64, burst: f64) -> Self {
        Self::new_at(refill_per_sec, burst, Instant::now())
    }

    /// Injectable clock for deterministic tests.
    pub fn new_at(refill_per_sec: f64, burst: f64, now: Instant) -> Self {
        let capacity = burst.max(1.0);
        TokenBucket {
            capacity,
            refill_per_sec: refill_per_sec.max(0.001),
            tokens: capacity,
            last: now,
        }
    }

    pub fn order_bucket() -> Self {
        TokenBucket::new(DEFAULT_ORDER_RATE_PER_SEC, DEFAULT_ORDER_RATE_PER_SEC)
    }

    pub fn capacity(&self) -> f64 {
        self.capacity
    }

    pub fn available(&self) -> f64 {
        self.tokens
    }

    /// Try to take one token. Returns `false` when the caller must wait.
    pub fn try_acquire(&mut self, now: Instant) -> bool {
        self.refill(now);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// How long until the next token is available (zero when one is ready).
    pub fn wait_time(&self, now: Instant) -> std::time::Duration {
        let mut tokens = self.tokens + self.elapsed_secs(now) * self.refill_per_sec;
        if tokens > self.capacity {
            tokens = self.capacity;
        }
        if tokens >= 1.0 {
            std::time::Duration::ZERO
        } else {
            let needed = 1.0 - tokens;
            std::time::Duration::from_secs_f64(needed / self.refill_per_sec)
        }
    }

    fn elapsed_secs(&self, now: Instant) -> f64 {
        now.saturating_duration_since(self.last).as_secs_f64()
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = self.elapsed_secs(now);
        if elapsed > 0.0 {
            self.tokens = (self.tokens + elapsed * self.refill_per_sec).min(self.capacity);
            self.last = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn allows_a_full_burst_then_blocks() {
        let t0 = Instant::now();
        let mut b = TokenBucket::new_at(10.0, 10.0, t0);
        for i in 0..10 {
            assert!(b.try_acquire(t0), "burst request {i} should pass");
        }
        assert!(!b.try_acquire(t0), "11th request must be refused");
    }

    #[test]
    fn refills_over_time() {
        let t0 = Instant::now();
        let mut b = TokenBucket::new_at(10.0, 1.0, t0);
        assert!(b.try_acquire(t0));
        assert!(!b.try_acquire(t0));
        // 10/sec => one token per 100 ms.
        assert!(!b.try_acquire(t0 + Duration::from_millis(50)));
        assert!(b.try_acquire(t0 + Duration::from_millis(120)));
    }

    #[test]
    fn capacity_caps_accumulation() {
        let t0 = Instant::now();
        let mut b = TokenBucket::new_at(10.0, 2.0, t0);
        // Idle for a long time: still only 2 tokens.
        let later = t0 + Duration::from_secs(60);
        assert!(b.try_acquire(later));
        assert!(b.try_acquire(later));
        assert!(!b.try_acquire(later));
    }

    #[test]
    fn wait_time_is_zero_when_ready_and_positive_when_empty() {
        let t0 = Instant::now();
        let mut b = TokenBucket::new_at(10.0, 1.0, t0);
        assert_eq!(b.wait_time(t0), Duration::ZERO);
        assert!(b.try_acquire(t0));
        let wait = b.wait_time(t0);
        assert!(wait > Duration::ZERO);
        assert!(wait <= Duration::from_millis(100));
    }

    #[test]
    fn default_order_bucket_matches_documented_limit() {
        let b = TokenBucket::order_bucket();
        assert_eq!(b.capacity(), 10.0);
    }
}
