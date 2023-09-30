//! A naïve spinning mutex.
//!
//! Waiting threads hammer an atomic variable until it becomes available. Best-case latency is low, but worst-case
//! latency is theoretically infinite.
use crate::LockAction;
use core::{
    cell::UnsafeCell,
    default::Default,
    fmt,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

/// A [spin lock](https://en.m.wikipedia.org/wiki/Spinlock) providing mutually exclusive access to data.
///
pub struct SpinMutex<T: ?Sized, L: LockAction> {
    locked: AtomicBool,
    _marker: core::marker::PhantomData<L>,
    data: UnsafeCell<T>,
}

/// A guard that provides mutable data access.
///
/// When the guard falls out of scope it will release the lock.
pub struct SpinMutexGuard<'a, T: ?Sized + 'a, L: LockAction> {
    lock: &'a AtomicBool,
    data: &'a mut T,
    _marker: core::marker::PhantomData<L>,
}

unsafe impl<T: ?Sized + Send, L: LockAction> Sync for SpinMutex<T, L> {}
unsafe impl<T: ?Sized + Send, L: LockAction> Send for SpinMutex<T, L> {}
unsafe impl<T: ?Sized + Sync, L: LockAction> Sync for SpinMutexGuard<'_, T, L> {}
unsafe impl<T: ?Sized + Send, L: LockAction> Send for SpinMutexGuard<'_, T, L> {}

impl<T, L: LockAction> SpinMutex<T, L> {
    /// Creates a new [`SpinMutex`] wrapping the supplied data.
    ///
    /// # Example
    ///
    /// ```
    /// use kernel_sync::SpinDefaultMutex;
    ///
    /// static MUTEX: SpinDefaultMutex<()> = SpinDefaultMutex::new(());
    ///
    /// fn demo() {
    ///     let lock = MUTEX.lock();
    ///     // do something with lock
    ///     drop(lock);
    /// }
    /// ```
    #[inline(always)]
    pub const fn new(data: T) -> Self {
        SpinMutex {
            locked: AtomicBool::new(false),
            data: UnsafeCell::new(data),
            _marker: core::marker::PhantomData,
        }
    }

    /// Consumes this [`SpinMutex`] and unwraps the underlying data.
    ///
    /// # Example
    ///
    /// ```
    /// let lock = kernel_sync::SpinDefaultMutex::new(42);
    /// assert_eq!(42, lock.into_inner());
    /// ```
    #[inline(always)]
    pub fn into_inner(self) -> T {
        // We know statically that there are no outstanding references to
        // `self` so there's no need to lock.
        self.data.into_inner()
    }
    /// Returns a mutable pointer to the underlying data.
    ///
    /// This is mostly meant to be used for applications which require manual unlocking, but where
    /// storing both the lock and the pointer to the inner data gets inefficient.
    ///
    /// # Example
    /// ```
    /// let lock = kernel_sync::SpinDefaultMutex::new(42);
    ///
    /// unsafe {
    ///     core::mem::forget(lock.lock());
    ///
    ///     assert_eq!(lock.as_mut_ptr().read(), 42);
    ///     lock.as_mut_ptr().write(58);
    ///
    ///     lock.force_unlock();
    /// }
    ///
    /// assert_eq!(*lock.lock(), 58);
    ///
    /// ```
    #[inline(always)]
    pub fn as_mut_ptr(&self) -> *mut T {
        self.data.get()
    }
}

impl<T: ?Sized, L: LockAction> SpinMutex<T, L> {
    /// Locks the [`SpinMutex`] and returns a guard that permits access to the inner data.
    ///
    /// The returned value may be dereferenced for data access
    /// and the lock will be dropped when the guard falls out of scope.
    ///
    /// ```
    /// let lock = kernel_sync::SpinDefaultMutex::new(0);
    /// {
    ///     let mut data = lock.lock();
    ///     // The lock is now locked and the data can be accessed
    ///     *data += 1;
    ///     // The lock is implicitly dropped at the end of the scope
    /// }
    /// ```
    #[inline(always)]
    pub fn lock(&self) -> SpinMutexGuard<T, L> {
        L::before_lock();
        while self
            .locked
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            // Wait until the lock looks unlocked before retrying
            while self.is_locked() {
                core::hint::spin_loop();
            }
        }
        SpinMutexGuard {
            lock: &self.locked,
            data: unsafe { &mut *self.data.get() },
            _marker: Default::default(),
        }
    }
    /// Try to lock this [`SpinMutex`], returning a lock guard if successful.
    ///
    /// # Example
    ///
    /// ```
    /// let lock = kernel_sync::SpinDefaultMutex::new(42);
    ///
    /// let maybe_guard = lock.try_lock();
    /// assert!(maybe_guard.is_some());
    ///
    /// // `maybe_guard` is still held, so the second call fails
    /// let maybe_guard2 = lock.try_lock();
    /// assert!(maybe_guard2.is_none());
    /// ```
    #[inline(always)]
    pub fn try_lock(&self) -> Option<SpinMutexGuard<T, L>> {
        L::before_lock();
        if self
            .locked
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(SpinMutexGuard {
                lock: &self.locked,
                data: unsafe { &mut *self.data.get() },
                _marker: Default::default(),
            })
        } else {
            L::after_lock();
            None
        }
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the [`SpinMutex`] mutably, and a mutable reference is guaranteed to be exclusive in
    /// Rust, no actual locking needs to take place -- the mutable borrow statically guarantees no locks exist. As
    /// such, this is a 'zero-cost' operation.
    ///
    /// # Example
    ///
    /// ```
    /// let mut lock = kernel_sync::SpinDefaultMutex::new(0);
    /// *lock.get_mut() = 10;
    /// assert_eq!(*lock.lock(), 10);
    /// ```
    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        // We know statically that there are no other references to `self`, so
        // there's no need to lock the inner mutex.
        unsafe { &mut *self.data.get() }
    }
    /// Returns `true` if the lock is currently held.
    ///
    /// # Safety
    ///
    /// This function provides no synchronization guarantees and so its result should be considered 'out of date'
    /// the instant it is called. Do not use it for synchronization purposes. However, it may be useful as a heuristic.
    #[inline(always)]
    pub fn is_locked(&self) -> bool {
        self.locked.load(Ordering::Relaxed)
    }

    /// Force unlock this [`SpinMutex`].
    ///
    /// # Safety
    ///
    /// This is *extremely* unsafe if the lock is not held by the current
    /// thread. However, this can be useful in some instances for exposing the
    /// lock to FFI that doesn't know how to deal with RAII.
    #[inline(always)]
    pub unsafe fn force_unlock(&self) {
        self.locked.store(false, Ordering::Release);
        L::after_lock();
    }
}

