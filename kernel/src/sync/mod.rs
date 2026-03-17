pub use spin::{Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// 单核安全的无锁单元（不需要保护）
pub struct UPSafeCell<T> {
    inner: core::cell::UnsafeCell<T>,
}

unsafe impl<T> Sync for UPSafeCell<T> {}
unsafe impl<T> Send for UPSafeCell<T> {}

impl<T> UPSafeCell<T> {
    pub const fn new(value: T) -> Self {
        Self {
            inner: core::cell::UnsafeCell::new(value),
        }
    }

    pub fn borrow_mut(&self) -> &mut T {
        unsafe { &mut *self.inner.get() }
    }

    pub fn borrow(&self) -> &T {
        unsafe { &*self.inner.get() }
    }
}
