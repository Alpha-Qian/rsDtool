//!定义分块下载的并发结构体
//!

use std::cell::UnsafeCell;
use std::ops::Deref;

use std::sync::atomic::{AtomicUsize, Ordering};

//use crate::downloader::family::AtomicSwapable;

use super::family::{AtomicCell, Lockable, Mutex, RefCounted, RefCounter, ThreadModel, AtomicSwapable, AsMutUnsafe};
use radium::Radium;

//
pub struct DownloadGroup<'a, F: ThreadModel, E: GroupExt<F>> {
    share: F::RefCounter<GroupShare<'a, F, E>>, //different from state reporter, this field can be pub
}

impl<'a, F: ThreadModel, E: GroupExt<F>> DownloadGroup<'a, F, E> {
    fn lock(&'a self) -> GroupGuard<'a, F, E> {
        let g = self.share.locked.lock();
        GroupGuard {
            group: self,
            guard: g,
        }
    }
}

///专为下载任务特化的任务管理器，运行时无关
struct GroupShare<'a, F: ThreadModel, E: GroupExt<F>> {
    locked: F::Mutex<InLockShare<'a, F, E>>,
    pub ext: E::GroupShareExt<'a>,
}

struct InLockShare<'a, F: ThreadModel, E: GroupExt<F>> {
    pub slots: Vec<Slot<'a, F, E>>, // or Box<[Slot]>?
    pub ext: E::InLockShareExt<'a>,
}

impl<'a, F: ThreadModel, E: GroupExt<F>> InLockShare<'a, F, E> {
    fn swap(&mut self, a: usize, b: usize) {
        self.slots.swap(a, b);
        unsafe {
            *self.slots[a].get_index() = a;
            *self.slots[b].get_index() = b;
        };
    }

    fn remove(&mut self, index: usize) -> Slot<'a, F, E> {
        let removed = self.slots.swap_remove(index);

        if index != self.slots.len() {
            *self.get_slot_index_mut(index) = index;
        }
        removed
    }

    ///从逻辑上只要拥有guard就拥有内部所有index字段的所有权
    fn get_slot_index_mut(&mut self, index: usize) -> &mut usize {
        //Safety: 我们有一个独占的&mut self，
        unsafe { &mut *self.slots[index].get_index() }
    }

    fn get_slot_index_ref(&self, index: usize) -> &usize {
        //Safety: 我们有&self，保证没有其他的&mut self
        unsafe { &*self.slots[index].get_index() }
    }

    fn push(&mut self, value: Slot<'a, F, E>) {
        self.slots.push(value);
    }

    ///Safety:
    /// 确保slot_share在self内
    pub unsafe fn remove_slot(&mut self, slot_share: &SlotShare<'_, F, E>) -> Slot<'a, F, E> {
        let index = unsafe { *slot_share.index.0.get() };
        self.remove(index)
    }
}

/// 内部存储项
struct Slot<'a, F: ThreadModel, E: GroupExt<F>> {
    share: RefCounter<F, SlotShare<'a, F, E>>,
    pub ext: E::SlotExt<'a>,
}

impl<'a, F: ThreadModel, E: GroupExt<F>> Slot<'a, F, E> {
    fn with_raw(share: RefCounter<F, SlotShare<'a, F, E>>, ext: E::SlotExt<'a>) -> Self {
        Self { share, ext }
    }

    fn get_index(&self) -> *mut usize {
        self.share.index.0.get()
    }
}

struct SlotShare<'a, F: ThreadModel, E: GroupExt<F>> {
    // 指向自己当前索引的共享引用
    index: SyncUnsafeCell<usize>,
    pub ext: E::SlotShareExt<'a>,
}

