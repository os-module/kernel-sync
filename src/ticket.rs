//! A ticket-based mutex.
//!
//! Waiting threads take a 'ticket' from the lock in the order they arrive and gain access to the lock when their
//! ticket is next in the queue. Best-case latency is slightly worse than a regular spinning mutex, but worse-case
//! latency is infinitely better. Waiting threads simply need to wait for all threads that come before them in the
//! queue to finish.
//!
use crate::LockAction;
use core::{
    cell::UnsafeCell,
    default::Default,
    fmt,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicUsize, Ordering},
};

/// A spin-based [ticket lock](https://en.wikipedia.org/wiki/Ticket_lock) providing mutually exclusive access to data.
///
/// A ticket lock is analogous to a queue management system for lock requests. When a thread tries to take a lock, it
/// is assigned a 'ticket'. It then spins until its ticket becomes next in line. When the lock guard is released, the
/// next ticket will be processed.
///
/// Ticket locks significantly reduce the worse-case performance of locking at the cost of slightly higher average-time
/// overhead.
///
pub struct TicketMutex<T: ?Sized, L: LockAction> {
    next_ticket: AtomicUsize,
    next_serving: AtomicUsize,
    _marker: core::marker::PhantomData<L>,
    data: UnsafeCell<T>,
}

/// A guard that protects some data.
///
/// When the guard is dropped, the next ticket will be processed.
pub struct TicketMutexGuard<'a, T: ?Sized + 'a, L: LockAction> {
    next_serving: &'a AtomicUsize,
    ticket: usize,
    data: &'a mut T,
    _marker: core::marker::PhantomData<L>,
}

unsafe impl<T: ?Sized + Send, L: LockAction> Sync for TicketMutex<T, L> {}
unsafe impl<T: ?Sized + Send, L: LockAction> Send for TicketMutex<T, L> {}

