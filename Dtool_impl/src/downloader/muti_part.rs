
use crate::downloader::segment::SegmentIter;

//use super::core::download_with_check_remain;
use super::httprequest::RequestInfo;
use std::cell::UnsafeCell;
use std::mem::ManuallyDrop;
use std::num::NonZeroU64;
use std::ops::{Deref, Index};
use std::ptr;
use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{sync::atomic::AtomicU64};
use parking_lot::{Mutex, RwLock};
use tokio;
use tokio::fs::File;
use tokio::task::{AbortHandle, JoinSet, JoinHandle};

use super::segment::{Segment};


use futures::task::AtomicWaker;
use std::future::Future;
use std::task::{Context, Poll};

pub struct Downloader {
    share: Arc<Share>,
    length: u64,
}

impl Downloader {
    pub fn process(&self) -> u64 {
        self.share.process.load(Ordering::Relaxed)
    }
}

struct Share{
    locked: Mutex<LockedShare>,
    waker: AtomicWaker,
    process: AtomicU64,
    requests: RequestInfo,
    //file: File,
    target_task_num: AtomicUsize,
    task_num: AtomicUsize,
}

struct LockedShare {
    slots: Vec<Slot>, //or named running_slots?
    idie_segments: Vec<Segment>,
}


/// 内部存储项
struct Slot {
    handle: JoinHandle<()>,
    // 每一个 Slot 都持有一个指向自己当前索引的共享引用
    share: Arc<SlotShare>,
    end: u64,
}

impl Slot {

    fn new(handle: JoinHandle<()>, share: Arc<SlotShare>, end: u64) -> Self{
        Self{
            handle,
            share,
            end
        }
    }

    fn remain(&self) -> &AtomicU64{
        &self.share.remain
    }

    fn get_index(&self) -> *mut usize {
        self.share.index.0.get()
    }

    // fn into_segment(self) -> Option<Segment> {
    //     let share = self.share.as_raw()
    //     //let remain = self.share.remain.get_mut().clone();
    //     let nonzero = NonZeroU64::new(remain)?;
    //     Segment::new(self.end - remain, nonzero).into()
    // }
}

struct SlotShare{
    index: SyncUnsafeCell<usize>,
    remain: AtomicU64,
}



impl SlotShare {
    fn new_pair(index_ref: usize, remain: u64) -> (Arc<SlotShare>, Arc<SlotShare>) {
        let share: Arc<_> =SlotShare{
            index: index_ref.into(),
            remain: remain.into()
        }.into();
        (share.clone(), share)
    }
}


struct Hander{
    share: Share,
    slot_share: SlotShare
}

trait HanderControl{
    fn run(segment: Segment) -> Self;
    fn abort(&mut self);
}

impl Hander {
    pub fn downloaded_len(&mut self, len: u64) -> u64{
        todo!()
    }

    pub fn writed_len(&mut self, len: u64) -> u64{
        todo!()
    }


    fn wake(&self) {
        self.share.waker.wake();
    }

    fn delete_hander_control(&self){
        todo!()
    }
}

impl Downloader {


    pub fn new_task<F>(&self, future: F, response: Option<Response>)
    where
        F: Future<Output = Result<(),()>> + Send + 'static,
    {
        self.share.task_num.fetch_add(1, Ordering::Release);
        

        let mut guard = self.share.locked.lock();
        let (share_for_slot, share_for_task) = SlotShare::new_pair(guard.slots.len(), todo!());
        let dl_info_for_task = self.share.clone();


        let task = async move {
            let result = future.await;

            
            let mut guard = dl_info_for_task.locked.lock();
            let idx = index_ref_for_task.load(Ordering::Acquire);
            
            // 关键：执行 swap_remove
            let last_idx = guard.slots.len() - 1;
            if idx != last_idx {
                // 如果移除的不是最后一个，最后一个元素会被移动到 idx 位置
                // 我们必须更新被移动元素的索引引用
                guard.slots[last_idx].index_ref.store(idx, Ordering::Release);
            }

            let my_slot = guard.slots.swap_remove(idx);

            // 精细化唤醒：任务删除了，通知 join_next 该返回了
            guard.waker.wake();

            dl_info_for_task.task_num.fetch_sub(1, Ordering::Release);
            
            
            match result {
                Ok(_) => {
                    
                },

                Err(_) => {
                    guard.idie_segments.push(Segment { remain: my_slot.remain().load(Ordering::Relaxed), end: my_slot.end });
                }
            }
        };

        guard.slots.push(Slot::new(tokio::spawn(task), share_for_slot, todo!()));
    }

    /// 核心：实现类似 Stream 的 poll_next 逻辑
    pub fn poll_wait_all(&self, cx: &mut Context<'_>) -> Poll<()> {
        let inner = self.share.locked.lock();
        
        if inner.slots.is_empty() {
            Poll::Ready(())
        } else{
            // 注册当前 Context 的 Waker
            // 当任何一个任务执行完“自清理”逻辑后，会调用 inner.waker.wake()
            self.share.waker.register(cx.waker());

            // 注意：由于是自清理，我们不需要在这里 poll 每一个 JoinHandle
            // 只要任务完成，它就会把自己从 Vec 删掉，并触发上面的 wake()
            // 这样下一次 poll 进来时，我们会发现 len 变小了。
            // 但为了简单，我们通常让 join_next 只是作为一个“信号”返回
            Poll::Pending
        }
    }

    pub async fn wait_all(&self) -> Option<()> {
        futures::future::poll_fn(|cx| self.poll_wait_all(cx)).await
    }

    // pub fn poll_wait_next()

    // fn abort_all(&mut self) {
    //     let inner= self.share.locked.lock();
    //     for i in inner.slots{ i.handle.abort(); }
    // }

    fn shutdown(&mut self) {
        self.abort_all();
        self.wait_all();
    }
}

impl LockedShare {

    fn swap_remove(&mut self, index: usize) -> Segment{
        let removed = self.slots.swap_remove(index);

        if index != self.slots.len() {
            *self.get_slot_index_mut(index) = index;
        }

    }


    fn get_slot_index_mut(&mut self, index: usize) -> &mut usize {
        //Safety: 我们有一个独占的&mut self，
        unsafe{
            &mut *self.slots[index].get_index()
        }
    }

    fn get_slot_index_ref(&self, index: usize) -> &usize {
        //Safety: 我们有&self，保证没有其他的&mut self
        unsafe {
            & *self.slots[index].get_index()
        }
    }
}

enum RunningState {
    Getting,
    Downloading,
}

use reqwest::{Client, Error, Response};


struct SyncUnsafeCell<T>(UnsafeCell<T>);

unsafe impl<T: Sync> Sync for SyncUnsafeCell<T>{}

impl<T> From<T> for SyncUnsafeCell<T> {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}