use std::ops::{Deref, DerefMut};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8};

use reqwest::Url;

use crate::downloader::request::DownloadRequest;

///下载任务句柄, 其实就是一个不可复制的Arc包装类型
pub struct Hander<T> {
    pub(crate) share: Arc<T>,
    pub(crate) end: u64
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
        (Self { share: a }, Self { share: b })
    }

    ///通过检查future中的原子计数来判断任务是否完成
    pub fn future_done(&self) -> bool {
        match Arc::strong_count(&self.share) {
            1 => true,
            2 => false,
            _ => panic!()//unreachable!(),
        }
    }
}

impl<T> Drop for Hander<T> {
    fn drop(&mut self) {
        debug_assert!(self.future_done())
    }
}

///用于可能需要重尝试的任务
pub struct ResumeInfo<T> {
    request: DownloadRequest,
    handler: Hander<T>,
}

impl<T> Deref for ResumeInfo<T> {
    type Target = Hander<T>;

    fn deref(&self) -> &Self::Target {
        &self.handler
    }
    
}

impl<T> DerefMut for ResumeInfo<T> {
    

    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.handler
    }
}


struct BlockWarp<T>{
    share: Arc<T>
}