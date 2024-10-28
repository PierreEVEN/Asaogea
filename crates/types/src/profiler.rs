use std::sync::{RwLock, RwLockReadGuard};
use std::time::{Duration, Instant};

pub struct Profiler {
    history: RwLock<Vec<Vec<RecordData>>>,
    current_frame: RwLock<Option<Vec<RecordData>>>
}

impl Profiler {
    pub fn new_frame(&self) {
        let mut current = self.current_frame.write().unwrap();
        if let Some(current) = current.take() {
            self.history.write().unwrap().push(current)
        }
        else {
            return;
        }
        *current = Some(vec![]);
    }

    pub fn record(&self) -> Record {
        Record {
            profiler: self,
            data: Some(RecordData {
                start: Instant::now(),
                elapsed: Duration::default(),
            }),
        }
    }

    pub fn enable(&self, enabled: bool) {
        if enabled {
            *self.current_frame.write().unwrap() = Some(vec![]);
        } else {
            *self.current_frame.write().unwrap() = None;
        }
    }
    
    pub fn history(&self) -> RwLockReadGuard<Vec<Vec<RecordData>>> {
        self.history.read().unwrap()
    }

    pub fn current(&self) -> RwLockReadGuard<Vec<RecordData>> {
        self.current_frame.read().unwrap().unwrap()
    }
    
    pub fn clear(&self) {
        self.history.write().unwrap().clear();
        self.current_frame.write().unwrap().clear();
    }
}

pub struct RecordData {
    start: Instant,
    elapsed: Duration,
}

impl RecordData {
    fn duration(&self) -> &Duration {
        &self.elapsed
    }
}

pub struct Record<'a> {
    profiler: &'a Profiler,
    data: Option<RecordData>,
}

impl<'a> Record<'a> {
    pub fn end(mut self) {
        if let Some(mut data) = self.data.take() {
            data.elapsed = data.start.elapsed();
            if let Some(current_frame) = self.profiler.current_frame.write().unwrap().as_mut() {
                current_frame.push(data);
            }
        }
    }
}

impl<'a> Drop for Record<'a> {
    fn drop(&mut self) {
        if let Some(mut data) = self.data.take() {
            data.elapsed = data.start.elapsed();
            if let Some(current_frame) = self.profiler.current_frame.write().unwrap().as_mut() {
                current_frame.push(data);
            }
        }
    }
}