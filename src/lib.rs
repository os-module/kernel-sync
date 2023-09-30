#![no_std]

extern crate alloc;
pub mod rwlock;

pub use rwlock::*;
pub mod ticket;
pub use ticket::*;

pub mod spin;

pub use spin::*;

cfg_if::cfg_if! {
    if #[cfg(not(feature = "riscv"))]{
        pub struct DefaultLockAction;
        impl LockAction for DefaultLockAction{}
        pub type TicketDefaultMutex<T> = TicketMutex<T,DefaultLockAction>;
        pub type TicketDefaultMutexGuard<'a, T> = TicketMutexGuard<'a, T,DefaultLockAction>;
        pub type SpinDefaultMutex<T> = SpinMutex<T,DefaultLockAction>;
        pub type SpinDefaultMutexGuard<'a, T> = SpinMutexGuard<'a, T,DefaultLockAction>;
        pub type RwDefaultLock<T> = RwLock<T,DefaultLockAction>;
        pub type RwDefaultLockReadGuard<'a, T> = RwLockReadGuard<'a, T,DefaultLockAction>;
        pub type RwDefaultLockWriteGuard<'a, T> = RwLockWriteGuard<'a, T,DefaultLockAction>;
        pub type RwDefaultLockUpgradableReadGuard<'a, T> = RwLockUpgradableReadGuard<'a, T,DefaultLockAction>;
    }else if #[cfg(feature = "riscv")]{
        mod riscv;
        pub use  crate::riscv::KernelLockAction;
        pub type TicketMutex<T> = crate::ticket::TicketMutex<T,KernelLockAction>;
        pub type TicketMutexGuard<'a, T> = crate::ticket::TicketMutexGuard<'a, T,KernelLockAction>;
        pub type SpinMutex<T> = crate::spin::SpinMutex<T,KernelLockAction>;
        pub type SpinMutexGuard<'a, T> = crate::spin::SpinMutexGuard<'a, T,KernelLockAction>;
        pub type RwLock<T> = crate::rwlock::RwLock<T,KernelLockAction>;
        pub type RwLockReadGuard<'a, T> = crate::rwlock::RwLockReadGuard<'a, T,KernelLockAction>;
        pub type RwLockWriteGuard<'a, T> = crate::rwlock::RwLockWriteGuard<'a, T,KernelLockAction>;
        pub type RwLockUpgradableReadGuard<'a, T> = crate::rwlock::RwLockUpgradableGuard<'a, T,KernelLockAction>;
    }
}

/// A trait for lock action
pub trait LockAction {
    fn before_lock() {}
    fn after_lock() {}
}
