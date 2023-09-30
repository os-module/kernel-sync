# kernel-sync

This library is modified from the [spin ](https://github.com/mvdnes/spin-rs)and [kernel-sync]([chyyuu/kernel-sync (gitee.com)](https://gitee.com/chyyuu/kernel-sync)) crates. It adds a new abstract LockAction, allowing kernel implementers to customize the behavior taken when acquiring and releasing locks, such as turning off interrupts and enabling interrupts.

```rust
/// A trait for lock action
pub trait LockAction {
    fn before_lock() {}
    fn after_lock() {}
}
```



## Features

- `SpinMutex`, `TicketMutex`, `RwLock`
- [`lock_api`](https://crates.io/crates/lock_api) compatibility
- `LockAction`



## Example

```rust
struct KernelLockAction;
impl LockAction for KernelLockAction {
    fn before_lock() {
        
    }
    fn after_lock() {
        
    }
}

fn main() {
    type SpinMutex<T> = kernel_sync::SpinMutex<T, KernelLockAction>;
    type TicketMutex<T> = kernel_sync::TicketMutex<T, KernelLockAction>;
    type RwLock<T> = kernel_sync::RwLock<T, KernelLockAction>;
    let x = SpinMutex::new(0);
    *x.lock() = 19;
    assert_eq!(*x.lock(), 19);
    let y = TicketMutex::new(0);
    *y.lock() = 19;
    assert_eq!(*y.lock(), 19);
    let z = RwLock::new(0);
    *z.write() = 19;
    assert_eq!(*z.read(), 19);
}
```