impl<T, L: LockAction> TicketMutex<T, L> {
    /// Creates a new [`TicketMutex`] wrapping the supplied data.
    ///
    /// # Example
    ///
    /// ```
    /// use kernel_sync::TicketDefaultMutex;
    ///
    /// static MUTEX: TicketDefaultMutex<()> = TicketDefaultMutex::<_>::new(());
    ///
    /// fn demo() {
    ///     let lock = MUTEX.lock();
    ///     // do something with lock
    ///     drop(lock);
    /// }
    /// ```
    #[inline(always)]
    pub const fn new(data: T) -> Self {
        TicketMutex {
            next_ticket: AtomicUsize::new(0),
            next_serving: AtomicUsize::new(0),
            data: UnsafeCell::new(data),
            _marker: core::marker::PhantomData,
        }
    }
    /// Consumes this [`TicketMutex`] and unwraps the underlying data.
    ///
    /// # Example
    ///
    /// ```
    /// let lock = kernel_sync::TicketDefaultMutex::new(42);
    /// assert_eq!(42, lock.into_inner());
    /// ```
    #[inline(always)]
    pub fn into_inner(self) -> T {
        // We know statically that there are no outstanding references to
        // `self` so there's no need to lock.
        self.data.into_inner()
    }
    /// Returns a mutable pointer to the underying data.
    ///
    /// This is mostly meant to be used for applications which require manual unlocking, but where
    /// storing both the lock and the pointer to the inner data gets inefficient.
    ///
    /// # Example
    /// ```
    /// let lock = kernel_sync::TicketDefaultMutex::new(42);
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

impl<T: ?Sized, L: LockAction> TicketMutex<T, L> {
    /// Locks the [`TicketMutex`] and returns a guard that permits access to the inner data.
    ///
    /// The returned data may be dereferenced for data access
    /// and the lock will be dropped when the guard falls out of scope.
    ///
    /// ```
    /// let lock = kernel_sync::TicketDefaultMutex::new(0);
    /// {
    ///     let mut data = lock.lock();
    ///     // The lock is now locked and the data can be accessed
    ///     *data += 1;
    ///     // The lock is implicitly dropped at the end of the scope
    /// }
    /// ```
    #[inline(always)]
    pub fn lock(&self) -> TicketMutexGuard<T, L> {
        L::before_lock();
        let ticket = self.next_ticket.fetch_add(1, Ordering::Relaxed);
        while self.next_serving.load(Ordering::Acquire) != ticket {
            core::hint::spin_loop();
        }
        TicketMutexGuard {
            next_serving: &self.next_serving,
            ticket,
            // Safety
            // We know that we are the next ticket to be served,
            // so there's no other thread accessing the data.
            //
            // Every other thread has another ticket number so it's
            // definitely stuck in the spin loop above.
            data: unsafe { &mut *self.data.get() },
            _marker: Default::default(),
        }
    }
    /// Try to lock this [`TicketMutex`], returning a lock guard if successful.
    ///
    /// # Example
    ///
    /// ```
    /// let lock = kernel_sync::TicketDefaultMutex::new(42);
    ///
    /// let maybe_guard = lock.try_lock();
    /// assert!(maybe_guard.is_some());
    ///
    /// // `maybe_guard` is still held, so the second call fails
    /// let maybe_guard2 = lock.try_lock();
    /// assert!(maybe_guard2.is_none());
    /// ```
    #[inline(always)]
    pub fn try_lock(&self) -> Option<TicketMutexGuard<T, L>> {
        L::before_lock();
        let ticket = self
            .next_ticket
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |ticket| {
                if self.next_serving.load(Ordering::Acquire) == ticket {
                    Some(ticket + 1)
                } else {
                    None
                }
            });
        if let Ok(ticket) = ticket {
            Some(TicketMutexGuard {
                next_serving: &self.next_serving,
                ticket,
                // Safety
                // We have a ticket that is equal to the next_serving ticket, so we know:
                // - that no other thread can have the same ticket id as this thread
                // - that we are the next one to be served so we have exclusive access to the data
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
    /// Since this call borrows the [`TicketMutex`] mutably, and a mutable reference is guaranteed to be exclusive in
    /// Rust, no actual locking needs to take place -- the mutable borrow statically guarantees no locks exist. As
    /// such, this is a 'zero-cost' operation.
    ///
    /// # Example
    ///
    /// ```
    /// let mut lock = kernel_sync::TicketDefaultMutex::new(0);
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
        let ticket = self.next_ticket.load(Ordering::Relaxed);
        self.next_serving.load(Ordering::Relaxed) != ticket
    }

    /// Force unlock this [`TicketMutex`], by serving the next ticket.
    ///
    /// # Safety
    ///
    /// This is *extremely* unsafe if the lock is not held by the current
    /// thread. However, this can be useful in some instances for exposing the
    /// lock to FFI that doesn't know how to deal with RAII.
    #[inline(always)]
    pub unsafe fn force_unlock(&self) {
        self.next_serving.fetch_add(1, Ordering::Release);
        L::after_lock()
    }
}

impl<'a, T: ?Sized, L: LockAction> Drop for TicketMutexGuard<'a, T, L> {
    /// The dropping of the TicketMutexGuard will release the lock it was created from.
    fn drop(&mut self) {
        let new_ticket = self.ticket + 1;
        self.next_serving.store(new_ticket, Ordering::Release);
        L::after_lock()
    }
}

impl<T: ?Sized + fmt::Debug, L: LockAction> fmt::Debug for TicketMutex<T, L> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Some(guard) => write!(f, "Mutex {{ data: ")
                .and_then(|()| (&*guard).fmt(f))
                .and_then(|()| write!(f, "}}")),
            None => write!(f, "Mutex {{ <locked> }}"),
        }
    }
}

impl<T: ?Sized + Default, L: LockAction> Default for TicketMutex<T, L> {
    fn default() -> Self {
        TicketMutex::new(T::default())
    }
}

impl<T, L: LockAction> From<T> for TicketMutex<T, L> {
    fn from(data: T) -> Self {
        Self::new(data)
    }
}

impl<'a, T: ?Sized + fmt::Display, L: LockAction> fmt::Display for TicketMutexGuard<'a, T, L> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized + fmt::Debug, L: LockAction> fmt::Debug for TicketMutexGuard<'a, T, L> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized, L: LockAction> Deref for TicketMutexGuard<'a, T, L> {
    type Target = T;
    fn deref(&self) -> &T {
        self.data
    }
}

impl<'a, T: ?Sized, L: LockAction> DerefMut for TicketMutexGuard<'a, T, L> {
    fn deref_mut(&mut self) -> &mut T {
        self.data
    }
}
#[cfg(feature = "lockapi")]
unsafe impl<L: LockAction> lock_api::RawMutex for TicketMutex<(), L> {
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