impl<'a, F: ThreadModel, E: GroupExt<F>> SlotShare<'a, F, E> {
    fn new_pair(
        index: usize,
        ext: E::SlotShareExt<'a>,
    ) -> (
        F::RefCounter<SlotShare<'a, F, E>>,
        F::RefCounter<SlotShare<'a, F, E>>,
    ) {
        let share = F::RefCounter::new(SlotShare {
            index: index.into(),
            ext,
        });
        (share.clone(), share)
    }
}

///每个下载分块向下载组报告状态的结构体
struct Reporter<'a, F: ThreadModel, E: GroupExt<F>> {
    share: F::RefCounter<GroupShare<'a, F, E>>, //must priv, cant get &mut self.share
    slot_share: F::RefCounter<SlotShare<'a, F, E>>, //must priv
}

impl<'a, F: ThreadModel, E: GroupExt<F>> Reporter<'a, F, E> {
    pub fn share(&self) -> &GroupShare<'a, F, E> {
        //only read
        &self.share
    }

    pub fn slot_share(&self) -> &SlotShare<'a, F, E> {
        //only read
        &self.slot_share
    }

    fn lock(&'a self) -> ReporterGuard<'a, F, E> {
        let guard = self.share.locked.lock();
        //guard
        ReporterGuard { view: self, guard }
    }
}

trait ProcessRecordKind {
    type State;
    type Downloaded<T>: Radium<Item = T>;
    type Writed<T>: Radium<Item = T>;

    fn report_downloaded_len(len: u64);
}

///静态标记空结构体
trait GroupExt<F: ThreadModel>: 'static {
    type GroupShareExt<'a>;
    type InLockShareExt<'a>;
    type SlotShareExt<'a>: Default; //Default用于预先分配
    type SlotExt<'a>: Default;

    //type Element<'a>: From<ExtTurple<'a, F, Self>> + Into<ExtTurple<'a, F, Self>>;
}

type ExtElement<'a, F, E: GroupExt<F>> = (E::SlotExt<'a>, E::SlotShareExt<'a>);

struct ExtHander<'a, E: GroupExt<F>, F: ThreadModel> {
    group_share: &'a E::GroupShareExt<'a>,
    slot_share: &'a E::SlotShareExt<'a>,
}

///tools
struct SyncUnsafeCell<T>(UnsafeCell<T>);

unsafe impl<T: Sync> Sync for SyncUnsafeCell<T> {}

impl<T> From<T> for SyncUnsafeCell<T> {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

//--------------------------LockedGuards---------------------------

///groupWriteGuard
struct GroupGuard<'a, F: ThreadModel, E: GroupExt<F>> {
    group: &'a DownloadGroup<'a, F, E>,
    guard: <F::Mutex<InLockShare<'a, F, E>> as Lockable>::Guard<'a>, //impl DerefMut<Target = InLockShare>
}

impl<'a, F: ThreadModel, E: GroupExt<F>> GroupGuard<'a, F, E> {
    ///move to lockedgroup
    fn new_reporter(
        &'a mut self,
        ext: E::SlotExt<'a>,
        ext_share: E::SlotShareExt<'a>,
    ) -> Reporter<F, E> {
        let (share1, share2) = SlotShare::<F, E>::new_pair(self.guard.slots.len(), ext_share);
        self.guard.push(Slot::with_raw(share1, ext));
        Reporter {
            share: self.group.share.clone(),
            slot_share: share2,
        }
    }

    fn slots(&mut self) -> &mut Vec<Slot<'a, F, E>> {
        &mut self.guard.slots
    }

    fn inlock_ext(&mut self) -> &mut E::InLockShareExt<'a> {
        &mut self.guard.ext
    }
}

///reporter WriteGuard
struct ReporterGuard<'a, F: ThreadModel, E: GroupExt<F>> {
    view: &'a Reporter<'a, F, E>,
    guard: <F::Mutex<InLockShare<'a, F, E>> as Lockable>::Guard<'a>,
}

