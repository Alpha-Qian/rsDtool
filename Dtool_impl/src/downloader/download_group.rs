use std::cell::UnsafeCell;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{sync::atomic::AtomicU64};

use super::segment::Segment;

use std::task::{Context, Poll, Waker};


use super::family::{ThreadModel, Lockable, RefCounted, RefCounter, AtomicCell, Mutex};
use radium::Radium;

///segment to worker part

trait SegmentToWorker{
    type Output : Abort;
    fn new(segment: Segment) -> Self::Output;
}

struct Control<'a, F: ThreadModel, T, U>{ //segment to worker controler
    download: DownloaderGroup<'a, F, U>,
    share: F::RefCounter<'static, ControlShared>,
    segment_to_worker: T,
}

struct ControlShared{
    task_num: AtomicUsize,
    targit_task_num: AtomicUsize,
}

impl<'a, F: ThreadModel, T: SegmentToWorker> Control<'a, F, T, T::Output> {
    fn new_task(&self) -> Result<(), ()>{
        let mut guard = self.download.share.locked.lock();
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


// Download Group

pub struct DownloaderGroup<'a, F: ThreadModel, T: 'a> {
    share: F::RefCounter<'a, GroupShare<F, T>>,
    waker: Waker,
}

impl<'a, F: ThreadModel, T> DownloaderGroup<'a, F, T> {
    async fn new() -> Self{
        todo!()
    }
}

/// TODO: 移动到专用的异步包装器上
impl<'a, F: ThreadModel, T> Future for DownloaderGroup<'a, F, T> {
    type Output = ();
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_wait_all(cx)
    }
}


///专为下载任务特化的任务管理器，运行时无关
struct GroupShare<F: ThreadModel, T>{
    locked: F::Mutex<LockedShare<F, T>>,
    process: F::AtomicCell<u64>,
}

struct LockedShare<F: ThreadModel, T> {
    running_slots: Vec<Slot<F, T>>,
    idie_segments: Vec<Segment>,//也许可以去掉
    //pub ext_data: GroupExt
}


impl<F: ThreadModel, T: Abort> LockedShare<F, T> {

    fn swap(&mut self, a: usize, b: usize) {
        self.running_slots.swap(a, b);
        unsafe {
            *self.running_slots[a].get_index() = a;
            *self.running_slots[b].get_index() = b;
        };
    }

    fn swap_remove_slot(&mut self, index: usize) -> Option<Slot<F, T>> {
        let removed = self.running_slots.swap_remove(index);

        if index != self.running_slots.len() {
            *self.get_slot_index_mut(index) = index;
        }

        //removed.into_segment()
        removed.into()
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
struct Slot<F: ThreadModel, T> {
    share: RefCounter<F, SlotShare<F>>, //F::RefCounter<'static, SlotShare<F>>,
    end: u64,

    abort_handler: T,//pub SlotExt
}

impl<F: ThreadModel, T: Abort> Slot<F, T> {

    fn new(handle: T, share: RefCounter<F, SlotShare<F>>, end: u64) -> Self{
        Self{
            abort_handler: handle,
            share,
            end
        }
    }

    fn remain(&self) -> &F::AtomicCell<u64>{
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
}

struct SlotShare<F: ThreadModel>{
    // 指向自己当前索引的共享引用
    index: SyncUnsafeCell<usize>,
    remain: F::AtomicCell<u64>,
    //pub ext: E::
}



impl<F: ThreadModel> SlotShare<F> {
    fn new_pair(index_ref: usize, remain: u64) -> (F::RefCounter<'static, SlotShare<F>>, F::RefCounter<'static, SlotShare<F>>) {
        let share = F::RefCounter::new( SlotShare{
            index: index_ref.into(),
            remain: remain.into()
        });
        (share.clone(), share)
    }
}

trait Abort{
    fn abort(&mut self);
}

///每个下载分块向下载组报告状态的结构体
struct StateReporter<F: ThreadModel, T>{
    share: GroupShare<F, T>,
    slot_share: SlotShare<F>
}

impl<F: ThreadModel, T> StateReporter<F, T> {
    pub fn report_downloaded_len(&mut self, len: u64) -> u64{
        todo!()
    }

    pub fn report_writed_len(&mut self, len: u64) -> u64{
        todo!()
    }

    pub fn set_state(&mut self){
        todo!()
    }

    pub fn remove_slot(&self) {
        todo!()
    }
    //state或许用泛型好些

    // 弃用：移动到异步包装器
    // fn wake_executer(&self) { 这个方法应该在group control上
    //     self.share.waker.wake();
    // }

    // fn remove_hander_control(&mut self){
    //     self.share.locked.
    // }
}

struct Remain(pub u64);
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


impl<'a, F: ThreadModel, T> DownloaderGroup<'a, F, T> {


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
    /// 
    /// TODO: 移动到专用的异步包装器上
    /// 
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

/// 核心：实现类似 Stream 的 poll_next 逻辑
    /// 
    /// TODO: 移动到专用的异步包装器上
    /// 
impl<F: ThreadModel, T> GroupShare<F, T> {
    pub fn poll_wait_all(&self, cx: &mut Context<'_>) -> Poll<()> {
        let inner = self.locked.lock();
        
        if inner.running_slots.is_empty() {
            Poll::Ready(())
        } else{
            self.waker.register(cx.waker());
            Poll::Pending
        }
    }
}

trait ProcessRecordKind{
    type State;
    type Downloaded<T>: Radium<Item = T>;
    type Writed<T>: Radium<Item = T>;

    fn report_downloaded_len(len: u64)

}

///感觉有点多余？
trait Ext{
    type GroupShareExt;
    type LockedShareExt;//
    type SlotShareExt;
    type SlotExt;

    type SlotState;
    fn report_save_len(len: u64);

    fn report_slot_state(state: Self::SlotState);

    fn done()
}

struct ExtHander<'a, E: Ext>{
    group_share: &'a E::GroupShareExt,
    slot_share: &'a E::SlotShareExt
}


///tools
struct SyncUnsafeCell<T>(UnsafeCell<T>);

unsafe impl<T: Sync> Sync for SyncUnsafeCell<T>{}

impl<T> From<T> for SyncUnsafeCell<T> {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}
