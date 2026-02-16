//! Time utilities for SRT protocol
//!
//! Provides monotonic clock for packet timestamps and timing operations.

use std::ops::{Add, Sub};
use std::time::{Duration, Instant};

/// Monotonic timestamp in microseconds
///
/// SRT uses microsecond timestamps for packet timing, RTT calculation,
/// and congestion control. This type wraps std::time::Instant and provides
/// conversions to/from microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Timestamp(Instant);

impl Timestamp {
    /// Get the current timestamp
    #[inline]
    pub fn now() -> Self {
        Timestamp(Instant::now())
    }

    /// Create a timestamp from a base instant
    #[inline]
    pub fn from_instant(instant: Instant) -> Self {
        Timestamp(instant)
    }

    /// Get the underlying instant
    #[inline]
    pub fn as_instant(&self) -> Instant {
        self.0
    }

    /// Calculate duration since another timestamp
    #[inline]
    pub fn duration_since(&self, earlier: Timestamp) -> Duration {
        self.0.duration_since(earlier.0)
    }

    /// Calculate elapsed time since this timestamp
    #[inline]
    pub fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }

    /// Convert to microseconds since a reference point
    ///
    /// Returns the number of microseconds elapsed since the reference timestamp.
    /// This is used for SRT packet timestamps which are 32-bit microsecond values.
    pub fn as_micros_since(&self, reference: Timestamp) -> u64 {
        self.0
            .duration_since(reference.0)
            .as_micros()
            .try_into()
            .unwrap_or(u64::MAX)
    }

    /// Convert to 32-bit microsecond timestamp (with wraparound)
    ///
    /// SRT uses 32-bit timestamps that wrap around every ~71 minutes.
    /// This matches the SRT protocol specification.
    pub fn as_srt_timestamp(&self, reference: Timestamp) -> u32 {
        self.as_micros_since(reference) as u32
    }

    /// Create a timestamp from microseconds offset from reference
    pub fn from_micros_offset(reference: Timestamp, micros: u64) -> Self {
        Timestamp(reference.0 + Duration::from_micros(micros))
    }
}

impl Add<Duration> for Timestamp {
    type Output = Timestamp;

    fn add(self, duration: Duration) -> Timestamp {
        Timestamp(self.0 + duration)
    }
}

impl Sub<Duration> for Timestamp {
    type Output = Timestamp;

    fn sub(self, duration: Duration) -> Timestamp {
        Timestamp(self.0 - duration)
    }
}

impl Sub for Timestamp {
    type Output = Duration;

    fn sub(self, other: Timestamp) -> Duration {
        self.0.duration_since(other.0)
    }
}

/// Timer for periodic operations
///
/// Used for periodic ACKs, NAKs, and keep-alive messages.
pub struct Timer {
    interval: Duration,
    last_fire: Timestamp,
}

impl Timer {
    /// Create a new timer with the given interval
    pub fn new(interval: Duration) -> Self {
        Timer {
            interval,
            last_fire: Timestamp::now(),
        }
    }

    /// Check if the timer has expired
    pub fn expired(&self) -> bool {
        self.last_fire.elapsed() >= self.interval
    }

    /// Reset the timer
    pub fn reset(&mut self) {
        self.last_fire = Timestamp::now();
    }

    /// Get time until next expiration
    pub fn time_until_expiration(&self) -> Duration {
        let elapsed = self.last_fire.elapsed();
        if elapsed >= self.interval {
            Duration::ZERO
        } else {
            self.interval - elapsed
        }
    }

    /// Fire the timer if expired, returning true if it fired
    pub fn try_fire(&mut self) -> bool {
        if self.expired() {
            self.reset();
            true
        } else {
            false
        }
    }
}

/// Rate limiter using token bucket algorithm
///
/// Used for pacing packet transmission according to congestion control.
pub struct RateLimiter {
    /// Maximum tokens (burst size)
    capacity: u64,
    /// Current token count
    tokens: u64,
    /// Tokens added per microsecond
    rate: f64,
    /// Last update time
    last_update: Timestamp,
}

