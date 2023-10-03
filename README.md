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
enable LockAction for riscv
```
kernel-sync = {path = ".",features = ["riscv"]}
```

```rust
use kernel_sync::{SpinMutex, TicketMutex, RwLock};
fn main() {
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