impl<T: ?Sized + fmt::Debug, L: LockAction> fmt::Debug for SpinMutex<T, L> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Some(guard) => write!(f, "Mutex {{ data: ")
                .and_then(|()| (&*guard).fmt(f))
                .and_then(|()| write!(f, "}}")),
            None => write!(f, "Mutex {{ <locked> }}"),
        }
    }
}

impl<T: ?Sized + Default, L: LockAction> Default for SpinMutex<T, L> {
    fn default() -> Self {
        SpinMutex::new(T::default())
    }
}

impl<T, L: LockAction> From<T> for SpinMutex<T, L> {
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<'a, T: ?Sized, L: LockAction> Drop for SpinMutexGuard<'a, T, L> {
    /// The dropping of the SpinMutexGuard will release the lock it was created from.
    fn drop(&mut self) {
        self.lock.store(false, Ordering::Release);
        L::after_lock();
    }
}

impl<'a, T: ?Sized, L: LockAction> Deref for SpinMutexGuard<'a, T, L> {
    type Target = T;
    fn deref(&self) -> &T {
        self.data
    }
}

impl<'a, T: ?Sized, L: LockAction> DerefMut for SpinMutexGuard<'a, T, L> {
    fn deref_mut(&mut self) -> &mut T {
        self.data
    }
}

impl<'a, T: ?Sized + fmt::Debug, L: LockAction> fmt::Debug for SpinMutexGuard<'a, T, L> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized + fmt::Display, L: LockAction> fmt::Display for SpinMutexGuard<'a, T, L> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

#[cfg(feature = "lockapi")]
unsafe impl<L: LockAction> lock_api::RawMutex for SpinMutex<(), L> {
    const INIT: Self = Self::new(());
    type GuardMarker = lock_api::GuardSend;

    fn lock(&self) {
        core::mem::forget(Self::lock(self))
    }

    fn try_lock(&self) -> bool {
        // Prevent guard destructor running
        Self::try_lock(self).map(core::mem::forget).is_some()
    }

    unsafe fn unlock(&self) {
        self.force_unlock();
    }

    fn is_locked(&self) -> bool {
        Self::is_locked(self)
    }
}
