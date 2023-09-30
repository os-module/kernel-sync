use kernel_sync::LockAction;
use riscv::register::sstatus;
use std::cell::{RefCell, RefMut};

fn cpu_id() -> u8 {
    let cpu_id: usize;
    // unsafe {
    // core::arch::asm!("mv {0}, tp", out(reg) cpu_id);
    cpu_id = 0;
    // }
    cpu_id as u8
}
fn intr_on() {
    unsafe { sstatus::set_sie() };
}
fn intr_off() {
    unsafe { sstatus::clear_sie() };
}
fn intr_get() -> bool {
    sstatus::read().sie()
}

#[derive(Debug, Default, Clone, Copy)]
#[repr(align(64))]
pub struct Cpu {
    pub noff: i32,              // Depth of push_off() nesting.
    pub interrupt_enable: bool, // Were interrupts enabled before push_off()?
}

impl Cpu {
    const fn new() -> Self {
        Self {
            noff: 0,
            interrupt_enable: false,
        }
    }
}

pub struct SafeRefCell<T>(RefCell<T>);

// #Safety: Only the corresponding cpu will access it.
unsafe impl<Cpu> Sync for SafeRefCell<Cpu> {}

impl<T> SafeRefCell<T> {
    const fn new(t: T) -> Self {
        Self(RefCell::new(t))
    }
}

// Avoid hard code
#[allow(clippy::declare_interior_mutable_const)]
const DEFAULT_CPU: SafeRefCell<Cpu> = SafeRefCell::new(Cpu::new());

const MAX_CORE_NUM: usize = 4;

static CPUS: [SafeRefCell<Cpu>; MAX_CORE_NUM] = [DEFAULT_CPU; MAX_CORE_NUM];

pub fn mycpu() -> RefMut<'static, Cpu> {
    CPUS[cpu_id() as usize].0.borrow_mut()
}

// push_off/pop_off are like intr_off()/intr_on() except that they are matched:
// it takes two pop_off()s to undo two push_off()s.  Also, if interrupts
// are initially off, then push_off, pop_off leaves them off.
pub(crate) fn push_off() {
    let old = intr_get();
    intr_off();
    let mut cpu = mycpu();
    if cpu.noff == 0 {
        cpu.interrupt_enable = old;
    }
    cpu.noff += 1;
}

pub(crate) fn pop_off() {
    let mut cpu = mycpu();
    if intr_get() || cpu.noff < 1 {
        panic!("pop_off");
    }
    cpu.noff -= 1;
    let should_enable = cpu.noff == 0 && cpu.interrupt_enable;
    drop(cpu);
    // NOTICE: intr_on() may lead to an immediate inerrupt, so we *MUST* drop(cpu) in advance.
    if should_enable {
        intr_on();
    }
}

struct KernelLockAction;
impl LockAction for KernelLockAction {
    fn before_lock() {
        push_off();
    }
    fn after_lock() {
        pop_off();
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
