//!多线程下载器
//! 在Range为0-时不可用
use crate::downloader::core::{ProgressRecorder, RemainGetter};
use crate::downloader::segment::SegmentIter;

//use super::core::download_with_check_remain;
use super::request::DownloadRequest;
use std::ops::Deref;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::{sync::atomic::AtomicU64};
use Dtool_core::ProcessSender;
use parking_lot::{Mutex, RwLock};
use tokio;

use super::segment::{Segment, SplitSegment};


// type BlockShare = Hander<Inner>;
// struct Inner {
//     remain: AtomicU64,
//     //end: u64,
// }

// impl Hander<Inner> {
//     fn start_unchecked(&self) {}

//     fn start(&self) {
//         assert!(self.future_done());
//         self.start_unchecked()
//     }
// }

struct Hander<T>{
    pub remain: Remain,
    pub end: u64,
    pub ext: T
}

impl Hander<T> {
    pub fn process(&self) -> u64 {
        self.end - self.remain.load(Ordering::Acquire)
    }

    pub fn split(&mut self, new_remain: u64) -> SplitSegment {
        self.
    }
}



///不可复制的包装器
struct Remain(Arc<AtomicU64>);

impl Remain {
    fn only_one_owner(&self) -> bool{
        Arc::strong_count(&self.0) == 1
    }

    fn make_two_copy(remain: u64) -> (Self, Self) {
        let remain = Arc::new(AtomicU64::new(remain));
        (Self(remain.clone()), Self(remain))
    }
}

impl Deref for Remain {
    type Target = AtomicU64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl ProgressRecorder for Remain {
    fn update_progress(&mut self, chunk_len: usize) {
        
    }
}

impl RemainGetter for Remain {
    fn get_remain(&mut self) -> u64 {
        self.0.load(Ordering::Acquire)
    }
}

///多线程下载执行器
struct RunningBlocks<T> {
    pub inner: Vec<Hander<T>>,
}

impl RunningBlocks<T> {
    fn new() -> Self{
        Self { inner: Vec::new() }
    }

    fn new_with_segment_iter<T>(iter: SegmentIter, execter: impl FnMut(Arc<AtomicU64>) -> T) -> Self {
        //let this = Self::new();
        //let inner = Vec::from_iter(iter)
    }

    fn into_segments(self) -> Vec<Segment>{
        todo!()
    }

    fn split_max_running_blocks(&mut self) -> Option<_> {
        todo!()
    }


    pub fn request(&self) -> &DownloadRequest {
        &self.request
    }

    pub fn request_mut(&mut self) -> &mut DownloadRequest {
        &mut self.request
    }

    // pub fn push_segment(&mut self, segment: Segment) -> Remain {
    //     self.inner.push(value);
    // }

    pub fn add(&mut self, hander: Hander<_>){
        self.inner.push(hander);
    }

    pub fn remove(&mut self, index: usize ) -> Hander<_>{
        self.inner.swap_remove(index)
    }

    // pub fn unlock_mutex<T>(this: Mutex<Self>, func: impl FnOnce(&mut Self) -> T) -> T{
    //     let mut guard = this.lock();
    //     func(&mut guard)
    // }

    // pub fn read<T>(this: RwLock<Self>, func: impl FnOnce(&Self) -> T) -> T{
    //     let guard = this.read();
    //     func(&guard)
    // }

    // pub fn write<T>(this: RwLock<Self>, func: impl FnOnce(&mut Self) -> T) -> T{
    //     let mut guard = this.write();
    //     func(&mut guard)
    // }
}


// struct BlockIter {
//     current_pos: u64,
//     total_size: u64,
// }

// impl BlockIter {
//     fn next_block(&mut self, block_size: u64) -> Option<BlockBuilder> {
//         if self.current_pos < self.total_size {
//             let block = BlockBuilder::new(self.current_pos, self.current_pos + block_size);
//             self.current_pos += block_size;
//             Some(block)
//         } else {
//             None
//         }
//     }
// }
