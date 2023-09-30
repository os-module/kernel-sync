#![no_std]

extern crate alloc;
pub mod rwlock;

pub use rwlock::*;
pub mod ticket;
pub use ticket::*;
pub mod spin;
pub use spin::*;

cfg_if::cfg_if! {
    if #[cfg(not(feature = "kernel"))]{
        pub struct DefaultLockAction;
        impl LockAction for DefaultLockAction{}
        pub type TicketDefaultMutex<T> = TicketMutex<T,DefaultLockAction>;
        pub type SpinDefaultMutex<T> = SpinMutex<T,DefaultLockAction>;
        pub type RwDefaultLock<T> = RwLock<T,DefaultLockAction>;
    }
}

/// A trait for lock action
pub trait LockAction {
    fn before_lock() {}
    fn after_lock() {}
}