impl<'a, F: ThreadModel, E: GroupExt<F>> ReporterGuard<'a, F, E> {
    fn slots(&mut self) -> &mut Vec<Slot<'a, F, E>> {
        &mut self.guard.slots
    }

    fn inlock_ext(&mut self) -> &mut E::InLockShareExt<'a> {
        &mut self.guard.ext
    }

    fn my_index(&mut self) -> &mut usize {
        //Safety: 我们有guard，逻辑上拥有index字段的所有权
        unsafe { &mut *self.view.slot_share.index.0.get() }
    }

    fn remove_me(&mut self) {
        let index = *self.my_index();
        let removed = self.guard.remove(index);
    }
    //
    // fn remove_me(&mut self) {
    //     //Safty: 我们确保了参数是内部元素
    //     unsafe {
    //         self.guard.remove_slot(&self.view.slot_share);
    //     }
    // }
}

// struct LazyDropVec<F: ThreadModel, T> {
//     pub inner: Vec<T>, //maybe in Mutex
//     pub alive: F::AtomicCell<usize>,
// }
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

// ///TODO: 移动到专用的异步包装器上
// impl<'a, F: ThreadModel, E: GroupExt<F>> DownloadGroup<'a, F, E> {
//     pub fn new_task<F>(&self, future: F, response: Option<Response>)
//     where
//         F: Future<Output = Result<(),()>> + Send + 'static,
//     {
//         self.share.task_num.fetch_add(1, Ordering::Release);
//         let mut guard = self.share.locked.lock();
//         let (share_for_slot, share_for_task) = SlotShare::new_pair(guard.slots.len(), todo!());
//         let dl_info_for_task = self.share.clone();
//         let task = async move {
//             let result = future.await;
//             let mut guard = dl_info_for_task.locked.lock();
//             let idx = index_ref_for_task.load(Ordering::Acquire);
//             // 关键：执行 swap_remove
//             let last_idx = guard.slots.len() - 1;
//             if idx != last_idx {
//                 // 如果移除的不是最后一个，最后一个元素会被移动到 idx 位置
//                 // 我们必须更新被移动元素的索引引用
//                 guard.slots[last_idx].index_ref.store(idx, Ordering::Release);
//             }
//             let my_slot = guard.slots.swap_remove(idx);
//            // 精细化唤醒：任务删除了，通知 join_next 该返回了
//             guard.waker.wake();
//             dl_info_for_task.task_num.fetch_sub(1, Ordering::Release);
//             match result {
//                 Ok(_) => {
//                 },
//                 Err(_) => {
//                     guard.idie_segments.push(Segment { remain: my_slot.remain().load(Ordering::Relaxed), end: my_slot.end });
//                 }
//             }
//         };
//         guard.slots.push(Slot::new(tokio::spawn(task), share_for_slot, todo!()));
//     }

//     核心：实现类似 Stream 的 poll_next 逻辑

//     TODO: 移动到专用的异步包装器上
//     pub fn poll_wait_all(&self, cx: &mut Context<'_>) -> Poll<()> {
//         todo!("move this method to GroupShare");
//         let inner = self.share.locked.lock();

//         if inner.running_slots.is_empty() {
//             Poll::Ready(())
//         } else{
//             self.share.waker.register(cx.waker());todo!("处理这个")
//             Poll::Pending
//         }
//     }

//     pub async fn wait_all(&self) -> Option<()> {
//         //futures::future::poll_fn(|cx| self.poll_wait_all(cx)).await
//         todo!()
//     }

//     pub fn poll_wait_next()

//     fn abort_all(&mut self) {
//         let inner= self.share.locked.lock();
//         for i in inner.slots{ i.handle.abort(); }
//     }
// }

// TODO: 移动到专用的异步包装器上
// impl<'a, F: ThreadModel, E: GroupExt<F>> Future for DownloadGroup<'a, F, E> {
//     type Output = ();
//     fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
//         self.poll_wait_all(cx)
//     }
// }

// 核心：实现类似 Stream 的 poll_next 逻辑
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
