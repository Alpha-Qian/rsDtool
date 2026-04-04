//!定义分块下载的并发结构体
use core::result::Result::Err;
use std::cell::UnsafeCell;
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{sync::atomic::AtomicU64};

use super::segment::Segment;

use std::task::{Context, Poll, Waker};

use super::family::{ThreadModel, Lockable, RefCounted, RefCounter, AtomicCell, Mutex};
use futures::future::Shared;
use radium::Radium;


// Download Group

pub struct DownloadGroup<'a, F: ThreadModel, E: GroupExt<F>> {
    share: F::RefCounter<GroupShare<'a, F, E>>,
}

impl<'a, F: ThreadModel, E: GroupExt<F>> DownloadGroup<'a, F, E> {
    async fn new() -> Self{
        todo!()
    }
}



///专为下载任务特化的任务管理器，运行时无关
struct GroupShare<'a, F: ThreadModel, E: GroupExt<F>>{
    locked: F::Mutex<LockedShare<F, E>>,
    process: F::AtomicCell<u64>,
    pub ext: E::GroupShareExt<'a>,
}


struct LockedShare<'a, F: ThreadModel, E: GroupExt<F>> {
    running_slots: Vec<Slot<F, E>>,
    pub ext: E::LockedShareExt<'a>,
}


impl<'a, F: ThreadModel, E: GroupExt<F>> LockedShare<'a, F, E> {

    fn swap(&mut self, a: usize, b: usize) {
        self.running_slots.swap(a, b);
        unsafe {
            *self.running_slots[a].get_index() = a;
            *self.running_slots[b].get_index() = b;
        };
    }

    fn remove_index(&mut self, index: usize) -> Option<Slot<F, E>> {
        let removed = self.running_slots.swap_remove(index);

        if index != self.running_slots.len() {
            *self.get_slot_index_mut(index) = index;
        }

        //removed.into_segment()
        removed.into()
    }

    ///从逻辑上只要拥有guard就拥有内部所有index字段的所有权
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

    fn push_running(&mut self, value: Slot<F, E>) {
        self.running_slots.push(value);
    }
    ///Safty:
    /// 确保slot_share在self内
    pub unsafe fn remove_slot(&mut self, slot_share: &SlotShare<'_, F, E>) {
        let index = unsafe{ *slot_share.index.0.get() };
        self.remove_index(index)
    }
}

/// 内部存储项
struct Slot<'a, F: ThreadModel, E: GroupExt<F>> {
    share: RefCounter<F, SlotShare<'a, F, E>>, //F::RefCounter<'static, SlotShare<F>>,
    end: u64,

    pub ext: E::SlotExt<'a>
}

impl<'a, F: ThreadModel, E: GroupExt<F>> Slot<'a, F, E> {

    fn new(share: RefCounter<F, SlotShare<F, E>>, end: u64, ext: E::SlotExt<'a>) -> Self{
        Self{
            share,
            end,
            ext
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

struct SlotShare<'a, F: ThreadModel, E: GroupExt<F>>{
    // 指向自己当前索引的共享引用
    index: SyncUnsafeCell<usize>,
    remain: F::AtomicCell<i64>,//TODO改为i64
    pub ext: E::SlotShareExt<'a>,
}

impl<'a, F: ThreadModel, E: GroupExt<F>> SlotShare<'a, F, E> {
    fn new_pair(index_ref: usize, remain: u64, ext: E::SlotShareExt) -> (F::RefCounter<SlotShare<'a, F, E>>, F::RefCounter<SlotShare<'a, F, E>>) {
        let share = F::RefCounter::new( SlotShare{
            index: index_ref.into(),
            remain: remain.into(),
            ext
        });
        (share.clone(), share)
    }

    // ///高频调用
    // pub fn fetch_sub_remain(&self, len: u64) -> u64{
    //     max(self.remain.fetch_sub(value, Ordering::Relaxed) - len, 0)
    // }

    // ///低频调用
    // pub fn fetch_sub_exchange(&self, len: u64) -> u64{
    //     let r = self.remain.fetch_update(
    //         Ordering::Relaxed, 
    //         Ordering::Relaxed, 
    //         |x| {
    //             max(r - len, 0).into()
    //         }
    //     ).unwrap();
    //     max(r - len, 0)
    // }

    // ///如果值小于0，则视为0
    // pub fn remain(&self) -> &F::AtomicCell<i64>{
    //     &self.remain
    // }

}
///每个下载分块向下载组报告状态的结构体
struct StateReporter<'a, F: ThreadModel, E: GroupExt<F>>{
    share: F::RefCounter<GroupShare<'a, F, E>>,
    slot_share: F::RefCounter<SlotShare<'a, F, E>>
}

impl<'a, F: ThreadModel, E: GroupExt<F>> StateReporter<'a, F, E> {

    ///TODO: 检查安全性
    pub unsafe fn new(locked: &mut LockedShare<'a, F, E>, share: F::RefCounter<GroupShare<'a, F, E>>, remain: u64, end: u64, ext: E::SlotExt, share_ext: E::SlotShareExt) -> Self{
        let (share1, share2) = SlotShare::<F, E>::new_pair(locked.running_slots.len(), remain, share_ext);
        let slot = Slot::<F, E>::new(share1, end, ext);
        locked.push_running(value);
        Self { share, slot_share: share2 }
    }

    ///Safety:
    /// 
    pub unsafe fn with_raw(locked: &mut LockedShare<'a, F, E>, s){}

    pub fn share(&self) -> &GroupShare<'a, F, E> {
        &self.share
    }

