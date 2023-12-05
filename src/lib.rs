#![no_std]

extern crate alloc;
pub mod rwlock;

mod arcrcu;
pub mod rculock;
pub mod ticket;
pub mod spin;



pub type TicketMutex<T> = ticket::TicketMutex<T,EmptyLockAction>;
pub type TicketMutexGuard<'a, T> = ticket::TicketMutexGuard<'a, T,EmptyLockAction>;
pub type SpinMutex<T> = spin::SpinMutex<T,EmptyLockAction>;
pub type SpinMutexGuard<'a, T> = spin::SpinMutexGuard<'a, T,EmptyLockAction>;
pub type RwLock<T> = rwlock::RwLock<T,EmptyLockAction>;
pub type RwLockReadGuard<'a, T> = rwlock::RwLockReadGuard<'a, T,EmptyLockAction>;
pub type RwLockWriteGuard<'a, T> = rwlock::RwLockWriteGuard<'a, T,EmptyLockAction>;
pub type RwLockUpgradableGuard<'a, T> = rwlock::RwLockUpgradableGuard<'a, T,EmptyLockAction>;
pub type RcuLock<T> = rculock::RcuLock<T, EmptyLockAction>;
pub type RcuLockReadGuard<'a, T> = rculock::RcuLockReadGuard<'a, T, EmptyLockAction>;
pub type RcuLockWriteGuard<'a, T> = rculock::RcuLockWriteGuard<'a, T, EmptyLockAction>;
pub struct EmptyLockAction;
impl LockAction for EmptyLockAction {}



/// A trait for lock action
pub trait LockAction {
    fn before_lock() {}
    fn after_lock() {}
}


