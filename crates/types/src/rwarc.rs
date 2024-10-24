use crate::rwslock::RwSLock;
use std::any::type_name;
use std::sync::{Arc, RwLockReadGuard, RwLockWriteGuard, Weak};

pub struct RwArc<T: Sized>(Arc<RwSLock<T>>);

impl<T: Sized> Clone for RwArc<T> {
    fn clone(&self) -> Self {
        RwArc(self.0.clone())
    }
}

impl<T: Sized> RwArc<T> {
    pub fn new(data: T) -> Self {
        Self(Arc::new(RwSLock::new(data)))
    }

    pub fn read(&self) -> RwLockReadGuard<T> {
        match self.0.read() {
            Ok(result) => {
                result
            }
            Err(err) => { panic!("Read RwArc failed : {}", err) }
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

    pub fn downgrade(&self) -> RwWeak<T> {
        RwWeak(Arc::downgrade(&self.0))
    }

    pub fn downgrade_read_only(&self) -> RwWeakReadOnly<T> {
        RwWeakReadOnly(Arc::downgrade(&self.0))
    }
}

#[derive(Clone, Default)]
pub struct RwWeak<T: Sized>(Weak<RwSLock<T>>);
impl<T: Sized> RwWeak<T> {
    pub fn upgrade(&self) -> RwArc<T> {
        match self.0.upgrade() {
            Some(result) => {
                RwArc(result)
            }
            None => { panic!("Base {} was destroyed", type_name::<T>()) }
        }
    }
}

#[derive(Default)]
pub struct RwWeakReadOnly<T: Sized>(Weak<RwSLock<T>>);
impl<T: Sized> RwWeakReadOnly<T> {
    pub fn upgrade(&self) -> RwArcReadOnly<T> {
        match self.0.upgrade() {
            Some(result) => {
                RwArcReadOnly(result)
            }
            None => { panic!("Base {} was destroyed", type_name::<T>()) }
        }
    }
}
impl<T: Sized> Clone for RwWeakReadOnly<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

#[derive(Clone)]
pub struct RwArcReadOnly<T: Sized>(Arc<RwSLock<T>>);

impl<T: Sized> RwArcReadOnly<T> {
    pub fn read(&self) -> RwLockReadGuard<T> {
        match self.0.read() {
            Ok(result) => {
                result
            }
            Err(err) => { panic!("Read RwArcReadOnly failed : {}", err) }
        }
    }
}


