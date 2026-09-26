//! A sliding-window rate limiter for the AniList client. AniList enforces 30 requests
//! per minute in practice, so every caller (Home, Browse, Show, Search, and the
//! Franchise walk) shares one `Throttle` and awaits `acquire()` before each request.

use std::collections::VecDeque;
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::Instant;

pub struct Throttle {
    limit: usize,
    window: Duration,
    calls: Mutex<VecDeque<Instant>>,
}

impl Throttle {
    pub fn new(limit: usize, window: Duration) -> Self {
        Self { limit, window, calls: Mutex::new(VecDeque::with_capacity(limit)) }
    }

    /// AniList's practical limit: 30 requests per minute (their docs currently say 90,
    /// but 30/min is what holds up without 429s).
    pub fn anilist() -> Self {
        Self::new(30, Duration::from_secs(60))
    }

    /// Waits, if needed, until a call is allowed under the limit, then records it.
    pub async fn acquire(&self) {
        loop {
            let wait = self.next_wait().await;
            match wait {
                None => return,
                Some(duration) => tokio::time::sleep(duration).await,
            }
        }
    }

    /// Drops expired timestamps and either reserves a slot (returning `None`) or
    /// reports how long to wait for the oldest timestamp to leave the window.
    async fn next_wait(&self) -> Option<Duration> {
        let mut calls = self.calls.lock().await;
        let now = Instant::now();
        while calls.front().is_some_and(|&t| now.duration_since(t) >= self.window) {
            calls.pop_front();
        }
        if calls.len() < self.limit {
            calls.push_back(now);
            None
        } else {
            let oldest = *calls.front().expect("len >= limit > 0 implies a front element");
            Some((oldest + self.window).saturating_duration_since(now))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn allows_a_burst_up_to_the_limit_without_waiting() {
        let throttle = Throttle::new(3, Duration::from_secs(60));
        let start = Instant::now();
        for _ in 0..3 {
            throttle.acquire().await;
        }
        assert_eq!(Instant::now(), start);
    }

    #[tokio::test(start_paused = true)]
    async fn the_call_after_the_limit_waits_for_the_window() {
        let throttle = Throttle::new(1, Duration::from_secs(60));
        throttle.acquire().await;
        let start = Instant::now();
        throttle.acquire().await;
        assert!(Instant::now() >= start + Duration::from_secs(60));
    }

    #[tokio::test(start_paused = true)]
    async fn a_freed_slot_lets_the_next_call_through_immediately() {
        let throttle = Throttle::new(1, Duration::from_secs(60));
        throttle.acquire().await;
        tokio::time::advance(Duration::from_secs(61)).await;
        let start = Instant::now();
        throttle.acquire().await;
        assert_eq!(Instant::now(), start);
    }
}
