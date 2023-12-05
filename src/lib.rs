#![no_std]

extern crate alloc;
pub mod rwlock;

mod arcrcu;
pub mod rculock;
pub use rculock::*;

pub use rwlock::*;
pub mod ticket;
pub use ticket::*;

pub mod spin;

pub use spin::*;

cfg_if::cfg_if! {
    if #[cfg(feature = "riscv")]{
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

        pub type RcuLock<T> = crate::rculock::RcuLock<T, KernelLockAction>;
        pub type RcuLockReadGuard<'a, T> = crate::rculock::RcuLockReadGuard<'a, T, KernelLockAction>;
        pub type RcuLockWriteGuard<'a, T> = crate::rculock::RcuLockWriteGuard<'a, T, KernelLockAction>;
    }else{
        pub type TicketMutex<T> = crate::ticket::TicketMutex<T,EmptyLockAction>;
        pub type TicketMutexGuard<'a, T> = crate::ticket::TicketMutexGuard<'a, T,EmptyLockAction>;
        pub type SpinMutex<T> = crate::spin::SpinMutex<T,EmptyLockAction>;
        pub type SpinMutexGuard<'a, T> = crate::spin::SpinMutexGuard<'a, T,EmptyLockAction>;
        pub type RwLock<T> = crate::rwlock::RwLock<T,EmptyLockAction>;
        pub type RwLockReadGuard<'a, T> = crate::rwlock::RwLockReadGuard<'a, T,EmptyLockAction>;
        pub type RwLockWriteGuard<'a, T> = crate::rwlock::RwLockWriteGuard<'a, T,EmptyLockAction>;
        pub type RwLockUpgradableReadGuard<'a, T> = crate::rwlock::RwLockUpgradableGuard<'a, T,EmptyLockAction>;

        pub type RcuLock<T> = crate::rculock::RcuLock<T, EmptyLockAction>;
        pub type RcuLockReadGuard<'a, T> = crate::rculock::RcuLockReadGuard<'a, T, EmptyLockAction>;
        pub type RcuLockWriteGuard<'a, T> = crate::rculock::RcuLockWriteGuard<'a, T, EmptyLockAction>;
    }
}

/// A trait for lock action
pub trait LockAction {
    fn before_lock() {}
    fn after_lock() {}
}

pub struct EmptyLockAction;
impl LockAction for EmptyLockAction {}