    pub fn slot_share(&self) -> &SlotShare<'a, F, E> {
        &self.slot_share
    }
    
    ///TODO:
    /// 移动到SlotShare的方法上
    ///Safty:
    /// 确保self在locked_share里
    // pub unsafe fn remove_slot(&self, locked_share: &mut LockedShare<'_, F, E>) {
    //     locked_share.swap_remove_slot(unsafe {
    //        *self.slot_share.index.0.get() //can use ghostcell or qcell
    //     });
    // }

    // fn test(&self) {
    //     let g: <<F as ThreadModel>::Mutex<LockedShare<'_, F, E>> as Lockable>::Guard<'_> = self.share.locked.lock();
    // }

    fn lock(&self) -> LockedReporter<'a, F, E> {
        let guard = self.share.locked.lock();
        //guard
        LockedReporter { reporter: &self, guard }
    }
}


struct LockedReporter<'a, F: ThreadModel, E: GroupExt<F>>{
    reporter: &'a StateReporter<'a, F, E>,
    guard: <F::Mutex<LockedShare<'a, F, E>> as Lockable>::Guard<'a>
}

impl<'a, F: ThreadModel, E: GroupExt<F>> LockedReporter<'a, F, E> {
    fn remove_slot(&self) {
        //Safty: 我们确保了参数是内部元素
        unsafe{ self.guard.remove_slot(&self.reporter.slot_share);}
    }

    fn slot_index(&self) -> &mut usize{
        //Safety: 我们有guard，拥有index字段的所有权
        unsafe{ &mut *self.reporter.slot_share.index.0.get()}
    }
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




trait ProcessRecordKind{
    type State;
    type Downloaded<T>: Radium<Item = T>;
    type Writed<T>: Radium<Item = T>;

    fn report_downloaded_len(len: u64);

}


trait GroupExt<F: ThreadModel>: 'static{//静态标记空结构体
    type GroupShareExt<'a>;
    type LockedShareExt<'a>;
    type SlotShareExt<'a>; //不需要Sync
    type SlotExt<'a>;
}


struct ExtHander<'a, E: GroupExt<F>, F: ThreadModel>{
    group_share: &'a E::GroupShareExt<'a>,
    slot_share: &'a E::SlotShareExt<'a> 
}


///tools
struct SyncUnsafeCell<T>(UnsafeCell<T>);

unsafe impl<T: Sync> Sync for SyncUnsafeCell<T>{}

impl<T> From<T> for SyncUnsafeCell<T> {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}


//segment to worker part
//
// trait SegmentToWorker{
//     type Output : Abort;
//     fn new(segment: Segment) -> Self::Output;
// }
//
// struct Control<'a, F: ThreadModel, T, U>{ //segment to worker controler
//     download: DownloaderGroup<'a, F, U>,
//     share: F::RefCounter<'static, ControlShared>,
//     segment_to_worker: T,
// }
//
// struct ControlShared{
//     task_num: AtomicUsize,
//     targit_task_num: AtomicUsize,
// }
//
// impl<'a, F: ThreadModel, T: SegmentToWorker> Control<'a, F, T, T::Output> {
//     fn new_task(&self) -> Result<(), ()>{
//         let mut guard = self.download.share.locked.lock();
//         //guard.idie_segments.pop().or(optb)
//         let segment: Segment = guard.idie_segments.pop().or(
//             {
//                 let max_slot = None;
//                 guard.running_slots.iter().for_each(|s|{} );
//                 todo!()
//             }
//         ).ok_or(())?;
   //     
//     }
// }
//
//
// // Downloader part
//
// trait ProcessTrack {
//     fn downloaded(len: usize);
//     fn writed(len: usize);
// }

/// TODO: 移动到专用的异步包装器上
impl<'a, F: ThreadModel, E: GroupExt<F>> DownloadGroup<'a, F, E> {


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

    // 核心：实现类似 Stream 的 poll_next 逻辑
    // 
    // TODO: 移动到专用的异步包装器上
    // pub fn poll_wait_all(&self, cx: &mut Context<'_>) -> Poll<()> {
    //     todo!("move this method to GroupShare");
    //     let inner = self.share.locked.lock();
        
    //     if inner.running_slots.is_empty() {
    //         Poll::Ready(())
    //     } else{
    //         self.share.waker.register(cx.waker());todo!("处理这个")
    //         Poll::Pending
    //     }
    // }

    // pub async fn wait_all(&self) -> Option<()> {
    //     //futures::future::poll_fn(|cx| self.poll_wait_all(cx)).await
    //     todo!()
    // }

    // pub fn poll_wait_next()

    // fn abort_all(&mut self) {
    //     let inner= self.share.locked.lock();
    //     for i in inner.slots{ i.handle.abort(); }
    // }
}

/// TODO: 移动到专用的异步包装器上
impl<'a, F: ThreadModel, E: GroupExt<F>> Future for DownloadGroup<'a, F, E> {
    type Output = ();
    fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.poll_wait_all(cx)
    }
}



/// 核心：实现类似 Stream 的 poll_next 逻辑
    /// 
    /// TODO: 移动到专用的异步包装器上
    /// 
// impl<F: ThreadModel, E: GroupExt<F>> GroupShare<F, E> {
//     pub fn poll_wait_all(&self, cx: &mut Context<'_>) -> Poll<()> {
//         let inner = self.locked.lock();
        
//         if inner.running_slots.is_empty() {
//             Poll::Ready(())
//         } else{
//             self.waker.register(cx.waker());
//             Poll::Pending
//         }
//     }
// }