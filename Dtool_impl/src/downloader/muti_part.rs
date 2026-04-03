use std::cell::UnsafeCell;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{sync::atomic::AtomicU64};

use super::segment::Segment;

use std::task::{Context, Poll, Waker};


use super::family::{SyncKind, SingleThread, MutiThread, MaybeMutex, SharePtrExt, SharePtr};
use radium::Radium;

///segment to worker part


trait SegmentToWorker{
    type Output : Abort;
    fn new(segment: Segment) -> Self::Output;
}

struct Control<'a, F: SyncKind, T, U>{ //segment to worker controler
    download: DownloaderGroup<'a, F, U>,
    share: F::SharePtr<'static, ControlShared>,
    segment_to_worker: T,
}

struct ControlShared{
    task_num: AtomicUsize,
    targit_task_num: AtomicUsize,
}

impl<'a, F: SyncKind, T: SegmentToWorker> Control<'a, F, T, T::Output> {
    fn new_task(&self) -> Result<(), ()>{
        let guard = self.download.share.locked.lock();
        //guard.idie_segments.pop().or(optb)
        let segment: Segment = guard.idie_segments.pop().or(
            {
                let max_slot = None;
                guard.running_slots.iter().for_each(|s|{} );
                todo!()
            }
        ).ok_or(())?;
        
    }
}


// Downloader part

trait ProcessTrack {
    fn downloaded(len: usize);
    fn writed(len: usize);
}

pub struct DownloaderGroup<'a, F: SyncKind, T: 'a> {
    share: F::SharePtr<'a, GroupShare<F, T>>,
    waker: Waker,
}

impl<'a, F: SyncKind, T> DownloaderGroup<'a, F, T> {
    async fn new() -> Self{

    }
}
impl<'a, F: SyncKind, T> Future for DownloaderGroup<'a, F, T> {
    type Output = ();
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_wait_all(cx)
    }
}



struct GroupShare<F: SyncKind, T>{
    locked: F::DataLock<LockedShare<F, T>>,
    //waker: Waker,
    //TODO: waker is cloneable!!!
    //waker相当于自带了一个Arc，不用放在Arc<GroupShare>里
    process: F::MaybeAtom<u64>,

}

struct LockedShare<F: SyncKind, T> {
    running_slots: Vec<Slot<F, T>>, //or named running_slots?
    idie_segments: Vec<Segment>,
}


impl<F: SyncKind, T: Abort> LockedShare<F, T> {

    fn swap(&mut self, a: usize, b: usize) {
        self.running_slots.swap(a, b);
        unsafe {
            *self.running_slots[a].get_index() = a;
            *self.running_slots[b].get_index() = b;
        };
    }

    fn swap_remove(&mut self, index: usize) -> Option<Segment> {
        let removed = self.running_slots.swap_remove(index);

        if index != self.running_slots.len() {
            *self.get_slot_index_mut(index) = index;
        }

        removed.into_segment()
    }


    fn get_slot_index_mut(&mut self, index: usize) -> &mut usize {
        //Safety: 我们有一个独占的&mut self，
        unsafe{
            &mut *self.running_slots[index].get_index()
        }
    }

    fn get_slot_index_ref(&self, index: usize) -> &usize {
        //Safety: 我们有&self，保证没有其他的&mut self
        unsafe {
            & *self.running_slots[index].get_index()
        }
    }

    fn push(&mut self, value: Slot<F, T>) {
        self.running_slots.push(value);
    }
}

/// 内部存储项
struct Slot<F: SyncKind, T> {
    abort_handler: T,
    
    share: F::SharePtr<'static, SlotShare>,
    end: u64,

    waker: Waker,//callback
}

impl<F: SyncKind, T: Abort> Slot<F, T> {

