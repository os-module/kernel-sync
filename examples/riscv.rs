use kernel_sync::{RwLock, SpinMutex, TicketMutex};

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
