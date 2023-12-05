# kernel-sync

This library is modified from the [spin ](https://github.com/mvdnes/spin-rs), [kernel-sync](https://gitee.com/chyyuu/kernel-sync) and [rcu-clean](https://github.com/droundy/rcu-clean) crates. It adds a new abstract LockAction, allowing kernel implementers to customize the behavior taken when acquiring and releasing locks, such as turning off interrupts and enabling interrupts.

```rust
/// A trait for lock action
pub trait LockAction {
    fn before_lock() {}
    fn after_lock() {}
}
```



## Features

- `SpinMutex`, `TicketMutex`, `RwLock`, `RcuLock`
- [`lock_api`](https://crates.io/crates/lock_api) compatibility
- `LockAction`



## Example
enable LockAction for riscv
```
kernel-sync = {git = "https://github.com/os-module/kernel-sync"}
```

```rust
use kernel_sync::{LockAction, rwlock::RwLock, spin::SpinMutex, ticket::TicketMutex};
pub struct KernelLockAction;
impl LockAction for KernelLockAction {
    fn before_lock() {
        // push_off(); //disable interrupt
    }
    fn after_lock() {
        // pop_off(); //enable interrupt
    }
}

fn main() {
    let x = SpinMutex::<_,KernelLockAction>::new(0);
    *x.lock() = 19;
    assert_eq!(*x.lock(), 19);
    let y = TicketMutex::<_,KernelLockAction>::new(0);
    *y.lock() = 19;
    assert_eq!(*y.lock(), 19);
    let z = RwLock::<_,KernelLockAction>::new(0);
    *z.write() = 19;
    assert_eq!(*z.read(), 19);
}
```



