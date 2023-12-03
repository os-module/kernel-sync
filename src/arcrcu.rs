use core::cell::{Cell, UnsafeCell};
use core::{ops, borrow, ptr, mem};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use std::fmt::Debug;
use alloc::boxed::Box;
use alloc::sync::Arc;

/// Based on [droundy/rcu-clean/arcrcu.rs](https://github.com/droundy/rcu-clean) on Github.
/// 
/// A thread-safe reference counted pointer that allows interior mutability
/// 
/// The [ArcRcu] is functionally roughly equivalent to
/// `Arc<RwLock<T>>`, except that reads (of the old value) may happen
/// while a write is taking place.  Reads on an [ArcRcu] are much
/// faster (by a factor of 2 or 3) than reads on either a
/// `Arc<RwLock<T>>` or a `Arc<Mutex<T>>`.  So in this case you gain
/// both ergonomics and read speed.  Writes are slow, so only use this
/// type if writes are rare (or their speed doesn't matter).

/// ```
/// let x = rcu_clean::ArcRcu::new(3);
/// let y: &usize = &(*x);
/// let z = x.clone();
/// *x.update() = 7; // Wow, we are mutating something we have borrowed!
/// assert_eq!(*y, 3); // the old reference is still valid.
/// assert_eq!(*x, 7); // but the pointer now points to the new value.
/// assert_eq!(*z, 7); // but the cloned pointer also points to the new value.
/// ```
/// 
/// Todo：改一下borrow_count机制，现在只要有读者或写者在占用这个锁，就无法释放旧版本的数据。需要改成Grace Period那样的。
pub struct ArcRcu<T> {
    pub inner: Arc<Inner<T>>,
    have_borrowed: Cell<bool>,
}
unsafe impl<T: Send + Sync> Send for ArcRcu<T> {}
unsafe impl<T: Send + Sync> Sync for ArcRcu<T> {}
impl<T: Clone> Clone for ArcRcu<T> {
    fn clone(&self) -> Self {
        ArcRcu {
            inner: self.inner.clone(),
            have_borrowed: Cell::new(false),
        }
    }
}
pub struct Inner<T> {
    borrow_count: AtomicUsize,
    pub am_writing: AtomicBool,
    list: List<T>,
}

pub struct List<T> {
    value: UnsafeCell<T>,
    next: AtomicPtr<List<T>>,
}

impl<T> ops::Deref for ArcRcu<T> {
    type Target = T;
    fn deref(&self) -> &T {
        let aleady_borrowed = self.have_borrowed.get();
        if !aleady_borrowed {
            self.inner.borrow_count.fetch_add(1, Ordering::Relaxed);
            self.have_borrowed.set(true); // indicate we have borrowed this once.
        }
        let next = self.inner.list.next.load(Ordering::Acquire);
        if next == null_mut() {
            unsafe { &*self.inner.list.value.get() }
        } else {
            unsafe { &*(*next).value.get() }
        }
    }
}
impl<T> borrow::Borrow<T> for ArcRcu<T> {
    fn borrow(&self) -> &T {
        &*self
    }
}
impl<T> Drop for List<T> {
    fn drop(&mut self) {
        let next = self.next.load(Ordering::Acquire);
        if next != null_mut() {
            let _free_this = unsafe { Box::from_raw(next) };
        }
    }
}

impl<T> Debug for List<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("List").field("next", &self.next).finish()
    }
}

impl<'a, T: Clone> ArcRcu<T> {
    pub fn new(x: T) -> Self {
        ArcRcu {
            have_borrowed: Cell::new(false),
            inner: Arc::new(Inner {
                borrow_count: AtomicUsize::new(0),
                am_writing: AtomicBool::new(false),
                list: List {
                    value: UnsafeCell::new(x),
                    next: AtomicPtr::new(null_mut()),
                },
            }),
        }
    }
    pub fn try_update(&'a self) -> Option<Guard<'a, T>> {
        if self.inner.am_writing.swap(true, Ordering::Relaxed) {
            None
        }
        else {
            Some(Guard {
                list: Some(List {
                    value: UnsafeCell::new((*(*self)).clone()),
                    next: AtomicPtr::new(self.inner.list.next.load(Ordering::Acquire)),
                }),
                rc_guts: &self.inner,
            })
        }
    }
    pub fn clean(&self) {
        let aleady_borrowed = self.have_borrowed.get();
        if aleady_borrowed {
            self.inner.borrow_count.fetch_sub(1, Ordering::Relaxed);
            self.have_borrowed.set(false); // indicate we have no longer borrowed this.
        }
        let borrow_count = self.inner.borrow_count.load(Ordering::Relaxed);
        let next = self.inner.list.next.load(Ordering::Acquire);
        std::println!("clean?");
        // if borrow_count == 0 && next != null_mut() {
        if next != null_mut() {
            std::println!("clean.");
            unsafe {
                // make a copy of the old datum that we will need to free
                let buffer: UnsafeCell<Option<T>> = UnsafeCell::new(None);
                ptr::copy_nonoverlapping(
                    self.inner.list.value.get(),
                    buffer.get() as *mut T,
                    1,
                );
                // std::println!("clean 1");
                // now copy the "good" value to the main spot
                ptr::copy_nonoverlapping((*next).value.get(), self.inner.list.value.get(), 1);
                // std::println!("clean 2");
                // Now we can set the pointer to null which activates
                // the copy we just made.
                let _to_be_freed =
                    Box::from_raw(self.inner.list.next.swap(null_mut(), Ordering::Release));
                // std::println!("{:?}", _to_be_freed);
                ptr::copy_nonoverlapping(buffer.get() as *mut T, (*next).value.get(), 1);
                // std::println!("clean 3");
                let buffer_copy: UnsafeCell<Option<T>> = UnsafeCell::new(None);
                ptr::copy_nonoverlapping(buffer_copy.get(), buffer.get(), 1);
                // std::println!("clean 4");
                // std::println!("{:?}", _to_be_freed);
            }
        }
    }
}

pub struct Guard<'a, T: Clone> {
    list: Option<List<T>>,
    rc_guts: &'a Inner<T>,
}
impl<'a, T: Clone> ops::Deref for Guard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        if let Some(ref list) = self.list {
            unsafe { &*list.value.get() }
        } else {
            unreachable!()
        }
    }
}
impl<'a, T: Clone> ops::DerefMut for Guard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        if let Some(ref list) = self.list {
            unsafe { &mut *list.value.get() }
        } else {
            unreachable!()
        }
    }
}
impl<'a, T: Clone> Drop for Guard<'a, T> {
    fn drop(&mut self) {
        let list = mem::replace(&mut self.list, None);
        self.rc_guts
            .list
            .next
            .store(Box::into_raw(Box::new(list.unwrap())), Ordering::Release);
        // self.rc_guts.am_writing.store(false, Ordering::Relaxed);
    }
}