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
