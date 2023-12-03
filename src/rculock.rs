//! 基于ArcRcu类型的，和RwLock相似的锁。允许读者和写者同时访问。

use core::{marker::PhantomData, ops::{Deref, DerefMut}};
use core::mem::swap;
use core::sync::atomic::Ordering;
use crate::{arcrcu::{ArcRcu, Guard}, LockAction};

/// 对ArcRcu的包装，使得其提供和RwLock相似的接口
/// R代表读取数据前后，内核需要执行的操作；W代表写入数据前后，内核需要执行的操作
/// SAFETY: W中的after_lock函数需要等待写者前的所有读者结束，这样才能正常释放旧版本数据的空间。
pub struct RcuLock<T: Clone, R: LockAction, W: LockAction> {
    phantom_r: PhantomData<R>,
    phantom_w: PhantomData<W>,
    rcu: ArcRcu<T>,
}

unsafe impl<T: Clone + Send + Sync, R: LockAction, W: LockAction> Send for RcuLock<T, R, W> {}
unsafe impl<T: Clone + Send + Sync, R: LockAction, W: LockAction> Sync for RcuLock<T, R, W> {}

impl<T: Clone, R: LockAction, W: LockAction> Clone for RcuLock<T, R, W> {
    fn clone(&self) -> Self {
        Self {
            phantom_r: PhantomData,
            phantom_w: PhantomData,
            rcu: self.rcu.clone()
        }
    }
}

impl<T: Clone, R: LockAction, W: LockAction> RcuLock<T, R, W> {
    pub fn new(data: T) -> Self {
        RcuLock { 
            phantom_r: PhantomData,
            phantom_w: PhantomData,
            rcu: ArcRcu::new(data),
        }
    }

    /// read必定成功，因此没有try_read
    pub fn read(&self) -> RcuLockReadGuard<T, R> {
        R::before_lock();
        let index = self.rcu.inner.current_borrow_count_index.load(Ordering::Acquire);
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

    pub fn write(&self) -> RcuLockWriteGuard<T, W> {
        W::before_lock();
        loop {
            match self.rcu.try_update() {
                Some(guard) => {
                    let index = self.rcu.inner.current_borrow_count_index.load(Ordering::Acquire);
                    self.rcu.inner.borrow_count[index].fetch_add(1, Ordering::AcqRel);
                    // let count = self.rcu.inner.borrow_count[index].load(Ordering::Acquire);
                    // std::println!("write, index = {index}, count = {} -> {count}", count - 1);
                    return RcuLockWriteGuard {
                        phantom: PhantomData,
                        data: Some(guard),
                        rcu: &self.rcu,
                        borrow_count_index: index,
                    }
                },
                None => {
                    core::hint::spin_loop();
                }
            }
        }
    }

    pub fn try_write(&self) -> Option<RcuLockWriteGuard<T, W>> {
        W::before_lock();
        match self.rcu.try_update() {
            Some(guard) => {
                let index = self.rcu.inner.current_borrow_count_index.load(Ordering::Acquire);
                self.rcu.inner.borrow_count[index].fetch_add(1, Ordering::AcqRel);
                // let count = self.rcu.inner.borrow_count[index].load(Ordering::Acquire);
                // std::println!("try_write, index = {index}, count = {} -> {count}", count - 1);
                Some(RcuLockWriteGuard {
                    phantom: PhantomData,
                    data: Some(guard),
                    rcu: &self.rcu,
                    borrow_count_index: index,
                })
            },
            None => {
                W::after_lock();
                None
            }
        }
    }
}

/// 对读取RCU获得的结构的封装，目前这层封装是为了调用R的方法
pub struct RcuLockReadGuard<'a, T: Clone, R: LockAction> {
    phantom: PhantomData<R>,
    data: &'a T,
    rcu: &'a ArcRcu<T>,
    borrow_count_index: usize,
}

impl<'a, T: Clone, R: LockAction> Deref for RcuLockReadGuard<'a, T, R> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T: Clone, R: LockAction> Drop for RcuLockReadGuard<'a, T, R> {
    fn drop(&mut self) {
        self.rcu.inner.borrow_count[self.borrow_count_index].fetch_sub(1, Ordering::AcqRel);
        // let count = self.rcu.inner.borrow_count[self.borrow_count_index].load(Ordering::Acquire);
        // std::println!("read drop, index = {}, count = {} -> {count}", self.borrow_count_index, count + 1);
        R::after_lock();
    }
}

pub struct RcuLockWriteGuard<'a, T: Clone, W: LockAction> {
    phantom: PhantomData<W>,
    data: Option<Guard<'a, T>>,
    /// 这个Guard所属的RCU
    rcu: &'a ArcRcu<T>,
    borrow_count_index: usize,
}

impl<'a, T: Clone, W: LockAction> Deref for RcuLockWriteGuard<'a, T, W> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match &self.data {
            Some(guard) => {
                &*guard
            },
            None => {
                panic!("unreachable76543212345");
            }
        }
    }
}

impl<'a, T: Clone, W: LockAction> DerefMut for RcuLockWriteGuard<'a, T, W> {

    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.data {
            Some(guard) => {
                &mut *guard
            },
            None => {
                panic!("unreachable0989876678");
            }
        }
    }
}

impl<'a, T: Clone, W: LockAction> Drop for RcuLockWriteGuard<'a, T, W> {
    fn drop(&mut self) {
        // 需要提前释放guard，这样才能使更改生效
        let mut guard: Option<Guard<T>> = None;
        swap(&mut guard, &mut (self.data));
        drop(guard.unwrap());
        // 将current_borrow_count_index在0和1间切换
        // 这样，更新数据后的读取就不会影响到这个引用计数了
        self.rcu.inner.current_borrow_count_index.fetch_xor(1, Ordering::AcqRel);
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
        W::after_lock();
    }
}



