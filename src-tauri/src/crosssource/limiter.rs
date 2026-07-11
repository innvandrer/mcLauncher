//! A small token-bucket rate limiter for outbound API calls. Each provider
//! (Modrinth, CurseForge) gets its own bucket; callers `acquire()` a token
//! before every request and are delayed when the bucket runs dry.

use std::time::{Duration, Instant};

struct Bucket {
    tokens: f64,
    last_refill: Instant,
}

pub struct RateLimiter {
    capacity: f64,
    refill_per_sec: f64,
    bucket: tokio::sync::Mutex<Bucket>,
}

impl RateLimiter {
    /// A bucket holding `capacity` burst tokens, refilling at `per_minute`
    /// tokens per minute.
    pub fn new(capacity: u32, per_minute: u32) -> Self {
        Self {
            capacity: capacity.max(1) as f64,
            refill_per_sec: (per_minute.max(1) as f64) / 60.0,
            bucket: tokio::sync::Mutex::new(Bucket {
                tokens: capacity.max(1) as f64,
                last_refill: Instant::now(),
            }),
        }
    }

    /// Refill `bucket` up to `now`, then either consume a token (returning
    /// `None`) or return how long the caller must wait for the next one.
    /// Separated from `acquire` so tests can drive it with synthetic clocks.
    fn poll_at(&self, bucket: &mut Bucket, now: Instant) -> Option<Duration> {
        let elapsed = now.saturating_duration_since(bucket.last_refill);
        bucket.tokens =
            (bucket.tokens + elapsed.as_secs_f64() * self.refill_per_sec).min(self.capacity);
        bucket.last_refill = now;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            None
        } else {
            let missing = 1.0 - bucket.tokens;
            Some(Duration::from_secs_f64(missing / self.refill_per_sec))
        }
    }

    /// Wait until a token is available, then consume it.
    pub async fn acquire(&self) {
        loop {
            let wait = {
                let mut bucket = self.bucket.lock().await;
                self.poll_at(&mut bucket, Instant::now())
            };
            match wait {
                None => return,
                // Sleep outside the lock so concurrent callers make progress.
                Some(d) => tokio::time::sleep(d).await,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bucket_at(limiter: &RateLimiter, now: Instant) -> Bucket {
        let _ = limiter;
        Bucket {
            tokens: 0.0,
            last_refill: now,
        }
    }

    #[test]
    fn burst_then_deny() {
        let limiter = RateLimiter::new(3, 60); // 3 burst, 1/sec refill
        let t0 = Instant::now();
        let mut bucket = Bucket {
            tokens: 3.0,
            last_refill: t0,
        };
        assert!(limiter.poll_at(&mut bucket, t0).is_none());
        assert!(limiter.poll_at(&mut bucket, t0).is_none());
        assert!(limiter.poll_at(&mut bucket, t0).is_none());
        // Bucket dry: fourth call at the same instant must wait ~1s.
        let wait = limiter.poll_at(&mut bucket, t0).expect("should be limited");
        assert!(wait > Duration::from_millis(900) && wait <= Duration::from_secs(1));
    }

    #[test]
    fn refills_over_time() {
        let limiter = RateLimiter::new(3, 60); // 1 token/sec
        let t0 = Instant::now();
        let mut bucket = bucket_at(&limiter, t0);
        // After 2.5s an empty bucket has 2.5 tokens: two grants, then a wait.
        let t1 = t0 + Duration::from_millis(2500);
        assert!(limiter.poll_at(&mut bucket, t1).is_none());
        assert!(limiter.poll_at(&mut bucket, t1).is_none());
        let wait = limiter.poll_at(&mut bucket, t1).expect("should be limited");
        assert!(wait > Duration::from_millis(400) && wait <= Duration::from_millis(600));
    }

    #[test]
    fn never_exceeds_capacity() {
        let limiter = RateLimiter::new(2, 6000); // huge refill rate
        let t0 = Instant::now();
        let mut bucket = bucket_at(&limiter, t0);
        // A long idle period must cap at 2 tokens, not accumulate hundreds.
        let t1 = t0 + Duration::from_secs(3600);
        assert!(limiter.poll_at(&mut bucket, t1).is_none());
        assert!(limiter.poll_at(&mut bucket, t1).is_none());
        assert!(limiter.poll_at(&mut bucket, t1).is_some());
    }
}
