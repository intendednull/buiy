//! Small server utilities: a monotonic-clock token bucket (the rate guards) and the
//! `getrandom`-backed entropy the room codes, reconnect tokens, and match seeds need
//! (spec §6.2/§6.3). Entropy is a server-only concern — the pure `dooduel_core` stays
//! dep-free and injects it via `SessionOpts` (spec §4.1).

use std::time::Duration;

use dooduel_core::protocol::ROOM_CODE_LEN;

/// A monotonic-clock token bucket (spec §6.2). Both rate guards ride it: the per-IP
/// connection/Join-attempt limiter (the room-code brute-force guard) and the
/// per-connection intent cap. `capacity` tokens accrue at `refill_per_sec`; each
/// [`try_take`](Self::try_take) consumes one if available.
///
/// Time is an **injected** monotonic `Duration` (elapsed since the bucket's owner
/// started), never a wall clock — so the tests drive it with virtual time and never
/// flake on a real timer.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    capacity: f64,
    refill_per_sec: f64,
    tokens: f64,
    last: Duration,
}

impl TokenBucket {
    /// A bucket that starts full (`capacity` tokens), refilling at `refill_per_sec`.
    pub fn new(capacity: f64, refill_per_sec: f64) -> Self {
        Self {
            capacity,
            refill_per_sec,
            tokens: capacity,
            last: Duration::ZERO,
        }
    }

    /// Accrue tokens for the time since the last call (capped at `capacity`), then take
    /// one if available. `now` must be monotonic non-decreasing (a backward step is
    /// clamped to no elapsed time). Returns `false` when the bucket is empty — the
    /// caller rejects the attempt.
    pub fn try_take(&mut self, now: Duration) -> bool {
        let elapsed = now.saturating_sub(self.last).as_secs_f64();
        self.last = now;
        self.tokens = (self.tokens + elapsed * self.refill_per_sec).min(self.capacity);
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// A fresh room-invite code — `ROOM_CODE_LEN` chars of `[A-Z0-9]`, `getrandom`-backed
/// (spec §6.2). The registry collision-checks it against live rooms. The `% 36` maps
/// a random byte into the alphabet with a negligible modulo bias (the code is only a
/// ~31-bit routing token, collision-checked regardless).
pub fn random_room_code() -> String {
    const ALPHABET: &[u8; 36] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut bytes = [0u8; ROOM_CODE_LEN];
    getrandom::fill(&mut bytes).expect("system entropy for a room code");
    bytes
        .iter()
        .map(|b| ALPHABET[*b as usize % ALPHABET.len()] as char)
        .collect()
}

/// A fresh reconnect token — 128 bits of `getrandom` entropy, lower-hex (spec §6.3).
/// Rotated on every (re)connection by the `Session`; this is the generator it calls.
pub fn random_token() -> String {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).expect("system entropy for a reconnect token");
    let mut s = String::with_capacity(32);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(s, "{b:02x}");
    }
    s
}

/// A fresh per-match PRNG seed — `getrandom`-backed (spec §4.1/§6, W2-review C1). The
/// seed is a redaction target, so it MUST be entropy-backed in networked play, never
/// the deterministic `DEFAULT_MATCH_SEED` (which makes the word stream predictable).
pub fn random_seed() -> u64 {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).expect("system entropy for a match seed");
    u64::from_le_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(ms: u64) -> Duration {
        Duration::from_millis(ms)
    }

    #[test]
    fn token_bucket_allows_a_burst_then_refills() {
        // Capacity 3, refill 1/sec: three immediate takes, the fourth blocked.
        let mut b = TokenBucket::new(3.0, 1.0);
        assert!(b.try_take(d(0)));
        assert!(b.try_take(d(0)));
        assert!(b.try_take(d(0)));
        assert!(!b.try_take(d(0)), "the burst is exhausted");
        // After ~1s one token has accrued.
        assert!(b.try_take(d(1000)), "a token refilled after a second");
        assert!(!b.try_take(d(1000)), "only one refilled");
    }

    #[test]
    fn token_bucket_rejects_sustained_over_rate() {
        // Capacity 5, refill 2/sec. A sustained 10/sec attempt is throttled: the first
        // 5 pass (burst), then only ~2 per second thereafter.
        let mut b = TokenBucket::new(5.0, 2.0);
        let mut allowed = 0;
        // 30 attempts over 3 seconds (every 100ms).
        for i in 0..30u64 {
            if b.try_take(d(i * 100)) {
                allowed += 1;
            }
        }
        // burst 5 + ~2/sec × 3s ≈ 11 — far below the 30 attempted (sustained flood
        // is throttled) yet the burst was honored.
        assert!(allowed >= 5, "the burst passed: {allowed}");
        assert!(
            allowed <= 13,
            "the sustained flood was throttled: {allowed}"
        );
    }

    #[test]
    fn room_code_is_six_uppercase_alnum() {
        let code = random_room_code();
        assert_eq!(code.len(), ROOM_CODE_LEN);
        assert!(
            code.chars()
                .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        );
    }

    #[test]
    fn token_is_128_bit_hex_and_varies() {
        let a = random_token();
        let b = random_token();
        assert_eq!(a.len(), 32, "16 bytes = 32 hex chars");
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b, "each token is fresh entropy");
    }
}
