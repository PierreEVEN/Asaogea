use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use anyhow::Error;

pub struct RwSLock<T: Sized>(RwLock<T>);

impl<T: Sized> RwSLock<T> {
    pub const fn new(t: T) -> Self {
        Self(RwLock::new(t))
    }
    
    pub fn read(&self) -> Result<RwLockReadGuard<'_, T>, Error> {
        self.0.read().map_err(|b| anyhow::anyhow!("{}", b))
    }

    pub fn write(&self) -> Result<RwLockWriteGuard<'_, T>, Error> {
        self.0.write().map_err(|b| anyhow::anyhow!("{}", b))
    }
}