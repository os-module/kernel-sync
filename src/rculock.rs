//! 基于ArcRcu类型的，和RwLock相似的锁。允许读者和写者同时访问。

use core::{marker::PhantomData, ops::{Deref, DerefMut}};

use alloc::rc::Rc;

use crate::{arcrcu::{ArcRcu, Guard}, LockAction};

/// 对ArcRcu的包装，使得其提供和RwLock相似的接口
/// R代表读取数据前后，内核需要执行的操作；W代表写入数据前后，内核需要执行的操作
/// Todo：添加对?Sized的支持
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
        RcuLockReadGuard {
            phantom: PhantomData,
            data: &*(self.rcu),
        }
    }

    pub fn write(&self) -> RcuLockWriteGuard<T, W> {
        W::before_lock();
        loop {
            match self.rcu.try_update() {
                Some(guard) => {
                    return RcuLockWriteGuard {
                        phantom: PhantomData,
                        data: guard,
                        rcu: self.rcu.clone(),
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
                Some(RcuLockWriteGuard {
                    phantom: PhantomData,
                    data: guard,
                    rcu: self.rcu.clone(),
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
}

impl<'a, T: Clone, R: LockAction> Deref for RcuLockReadGuard<'a, T, R> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, T: Clone, R: LockAction> Drop for RcuLockReadGuard<'a, T, R> {
    fn drop(&mut self) {
        R::after_lock();
    }
}

pub struct RcuLockWriteGuard<'a, T: Clone, W: LockAction> {
    phantom: PhantomData<W>,
    data: Guard<'a, T>,
    /// 这个Guard所属的RCU
    rcu: ArcRcu<T>,
}

impl<'a, T: Clone, W: LockAction> Deref for RcuLockWriteGuard<'a, T, W> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &*(self.data)
    }
}

impl<'a, T: Clone, W: LockAction> DerefMut for RcuLockWriteGuard<'a, T, W> {

    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut *(self.data)
    }
}

impl<'a, T: Clone, W: LockAction> Drop for RcuLockWriteGuard<'a, T, W> {
    fn drop(&mut self) {
        // 调用after_lock函数，等待在此之前的所有读者执行完毕
        W::after_lock();
        // 清理之前的版本
        self.rcu.clean();
    }
}



