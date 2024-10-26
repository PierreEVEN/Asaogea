use std::any::type_name;
use std::ops::{Deref, DerefMut};
use std::ptr::null;
use std::sync::atomic::{AtomicUsize, Ordering};

pub struct Resource<T> {
    data: *const T,
    alloc: ResourceAlloc,
}

impl<T> Default for Resource<T> {
    fn default() -> Self {
        Self {
            data: null(),
            alloc: ResourceAlloc::default(),
        }
    }
}

impl<T> Drop for Resource<T> {
    fn drop(&mut self) {
        if !self.data.is_null() {
            unsafe {
                drop(Box::from_raw(self.data as *mut T));
                *(self.alloc.valid as *mut bool) = false;
            }
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

    pub fn is_valid(&self) -> bool {
        !self.data.is_null()
    }

    pub fn handle(&self) -> ResourceHandle<T> {
        assert!(!self.data.is_null(), "Cannot get handle of a null Resource<{}>", type_name::<T>());
        ResourceHandle {
            ptr: self.data,
            alloc: self.alloc.clone(),
        }
    }

    pub fn handle_mut(&self) -> ResourceHandleMut<T> {
        assert!(!self.data.is_null(), "Cannot get handle of a null Resource<{}>", type_name::<T>());
        ResourceHandleMut {
            ptr: self.data,
            alloc: self.alloc.clone(),
        }
    }
}


impl<T> Deref for Resource<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        assert!(!self.data.is_null(), "Resource<{}> is null", type_name::<T>());
        unsafe { self.data.as_ref().unwrap() }
    }
}

impl<T> DerefMut for Resource<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        assert!(!self.data.is_null(), "Resource<{}> is null", type_name::<T>());
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

    pub fn as_ref(&self) -> ResourceHandle<T> {
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
            alloc: ResourceAlloc::default()
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

impl Default for ResourceAlloc {
    fn default() -> Self {
        Self {
            count: Box::leak(Box::new(AtomicUsize::new(1))),
            valid: Box::leak(Box::new(false)),
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
