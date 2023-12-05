//! 基于ArcRcu类型的，和RwLock相似的锁。允许读者和写者同时访问。

use crate::{
    arcrcu::{ArcRcu, Guard},
    LockAction,
};
use core::fmt::Debug;
use core::mem::swap;
use core::sync::atomic::Ordering;
use core::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

/// 对ArcRcu的包装，使得其提供和RwLock相似的接口
/// 该锁本身具备了Arc的性质，RcuLock<T>类似于Arc<RwLock<T>>。
/// 使用引用计数机制来实现RCU。
/// 使用长度为2的数组来记录引用计数。初始时，在位置0记录引用计数。写者每更新一次数据，就将记录引用计数的位置在0和1之间切换一次。
/// 这样，更新后的读者就不会影响到这个写者的宽限期（grace peroid）了，其只需等待写者之前的读者完成，然后释放旧版本的数据即可。
/// 最好在L中实现关中断，这样可以避免将某些更新后的读者划到写者的宽限期。

pub struct RcuLock<T: Clone, L: LockAction> {
    phantom: PhantomData<L>,
    rcu: ArcRcu<T>,
}

impl<T: Clone + Debug, L: LockAction> Debug for RcuLock<T, L> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RcuLock").field("rcu", &self.rcu).finish()
    }
}

unsafe impl<T: Clone + Send + Sync, L: LockAction> Send for RcuLock<T, L> {}
unsafe impl<T: Clone + Send + Sync, L: LockAction> Sync for RcuLock<T, L> {}

impl<T: Clone, L: LockAction> Clone for RcuLock<T, L> {
    fn clone(&self) -> Self {
        Self {
            phantom: PhantomData,
            rcu: self.rcu.clone(),
        }
    }
}

impl<T: Clone, L: LockAction> RcuLock<T, L> {
    pub fn new(data: T) -> Self {
        RcuLock {
            phantom: PhantomData,
            rcu: ArcRcu::new(data),
        }
    }

    pub fn read(&self) -> RcuLockReadGuard<T, L> {
        L::before_lock();
        let index = self
            .rcu
            .inner
            .current_borrow_count_index
            .load(Ordering::Acquire);
        self.rcu.inner.borrow_count[index].fetch_add(1, Ordering::AcqRel);
        // let count = self.rcu.inner.borrow_count[index].load(Ordering::Acquire);
        // std::println!("read, index = {index}, count = {} -> {count}", count - 1);
        RcuLockReadGuard {
            phantom: PhantomData,
            data: &*(self.rcu),
            rcu: &self.rcu,
            borrow_count_index: index,
        }
    }

    pub fn write(&self) -> RcuLockWriteGuard<T, L> {
        L::before_lock();
        loop {
            match self.rcu.try_update() {
                Some(guard) => {
                    let index = self
                        .rcu
                        .inner
                        .current_borrow_count_index
                        .load(Ordering::Acquire);
                    self.rcu.inner.borrow_count[index].fetch_add(1, Ordering::AcqRel);
                    // let count = self.rcu.inner.borrow_count[index].load(Ordering::Acquire);
                    // std::println!("write, index = {index}, count = {} -> {count}", count - 1);
                    return RcuLockWriteGuard {
                        phantom: PhantomData,
                        data: Some(guard),
                        rcu: &self.rcu,
                        borrow_count_index: index,
                    };
                }
                None => {
                    core::hint::spin_loop();
                }
            }
        }
    }

    pub fn try_write(&self) -> Option<RcuLockWriteGuard<T, L>> {
        L::before_lock();
        match self.rcu.try_update() {
            Some(guard) => {
                let index = self
                    .rcu
                    .inner
                    .current_borrow_count_index
                    .load(Ordering::Acquire);
                self.rcu.inner.borrow_count[index].fetch_add(1, Ordering::AcqRel);
                // let count = self.rcu.inner.borrow_count[index].load(Ordering::Acquire);
                // std::println!("try_write, index = {index}, count = {} -> {count}", count - 1);
                Some(RcuLockWriteGuard {
                    phantom: PhantomData,
                    data: Some(guard),
                    rcu: &self.rcu,
                    borrow_count_index: index,
                })
            }
            None => {
                L::after_lock();
                None
            }
        }
    }
}

/// 对读取RCU获得的结构的封装，目前这层封装是为了调用R的方法，以及维护引用计数
pub struct RcuLockReadGuard<'a, T: Clone, L: LockAction> {
    phantom: PhantomData<L>,
    data: &'a T,
    rcu: &'a ArcRcu<T>,
    borrow_count_index: usize,
}

impl<'a, T: Clone, L: LockAction> Deref for RcuLockReadGuard<'a, T, L> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T: Clone, L: LockAction> Drop for RcuLockReadGuard<'a, T, L> {
    fn drop(&mut self) {
        self.rcu.inner.borrow_count[self.borrow_count_index].fetch_sub(1, Ordering::AcqRel);
        // let count = self.rcu.inner.borrow_count[self.borrow_count_index].load(Ordering::Acquire);
        // std::println!("read drop, index = {}, count = {} -> {count}", self.borrow_count_index, count + 1);
        L::after_lock();
    }
}

pub struct RcuLockWriteGuard<'a, T: Clone, L: LockAction> {
    phantom: PhantomData<L>,
    data: Option<Guard<'a, T>>,
    /// 这个Guard所属的RCU
    rcu: &'a ArcRcu<T>,
    borrow_count_index: usize,
}

impl<'a, T: Clone, L: LockAction> Deref for RcuLockWriteGuard<'a, T, L> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match &self.data {
            Some(guard) => guard,
            None => {
                panic!("unreachable76543212345");
            }
        }
    }
}

impl<'a, T: Clone, L: LockAction> DerefMut for RcuLockWriteGuard<'a, T, L> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.data {
            Some(guard) => &mut *guard,
            None => {
                panic!("unreachable0989876678");
            }
        }
    }
}

impl<'a, T: Clone, L: LockAction> Drop for RcuLockWriteGuard<'a, T, L> {
    fn drop(&mut self) {
        // 需要提前释放guard，这样才能使更改生效
        let mut guard: Option<Guard<T>> = None;
        swap(&mut guard, &mut (self.data));
        drop(guard.unwrap());
        // 将current_borrow_count_index在0和1间切换
        // 这样，更新数据后的读取就不会影响到这个引用计数了
        self.rcu
            .inner
            .current_borrow_count_index
            .fetch_xor(1, Ordering::AcqRel);
        // 下降引用计数
        self.rcu.inner.borrow_count[self.borrow_count_index].fetch_sub(1, Ordering::AcqRel);
        // let count = self.rcu.inner.borrow_count[self.borrow_count_index].load(Ordering::Acquire);
        // std::println!("write drop, index = {}, count = {} -> {count}", self.borrow_count_index, count + 1);
        // 等待在此之前的所有读者执行完毕
        while self.rcu.inner.borrow_count[self.borrow_count_index].load(Ordering::Acquire) > 0 {
            core::hint::spin_loop();
        }
        // 清理之前的版本
        self.rcu.clean();
        // 释放写者锁
        self.rcu.inner.am_writing.store(false, Ordering::Relaxed);
        L::after_lock();
    }
}
