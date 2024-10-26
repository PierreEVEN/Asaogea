use std::time::{Duration, Instant};

pub struct TimeDelta {
    last: Instant,
    last_recorded_delta: Duration
}

impl Default for TimeDelta {
    fn default() -> Self {
        let last = Instant::now();
        Self{
            last_recorded_delta: last.elapsed(),
            last,
        }
    }
}

impl TimeDelta {
    pub fn next(&mut self) {
        self.last_recorded_delta = self.last.elapsed();
        self.last = Instant::now();
    }
    
    pub fn delta_time(&self) -> &Duration {
        &self.last_recorded_delta
    }
}