use std::any::type_name;
use std::ops::{Deref, DerefMut};
use std::ptr::null;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Resource<T> {
    data: *const T,
    alloc: ResourceAlloc,
}

impl<T> Drop for Resource<T> {
    fn drop(&mut self) {
        unsafe {
            *(self.alloc.valid as *mut bool) = false;
            drop(Box::from_raw(self.data as *mut T));
        }
    }
}

impl<T> Resource<T> {
    pub fn new(data: T) -> Self {
        Self {
            data: Box::leak(Box::new(data)),
            alloc: ResourceAlloc {
                count: Box::leak(Box::new(AtomicUsize::new(1))),
                valid: Box::leak(Box::new(true)),
            },
        }
    }

    pub fn handle(&self) -> ResourceHandle<T> {
        ResourceHandle {
            ptr: self.data,
            alloc: self.alloc.clone(),
        }
    }

    pub fn handle_mut(&self) -> ResourceHandleMut<T> {
        ResourceHandleMut {
            ptr: self.data,
            alloc: self.alloc.clone(),
        }
    }
}


impl<T> Deref for Resource<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.data.as_ref().unwrap() }
    }
}

impl<T> DerefMut for Resource<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.data.cast_mut() }
    }
}

pub struct ResourceHandle<T> {
    ptr: *const T,
    alloc: ResourceAlloc,
}

impl<T> Clone for ResourceHandle<T> {
    fn clone(&self) -> Self {
        Self {
            ptr: self.ptr,
            alloc: self.alloc.clone(),
        }
    }
}

impl<T> ResourceHandle<T> {
    pub fn is_valid(&self) -> bool {
        unsafe { *self.alloc.valid }
    }
}

impl<T> Default for ResourceHandle<T> {
    fn default() -> Self {
        Self {
            ptr: null(),
            alloc: ResourceAlloc {
                count: Box::leak(Box::new(AtomicUsize::new(1))),
                valid: Box::leak(Box::new(false)),
            },
        }
    }
}

impl<T> Deref for ResourceHandle<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            if *self.alloc.valid {
                self.ptr.as_ref().unwrap()
            } else {
                panic!("Object of type {} have been destroyed", type_name::<T>());
            }
        }
    }
}



pub struct ResourceHandleMut<T> {
    ptr: *const T,
    alloc: ResourceAlloc,
}

impl<T> ResourceHandleMut<T> {
    pub fn is_valid(&self) -> bool {
        unsafe { *self.alloc.valid }
    }

    fn clone(&self) -> ResourceHandle<T> {
        ResourceHandle::<T> {
            ptr: self.ptr,
            alloc: self.alloc.clone(),
        }
    }
}

impl<T> Default for ResourceHandleMut<T> {
    fn default() -> Self {
        Self {
            ptr: null(),
            alloc: ResourceAlloc {
                count: Box::leak(Box::new(AtomicUsize::new(1))),
                valid: Box::leak(Box::new(false)),
            },
        }
    }
}

impl<T> Deref for ResourceHandleMut<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        unsafe {
            if *self.alloc.valid {
                self.ptr.as_ref().unwrap()
            } else {
                panic!("Object of type {} have been destroyed", type_name::<T>());
            }
        }
    }
}

impl<T> DerefMut for ResourceHandleMut<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            if *self.alloc.valid {
                &mut *self.ptr.cast_mut()
            } else {
                panic!("Object of type {} have been destroyed", type_name::<T>());
            }
        }
    }
}


pub struct ResourceAlloc {
    count: *const AtomicUsize,
    valid: *const bool,
}

impl Clone for ResourceAlloc {
    fn clone(&self) -> Self {
        unsafe { (*self.count).fetch_add(1, Ordering::SeqCst); }
        Self {
            count: self.count,
            valid: self.valid,
        }
    }
}

impl Drop for ResourceAlloc {
    fn drop(&mut self) {
        unsafe {
            if (*self.count).fetch_sub(1, Ordering::SeqCst) == 1 {
                // Drop data if this reference is the last one
                drop(Box::from_raw(self.count as *mut AtomicUsize));
                drop(Box::from_raw(self.valid as *mut AtomicUsize));
            }
        }
    }
}
