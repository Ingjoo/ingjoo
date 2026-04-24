use std::time::Duration;

use rand::Rng;

#[derive(Clone)]
pub struct RetryPolicy {
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl RetryPolicy {
    pub fn new() -> Self {
        Self {
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(60),
        }
    }

    pub fn with_base_delay(mut self, d: Duration) -> Self {
        self.base_delay = d;
        self
    }

    pub fn with_max_delay(mut self, d: Duration) -> Self {
        self.max_delay = d;
        self
    }

    /// 指数退避 + jitter（±20%）
    pub fn delay_for_attempt(&self, attempts: i32) -> Duration {
        let secs = self.base_delay.as_secs_f64() * 2_f64.powi(attempts - 1);
        let secs = secs.min(self.max_delay.as_secs_f64());

        let mut rng = rand::thread_rng();
        let jitter: f64 = rng.gen_range(-0.2..0.2);
        let final_secs = (secs * (1.0 + jitter)).max(0.0);

        Duration::from_secs_f64(final_secs)
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_increases_exponentially() {
        let policy = RetryPolicy::new();
        let d1 = policy.delay_for_attempt(1);
        let d2 = policy.delay_for_attempt(2);
        let d3 = policy.delay_for_attempt(3);
        assert!(d1.as_secs_f64() > 0.0);
        assert!(d2.as_secs_f64() >= d1.as_secs_f64() * 1.5);
        assert!(d3.as_secs_f64() >= d2.as_secs_f64() * 1.5);
    }

    #[test]
    fn delay_capped_at_max() {
        let policy = RetryPolicy::new().with_max_delay(Duration::from_secs(5));
        let d = policy.delay_for_attempt(100);
        assert!(d <= Duration::from_secs(10));
    }
}
