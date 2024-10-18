use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Clone)]
pub struct RwArc<T: Sized>(std::sync::Arc<RwLock<T>>);

impl<T: Sized> RwArc<T> {
    pub fn new(data: T) -> Self {
        Self(std::sync::Arc::new(RwLock::new(data)))
    }

    pub fn read(&self) -> RwLockReadGuard<T> {
        match self.0.read() {
            Ok(result) => {
                result
            }
            Err(err) => { panic!("Write RwArc failed : {}", err) }
        }
    }

    pub fn write(&self) -> RwLockWriteGuard<T> {
        match self.0.write() {
            Ok(result) => {
                result
            }
            Err(err) => { panic!("Write RwArc failed : {}", err) }
        }
    }
}