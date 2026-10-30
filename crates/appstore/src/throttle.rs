use std::future::Future;
use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::Semaphore;
use tokio::time::Instant;

use crate::{Error, Result};

/// Limits for requests to Apple's public endpoints.
#[derive(Debug, Clone)]
pub struct ThrottleConfig {
    /// Requests in flight at once.
    pub max_concurrent: usize,
    /// Starting gap between request starts. Doubles while Apple throttles us
    /// and eases back towards this after successes.
    pub min_interval: Duration,
    /// The widest the gap grows to.
    pub max_interval: Duration,
    /// Retries per request after a throttled or transient failure.
    pub max_retries: u32,
    /// First backoff wait; doubles per retry (plus jitter) up to `max_backoff`.
    pub base_backoff: Duration,
    pub max_backoff: Duration,
}

impl Default for ThrottleConfig {
    /// Apple documents the iTunes Search API at "approximately 20 calls per
    /// minute"; in practice it tolerates short bursts well above that and
    /// answers 403/429 when it won't. So start quick, and halve the pace as
    /// soon as Apple pushes back rather than guessing a fixed safe rate.
    fn default() -> Self {
        Self {
            max_concurrent: 4,
            min_interval: Duration::from_millis(300),
            max_interval: Duration::from_secs(6),
            max_retries: 4,
            base_backoff: Duration::from_secs(2),
            max_backoff: Duration::from_secs(30),
        }
    }
}

/// The outcome of one attempt, as the caller classified it.
pub enum Attempt<T> {
    Done(T),
    /// Apple refused because of request volume (403/429/503).
    Throttled {
        retry_after: Option<Duration>,
    },
    /// A network error or 5xx worth retrying.
    Transient(Error),
    /// Retrying won't help.
    Fatal(Error),
}

/// Shared by every request to one host: a concurrency cap, an adaptive gap
/// between request starts, and exponential backoff with jitter.
pub struct Throttle {
    config: ThrottleConfig,
    permits: Semaphore,
    pace: Mutex<Pace>,
}

struct Pace {
    next_start: Instant,
    interval: Duration,
}

impl Throttle {
    pub fn new(config: ThrottleConfig) -> Self {
        Self {
            permits: Semaphore::new(config.max_concurrent.max(1)),
            pace: Mutex::new(Pace {
                next_start: Instant::now(),
                interval: config.min_interval,
            }),
            config,
        }
    }

    /// The current gap between request starts.
    pub fn interval(&self) -> Duration {
        self.pace.lock().unwrap().interval
    }

    /// Runs `attempt` until it is done, fails fatally, or runs out of retries.
    pub async fn run<T, F, Fut>(&self, mut attempt: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Attempt<T>>,
    {
        let _permit = self.permits.acquire().await.expect("throttle semaphore closed");
        let mut retry = 0;
        loop {
            self.wait_for_slot().await;
            let wait = match attempt().await {
                Attempt::Done(value) => {
                    self.ease();
                    return Ok(value);
                }
                Attempt::Fatal(error) => return Err(error),
                Attempt::Throttled { retry_after } => {
                    self.slow_down();
                    if retry >= self.config.max_retries {
                        return Err(Error::RateLimited);
                    }
                    retry_after.unwrap_or_else(|| self.backoff(retry))
                }
                Attempt::Transient(error) => {
                    if retry >= self.config.max_retries {
                        return Err(error);
                    }
                    self.backoff(retry)
                }
            };
            retry += 1;
            tracing::debug!(retry, ?wait, "backing off");
            tokio::time::sleep(wait.min(self.config.max_backoff)).await;
        }
    }

    async fn wait_for_slot(&self) {
        let start = {
            let mut pace = self.pace.lock().unwrap();
            let start = pace.next_start.max(Instant::now());
            pace.next_start = start + pace.interval;
            start
        };
        tokio::time::sleep_until(start).await;
    }

    fn slow_down(&self) {
        let mut pace = self.pace.lock().unwrap();
        pace.interval = (pace.interval * 2).min(self.config.max_interval);
    }

    fn ease(&self) {
        let mut pace = self.pace.lock().unwrap();
        pace.interval = (pace.interval * 9 / 10).max(self.config.min_interval);
    }

    fn backoff(&self, retry: u32) -> Duration {
        let exponential = self.config.base_backoff.saturating_mul(1 << retry.min(16));
        let jitter = 0.5 + rand::random::<f32>() * 0.5;
        exponential.min(self.config.max_backoff).mul_f32(jitter)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    fn config() -> ThrottleConfig {
        ThrottleConfig {
            max_concurrent: 2,
            min_interval: Duration::from_millis(100),
            max_interval: Duration::from_secs(2),
            max_retries: 3,
            base_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(10),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn retries_throttled_requests_then_succeeds() {
        let throttle = Throttle::new(config());
        let calls = AtomicUsize::new(0);
        let result = throttle
            .run(|| async {
                if calls.fetch_add(1, Ordering::SeqCst) < 2 {
                    Attempt::Throttled { retry_after: None }
                } else {
                    Attempt::Done("ok")
                }
            })
            .await;
        assert_eq!(result.unwrap(), "ok");
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        // Two throttles doubled the gap twice, one success eased it by 10%.
        assert_eq!(throttle.interval(), Duration::from_millis(360));
    }

    #[tokio::test(start_paused = true)]
    async fn gives_up_after_max_retries() {
        let throttle = Throttle::new(config());
        let calls = AtomicUsize::new(0);
        let result: Result<()> = throttle
            .run(|| async {
                calls.fetch_add(1, Ordering::SeqCst);
                Attempt::Throttled {
                    retry_after: Some(Duration::from_secs(1)),
                }
            })
            .await;
        assert!(matches!(result, Err(Error::RateLimited)));
        assert_eq!(calls.load(Ordering::SeqCst), 4);
        assert_eq!(throttle.interval(), Duration::from_secs(1) + Duration::from_millis(600));
    }

    #[tokio::test(start_paused = true)]
    async fn fatal_errors_do_not_retry() {
        let throttle = Throttle::new(config());
        let calls = AtomicUsize::new(0);
        let result: Result<()> = throttle
            .run(|| async {
                calls.fetch_add(1, Ordering::SeqCst);
                Attempt::Fatal(Error::Status(404))
            })
            .await;
        assert!(matches!(result, Err(Error::Status(404))));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn caps_concurrency_and_spaces_starts() {
        let throttle = Arc::new(Throttle::new(config()));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let started = Instant::now();
        let tasks: Vec<_> = (0..6)
            .map(|_| {
                let (throttle, in_flight, peak) = (throttle.clone(), in_flight.clone(), peak.clone());
                tokio::spawn(async move {
                    throttle
                        .run(|| {
                            let (in_flight, peak) = (in_flight.clone(), peak.clone());
                            async move {
                                let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                                peak.fetch_max(now, Ordering::SeqCst);
                                tokio::time::sleep(Duration::from_millis(500)).await;
                                in_flight.fetch_sub(1, Ordering::SeqCst);
                                Attempt::Done(())
                            }
                        })
                        .await
                })
            })
            .collect();
        for task in tasks {
            task.await.unwrap().unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), 2);
        // Six 500ms requests two at a time need at least three rounds.
        assert!(started.elapsed() >= Duration::from_millis(1500));
    }
}
