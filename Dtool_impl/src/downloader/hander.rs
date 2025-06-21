use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, AtomicBool};

///其实就是一个不可复制的Arc包装类型
pub struct Hander<T> {
    share: Arc<T>
}

impl<T> Deref for Hander<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.share
    }
}

impl<T> Hander<T> {

    pub fn new(inner: T) -> (Self, Self) {
        let a = Arc::new(inner);
        let b = a.clone();
        (Self{share: a}, Self{share: b})
    }

    ///通过检查future中的原子计数来判断任务是否完成
    pub fn future_done(&self) -> bool{
        match self.share.strong_count(){
            1 => true,
            2 => false,
            _ => unreachable!()
        }
    }

}

impl<T> Drop for Hander<T> {
    fn drop(&mut self) {
        debug_assert!(self.future_done())
    }
}