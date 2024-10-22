use std::sync::{RwLock, RwLockReadGuard};

#[derive(Default)]
pub struct RwOption<T: Sized>(RwLock<Option<T>>);

impl<T: Sized> RwOption<T> {
    pub fn new(t: T) -> Self {
        Self(RwLock::new(Some(t)))
    }
}