impl RateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `rate_bps` - Rate in bits per second
    /// * `burst_bytes` - Maximum burst size in bytes
    pub fn new(rate_bps: u64, burst_bytes: u64) -> Self {
        let rate_bytes_per_us = (rate_bps as f64) / 8.0 / 1_000_000.0;

        RateLimiter {
            capacity: burst_bytes,
            tokens: burst_bytes,
            rate: rate_bytes_per_us,
            last_update: Timestamp::now(),
        }
    }

    /// Update the rate
    pub fn set_rate(&mut self, rate_bps: u64) {
        self.refill();
        self.rate = (rate_bps as f64) / 8.0 / 1_000_000.0;
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Timestamp::now();
        let elapsed_us = now.as_micros_since(self.last_update) as f64;
        let new_tokens = (elapsed_us * self.rate) as u64;

        if new_tokens > 0 {
            self.tokens = (self.tokens + new_tokens).min(self.capacity);
            self.last_update = now;
        }
    }

    /// Check if we can send `bytes` worth of data
    pub fn check(&mut self, bytes: usize) -> bool {
        self.refill();
        self.tokens >= bytes as u64
    }

    /// Consume tokens for sending `bytes` worth of data
    ///
    /// Returns true if successful, false if insufficient tokens
    pub fn consume(&mut self, bytes: usize) -> bool {
        self.refill();
        if self.tokens >= bytes as u64 {
            self.tokens -= bytes as u64;
            true
        } else {
            false
        }
    }

    /// Get time to wait before `bytes` will be available
    pub fn time_to_available(&mut self, bytes: usize) -> Duration {
        self.refill();

        if self.tokens >= bytes as u64 {
            return Duration::ZERO;
        }

        let needed = (bytes as u64) - self.tokens;
        let micros = (needed as f64 / self.rate).ceil() as u64;
        Duration::from_micros(micros)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_timestamp_creation() {
        let ts = Timestamp::now();
        assert!(ts.elapsed() < Duration::from_millis(10));
    }

    #[test]
    fn test_timestamp_arithmetic() {
        let ts1 = Timestamp::now();
        thread::sleep(Duration::from_millis(10));
        let ts2 = Timestamp::now();

        let diff = ts2 - ts1;
        assert!(diff >= Duration::from_millis(10));
        assert!(diff < Duration::from_millis(50));
    }

    #[test]
    fn test_srt_timestamp() {
        let reference = Timestamp::now();
        thread::sleep(Duration::from_millis(10));
        let ts = Timestamp::now();

        let srt_ts = ts.as_srt_timestamp(reference);
        assert!(srt_ts >= 10_000); // At least 10ms = 10,000 microseconds
        assert!(srt_ts < 50_000); // Less than 50ms
    }

    #[test]
    fn test_timer() {
        let mut timer = Timer::new(Duration::from_millis(10));
        assert!(!timer.expired());

        thread::sleep(Duration::from_millis(11));
        assert!(timer.expired());

        timer.reset();
        assert!(!timer.expired());
    }

    #[test]
    fn test_timer_try_fire() {
        let mut timer = Timer::new(Duration::from_millis(10));
        assert!(!timer.try_fire());

        thread::sleep(Duration::from_millis(11));
        assert!(timer.try_fire());
        assert!(!timer.try_fire()); // Should not fire again immediately
    }

    #[test]
    fn test_rate_limiter() {
        // 1 MB/s = 1 byte per microsecond
        let mut limiter = RateLimiter::new(8_000_000, 1000);

        // Should be able to send initially
        assert!(limiter.check(500));
        assert!(limiter.consume(500));

        // Should still have tokens
        assert!(limiter.check(500));
        assert!(limiter.consume(500));

        // Should be depleted now
        assert!(!limiter.check(100));

        // Wait a bit and tokens should refill
        thread::sleep(Duration::from_millis(1));
        assert!(limiter.check(100));
    }

    #[test]
    fn test_rate_limiter_time_to_available() {
        let mut limiter = RateLimiter::new(1_000_000, 100); // 1 Mbps, 100 byte burst

        limiter.consume(100); // Deplete all tokens

        let wait_time = limiter.time_to_available(100);
        assert!(wait_time > Duration::ZERO);
        assert!(wait_time <= Duration::from_millis(1000)); // Should be around 800ms
    }
}
