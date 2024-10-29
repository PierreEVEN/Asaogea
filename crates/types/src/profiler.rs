use std::sync::{RwLock, RwLockReadGuard};
use std::time::{Duration, Instant};

static mut GLOBAL_PROFILER: Option<Profiler> = None;

pub struct Profiler {
    history: RwLock<Vec<Vec<RecordData>>>,
    current_frame: RwLock<Option<Vec<RecordData>>>
}

impl Profiler {

    pub fn init() {
        unsafe {
            GLOBAL_PROFILER = Some(Profiler {
                history: Default::default(),
                current_frame: Default::default(),
            })
        }
    }

    pub fn get() -> &'static Self {
        unsafe { GLOBAL_PROFILER.as_ref().expect("Profiler have not been initialized using Profiler::init()") }
    }

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

    pub fn record(&self, name: &str) -> Record {
        Record {
            profiler: self,
            data: Some(RecordData {
                name: name.to_string(),
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

    pub fn current(&self) -> Vec<RecordData> {
        if let Some(data) = self.current_frame.read().unwrap().as_ref() {
            data.clone()
        } else {
            vec![]
        }
    }
    
    pub fn clear(&self) {
        self.history.write().unwrap().clear();
        let mut current = self.current_frame.write().unwrap();
        if current.is_some() {
            *current = Some(vec![]);
        }
    }
}

#[derive(Clone, Debug)]
pub struct RecordData {
    pub name: String,
    pub start: Instant,
    pub elapsed: Duration,
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

#[macro_export]
macro_rules! measure {
    ($i:expr, $content:block) => {
        let mut profiler = types::profiler::Profiler::get().record($i);
        let ret = $content;
        profiler.end();
        ret
    }
}