    fn new(handle: T, share: F::SharePtr<'static, SlotShare>, end: u64) -> Self{
        Self{
            abort_handler: handle,
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

    fn into_segment(self) -> Option<Segment> {
        let remain= NonZeroU64::new(self.remain().load(Ordering::Relaxed))?;
        let start = self.end - remain.get();

        Segment::new(start, remain).into()
    }

    fn wake(self) {//notifield
        self.waker.wake();
    }

}

struct SlotShare{
    // 指向自己当前索引的共享引用
    index: SyncUnsafeCell<usize>,
    remain: AtomicU64,
}



impl SlotShare {
    fn new_pair<F: SyncKind>(index_ref: usize, remain: u64) -> (F::SharePtr<'static, SlotShare>, F::SharePtr<'static, SlotShare>) {
        let share = F::SharePtr::new( SlotShare{
            index: index_ref.into(),
            remain: remain.into()
        });
        (share.clone(), share)
    }
}


struct StateSender<F: SyncKind, T>{
    share: GroupShare<F, T>,
    slot_share: SlotShare
}

trait Abort{
    fn abort(&mut self);
}

impl<F: SyncKind, T> StateSender<F, T> {
    pub fn downloaded_len(&mut self, len: u64) -> u64{
        todo!()
    }

    pub fn writed_len(&mut self, len: u64) -> u64{
        todo!()
    }

    pub fn set_state(&mut self)

    fn wake_executer(&self) {
        self.share.waker.wake();
    }

    fn remove_hander_control()
}

// struct WriteCorrect<'a, T>{
//     hander: &'a mut Hander<T>,
//     len: u64,
//     pub progress: u64,
// }

// impl<'a> WriteCorrect<'a> {
//     pub fn ok(self) -> u64 {
//         //self.hander.share;
//         self.progress
//     }
// }


impl<'a, F: SyncKind, T> DownloaderGroup<'a, F, T> {


    // pub fn new_task<F>(&self, future: F, response: Option<Response>)
    // where
    //     F: Future<Output = Result<(),()>> + Send + 'static,
    // {
    //     self.share.task_num.fetch_add(1, Ordering::Release);
    //     let mut guard = self.share.locked.lock();
    //     let (share_for_slot, share_for_task) = SlotShare::new_pair(guard.slots.len(), todo!());
    //     let dl_info_for_task = self.share.clone();
    //     let task = async move {
    //         let result = future.await;         
    //         let mut guard = dl_info_for_task.locked.lock();
    //         let idx = index_ref_for_task.load(Ordering::Acquire);       
    //         // 关键：执行 swap_remove
    //         let last_idx = guard.slots.len() - 1;
    //         if idx != last_idx {
    //             // 如果移除的不是最后一个，最后一个元素会被移动到 idx 位置
    //             // 我们必须更新被移动元素的索引引用
    //             guard.slots[last_idx].index_ref.store(idx, Ordering::Release);
    //         }
    //         let my_slot = guard.slots.swap_remove(idx);
    //        // 精细化唤醒：任务删除了，通知 join_next 该返回了
    //         guard.waker.wake();
    //         dl_info_for_task.task_num.fetch_sub(1, Ordering::Release);          
    //         match result {
    //             Ok(_) => {             
    //             },
    //             Err(_) => {
    //                 guard.idie_segments.push(Segment { remain: my_slot.remain().load(Ordering::Relaxed), end: my_slot.end });
    //             }
    //         }
    //     };
    //     guard.slots.push(Slot::new(tokio::spawn(task), share_for_slot, todo!()));
    // }

    /// 核心：实现类似 Stream 的 poll_next 逻辑
    pub fn poll_wait_all(&self, cx: &mut Context<'_>) -> Poll<()> {
        todo!("move this method to GroupShare");
        let inner = self.share.locked.lock();
        
        if inner.running_slots.is_empty() {
            Poll::Ready(())
        } else{
            self.share.waker.register(cx.waker());todo!("处理这个")
            Poll::Pending
        }
    }

    pub async fn wait_all(&self) -> Option<()> {
        //futures::future::poll_fn(|cx| self.poll_wait_all(cx)).await
        todo!()
    }

    // pub fn poll_wait_next()

    // fn abort_all(&mut self) {
    //     let inner= self.share.locked.lock();
    //     for i in inner.slots{ i.handle.abort(); }
    // }
}

impl<F: SyncKind, T> GroupShare<F, T> {
    pub fn poll_wait_all(&self, cx: &mut Context<'_>) -> Poll<()> {
        let inner = self.locked.lock();
        
        if inner.running_slots.is_empty() {
            Poll::Ready(())
        } else{
            self.waker.register(cx.waker());todo!("处理这个")
            Poll::Pending
        }
    }
}



enum RunningState {
    Getting,
    Downloading,
    Done,
    Error,
}



///tools
struct SyncUnsafeCell<T>(UnsafeCell<T>);

unsafe impl<T: Sync> Sync for SyncUnsafeCell<T>{}

impl<T> From<T> for SyncUnsafeCell<T> {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}
