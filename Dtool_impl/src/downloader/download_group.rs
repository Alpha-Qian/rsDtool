//!定义分块下载的并发结构体
//!

use std::cell::UnsafeCell;
use std::mem;
use std::ops::{Deref, DerefMut, Index, IndexMut, RangeBounds, RangeFrom};

use std::slice::SliceIndex;
use std::sync::atomic::{AtomicUsize, Ordering};

//use crate::downloader::family::AtomicSwapable;

use super::family::{AtomicCell, Lockable, Mutex, RefCounted, RefCounter, ThreadModel};
use headers::Range;
use radium::Radium;

///一个可以看作多生产者多消费者的数据结构
///线程模型通用
///这个结构体相当于消费者
#[derive(Clone)]
pub struct DownloadGroup<'data, F, E>(
    pub F::RefCounter<GroupShared<'data, F, E>>,
)
where F: ThreadModel, E: GroupExt<F>;


//可以访问：GroupShareExt, Lock
impl<'data, F, E> DownloadGroup<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    pub fn new(share_ext: E::ShareExt<'data>, inlock_ext: E::InLockExt<'data>) -> Self{
        Self::from_raw(F::RefCounter::new(GroupShared::new(share_ext, inlock_ext)))
    }
    pub(crate) fn from_raw(inner: F::RefCounter<GroupShared<'data, F, E>>) -> Self {
        Self(inner)
    }

    pub fn lock<'a>(&'a self) -> GroupGuard<'a, 'data, F, E> {
        let guard = self.0.locked.lock();
        unsafe { GroupGuard::from_raw(&self, guard) }
    }

    pub fn share_ext(&self) -> &E::ShareExt<'data> {
        &self.0.ext
    }

}

///每个下载分块向下载组报告状态的结构体
/// 这个结构体是生产者也是消费者
pub struct Reporter<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    //leak &mut of this field will cause UB
    group: DownloadGroup<'data, F, E>,
    //leak &mut of this field will cause UB
    slot_share: F::RefCounter<SlotShare<'data, F, E>>,
}

//可以访问：GroupShareExt, SlotShareExt
impl<'data, F, E> Reporter<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    unsafe fn from_raw(
        group: DownloadGroup<'data, F, E>,
        slot_share: RefSlotShare<'data, F, E>,
    ) -> Self {
        Self { group, slot_share }
    }

    pub(crate) fn slot_share(&self) -> &RefSlotShare<'data, F, E> {
        &self.slot_share
    }

    pub(crate) fn group(&self) -> &DownloadGroup<'data, F, E> {
        &self.group
    }

    pub(crate) fn group_share(&self) -> &GroupShared<'data, F, E> {
        &self.group.0
    }

    pub fn share_ext(&self) -> &E::ShareExt<'data> {
        &self.group_share().ext
    }

    pub fn slot_share_ext(&self) -> &E::SlotShareExt<'data> {
        &self.slot_share().ext
    }

    pub fn lock<'a>(&'a self) -> ReporterGuard<'a, 'data, F, E> {
        let group_guard = self.group.lock();
        unsafe { ReporterGuard::from_raw(group_guard, &self.slot_share) }
    }
}

///groupWriteGuard
/// 可以访问GroupShareExt, InLockExt, SlotVector
pub struct GroupGuard<'a, 'data, F, E>
where
    <F as ThreadModel>::Mutex<InLockShared<'data, F, E>>: 'a, // 满足 Lockable Trait 的 GAT 约束
    F: ThreadModel,
    E: GroupExt<F>,
{
    //leak &mut of this field will cause UB
    group: &'a mut DownloadGroup<'data, F, E>,
    //leak &mut of this field will cause UB
    guard: InLockSharedGuard<'a, 'data, F, E>, //<F::Mutex<InLockShare<'data, F, E>> as Lockable>::Guard<'a>,
}

impl<'a, 'data, F, E> GroupGuard<'a, 'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    ///安全性：确保guard来自group内部
    pub unsafe fn from_raw(
        group: &'a mut DownloadGroup<'data, F, E>,
        guard: InLockSharedGuard<'a, 'data, F, E>,
    ) -> Self {
        Self { group, guard }
    }

    ///move to lockedgroup
    pub fn new_reporter(
        &mut self,
        ext: E::SlotInlockExt<'data>,
        ext_share: E::SlotShareExt<'data>,
    ) -> Option<Reporter<'data, F, E>> {
        let mut slots = self.slots_mut()?;

        let (slot_share1, slot_share2) =
            SlotShare::<F, E>::new_pair(slots.len(), ext_share);
        let slot = unsafe {
            Slot::with_raw(slot_share1, ext)
        };

        //安全性：Slot来源于group内部
        unsafe{ slots.push(slot);}
        //安全性：slot_share来源于group内部
        unsafe { Reporter::from_raw(self.group.clone(), slot_share2) }.into()
    }

    pub fn guard(&self) -> &InLockSharedGuard<'a, 'data, F, E> {
        &self.guard
    }

    pub fn group(&self) -> & &'a mut DownloadGroup<'data, F, E> {
        & self.group
    }
    ///安全性：保证不从该方法返回的结构体（及其子结构体）内写入任何不合法内容
    pub unsafe fn guard_mut(&mut self) -> &mut InLockSharedGuard<'a, 'data, F, E> {
        &mut self.guard
    }
    ///安全性：保证不从该方法返回的结构体（及其子结构体）内写入任何不合法内容
    pub unsafe fn group_mut(&mut self) -> &mut &'a mut  DownloadGroup<'data, F, E> {
        &mut self.group
    }

    pub fn slot(&self) -> & Vec<Slot<'data, F, E>> {
        &self.guard.slots
    }

    pub unsafe fn slot_mut(&mut self) -> &mut Vec<Slot<'data, F, E>> {
        &mut self.guard.slots
    }

    pub fn in_lock_ext(&self) -> & E::InLockExt<'data> {
        &self.guard.ext
    }

    pub fn in_lock_ext_mut(&mut self) -> &mut E::InLockExt<'data> {
        &mut self.guard.ext
    }
}

///reporter WriteGuard
/// 可以访问 GroupShareExt, MySlotShareExt, InLockExt, MySlotInLockExt, SlotVector
pub struct ReporterGuard<'a, 'data, F: ThreadModel, E: GroupExt<F>> {
    //leak &mut of this field will cause UB
    group_guard: GroupGuard<'a, 'data, F, E>,
    //leak &mut of this field will cause UB
    slot_share: &'a F::RefCounter<SlotShare<'data, F, E>>,
}

impl<'a, 'data, F, E> ReporterGuard<'a, 'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    unsafe fn from_raw(
        group_guard: GroupGuard<'a, 'data, F, E>,
        slot_share: &'a F::RefCounter<SlotShare<'data, F, E>>,
    ) -> Self {
        Self {
            group_guard,
            slot_share,
        }
    }
    pub fn group_guard(&self) -> & GroupGuard<'a, 'data, F, E> {
        & self.group_guard
    }

    pub fn group(&self) -> & &mut DownloadGroup<'data, F, E> {
        self.group_guard().group()
    }

    pub fn guard(&self) -> & InLockSharedGuard<'a, 'data, F, E> {
        self.group_guard().guard()
    }

    ///安全性：保证不从该方法返回的结构体（及其子结构体）内写入任何不合法内容
    pub unsafe fn group_guard_mut(&mut self) -> &mut GroupGuard<'a, 'data, F, E>{
        &mut self.group_guard
    }

    pub unsafe fn group_mut(&mut self) -> &mut &'a mut DownloadGroup<'data, F, E> {
        unsafe { self.group_guard_mut().group_mut() }
    }

    pub unsafe fn guard_mut(&mut self) -> &mut InLockSharedGuard<'a, 'data, F, E> {
        unsafe { self.group_guard_mut().guard_mut() }
    }

    //---------------------从GroupGuard中继承的方法：--------------

    pub fn new_reporter(
        &mut self,
        ext: E::SlotInlockExt<'data>,
        ext_share: E::SlotShareExt<'data>,
    ) -> Option<Reporter<'data, F, E>> {
        self.group_guard.new_reporter(ext, ext_share)
    }

    pub fn in_lock_ext(&self) -> &E::InLockExt<'data> {
        self.group_guard.in_lock_ext()
    }

    pub fn in_lock_ext_mut(&mut self) -> &mut E::InLockExt<'data> {
        self.group_guard.in_lock_ext_mut()
    }

    //-------------------------独有方法----------------------------------
    // pub fn my_index(&self) -> &usize {

    // }
    //SAFE
    pub(crate) fn my_index_mut(&mut self) -> &mut usize {
        //Safety: 我们有guard，逻辑上拥有index字段的所有权
        unsafe { &mut *self.slot_share.index.0.get() }
    }

    

    pub fn remove_me(&mut self) -> Slot<'data, F, E> {
        let index = *self.my_index_mut();
        let slot = self.group_guard.guard.slots_mut().unwrap();
        slot.swap_remove_and_update_index(index)
    }

    pub fn my_slot_ext(&self) -> &E::SlotInlockExt<'data> {
        self.my_slot_mut().slot_ext()
    }

    pub fn my_slot_ext_mut(&mut self) -> &mut E::SlotInlockExt<'data> {
        todo!()
    }


}

//---------------------------------封装的公开api---------------------

// #[derive(Clone, Debug, Default)]
// struct SlotVec<'data, F, E> (
//     pub Vec<Slot<'data, F, E>>
// )
// where F: ThreadModel, E: GroupExt<F>;

// impl<'data, F, E> SlotVec<'data, F, E> 
// where F: ThreadModel, E: GroupExt<F>{
//     fn swap_remove_and_update_index(&mut self, index: usize) -> Slot<'data, F, E> {
//         let removed = self.0.swap_remove(index);

//         if index != self.0.len() {
//             *self.as_mut_slot_slice().get_index_mut(index) = index;
//         }
//         removed
//     }

//     fn as_slot_slice(&self) -> &SlotSlice<'data, F, E> {
//         unsafe {
//             std::mem::transmute(self.0.as_slice())
//         }
//     }

//     fn as_mut_slot_slice(&mut self) -> &mut SlotSlice<'data, F, E> {
//         unsafe {
//             std::mem::transmute(self.0.as_mut_slice())
//         }
//     }

//     fn push_slot(&mut self, slot: Slot<'data, F, E>) {
//         self.0.push(slot);
//     }
// }

// impl<'data, F, E> Deref for SlotVec<'data, F, E> 
// where F: ThreadModel, E: GroupExt<F>
// {
//     type Target = SlotSlice<'data, F, E>;
//     fn deref(&self) -> &Self::Target {
//         self.as_slot_slice()
//     }
// }

// impl <'data, F, E> DerefMut for SlotVec<'data, F, E>
// where F:ThreadModel, E: GroupExt<F>
// {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         self.as_mut_slot_slice()
//     }
// }

///可以访问my_index, SlotShareExt, SlotInLockShareExt
impl<'a, F, E> Slot<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    //安全性：承诺Slot逻辑上拥有SlotShare内index字段的所有权
    unsafe fn with_raw(share: RefCounter<F, SlotShare<'a, F, E>>, ext: E::SlotInlockExt<'a>) -> Self {
        Self { share, ext }
    }

    fn index(&self) -> &usize {
        //安全性：因为with_raw是unsafe的
        unsafe { & *self.share.index.0.get() }
    }

    fn index_mut(&mut self) -> &mut usize {
        unsafe { &mut *self.share.index.0.get() }
    }

    pub fn share(&self) -> &RefSlotShare<'a, F, E> {
        &self.share
    }

    pub unsafe fn share_mut(&mut self) -> &mut RefSlotShare<'a, F, E> {
        &mut self.share
    }

    pub fn slot_inlock_ext(&self) -> &E::SlotInlockExt<'a> {
        &self.ext
    }

    pub fn slot_inlock_ext_mut(&mut self) -> &mut E::SlotInlockExt<'a> {
        &mut self.ext
    }
}


//-----------------------------------------inner impl-------------------------------

///专为下载任务特化的任务管理器，运行时无关
struct GroupShared<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    //leak &mut of this field will cause UB
    locked: F::Mutex<InLockShared<'a, F, E>>,

    //leak &mut of this field is Safe
    pub ext: E::ShareExt<'a>,
}

impl<'a, F: ThreadModel, E: GroupExt<F>> GroupShared<'a, F, E> {
    fn new(share_ext: E::ShareExt<'a>, inlock_ext: E::InLockExt<'a>) -> Self{
        let t = InLockShared{
            slots: None,
            ext: inlock_ext
        };
        Self { locked: F::Mutex::new(t), ext: share_ext }
    }
    fn locked(&self) -> &F::Mutex<InLockShared<'a, F, E>> {
        &self.locked
    }

    fn locked_mut(&mut self) -> &mut F::Mutex<InLockShared<'a, F, E>> {
        &mut self.locked
    }
}
type RefGroupShared<'data, F: ThreadModel, E: GroupExt<F>> =
    F::RefCounter<GroupShared<'data, F, E>>;

struct InLockShared<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    slots: Vec<Slot<'a, F, E>>, // or Box<[Slot]>?
    pub ext: E::InLockExt<'a>,
}
//pub type InLockSharedMut<
pub type InLockSharedGuard<'a, 'data, F: ThreadModel, E: GroupExt<F>> =
    <F::Mutex<InLockShared<'data, F, E>> as Lockable>::Guard<'a>;

impl<'a, F, E> InLockShared<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{

    fn new(inlock_ext: E::InLockExt<'a>) -> Self{
        Self{
            slots:Vec::new(),
            ext: inlock_ext
        }   
    }
}


/// 内部存储项
/// &mut of Self will cause UB
pub(crate) struct Slot<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    //leak &mut of this field will cause UB
    share: RefCounter<F, SlotShare<'a, F, E>>,

    pub ext: E::SlotInlockExt<'a>,
}



struct SlotShare<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    index: SyncUnsafeCell<usize>,
    //leak &mut of this field is inposeable
    pub ext: E::SlotShareExt<'data>,
}
type RefSlotShare<'data, F: ThreadModel, E: GroupExt<F>> = F::RefCounter<SlotShare<'data, F, E>>;

impl<'data, F, E> SlotShare<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn new_pair(
        index: usize,
        ext: E::SlotShareExt<'data>,
    ) -> (
        F::RefCounter<SlotShare<'data, F, E>>,
        F::RefCounter<SlotShare<'data, F, E>>,
    ) {
        let share: F::RefCounter<Self> = F::RefCounter::new(SlotShare {
            index: index.into(),
            ext,
        });
        (share.clone(), share)
    }
}

//--------------------------LockedGuards---------------------------

pub trait GroupExt<F: ThreadModel>: 'static + Copy {
    type ShareExt<'data>;
    type InLockExt<'data>;
    type SlotShareExt<'data>;
    type SlotInlockExt<'data>;
}

///还不知道具体怎么用
trait ProcessRecordKind {
    type State;
    type Downloaded<T>: Radium<Item = T>;
    type Writed<T>: Radium<Item = T>;

    fn report_downloaded_len(len: u64);
}

type ExtElement<'a, F, E: GroupExt<F>> = (E::SlotInlockExt<'a>, E::SlotShareExt<'a>);

struct ExtHander<'a, E: GroupExt<F>, F: ThreadModel> {
    group_share: &'a E::ShareExt<'a>, //leak &mut of this field will cause UB
    slot_share: &'a E::SlotShareExt<'a>,   //leak &mut of this field will cause UB
}

///标准库SyncUnsafeCell还未稳定
struct SyncUnsafeCell<T>(UnsafeCell<T>);

unsafe impl<T: Sync> Sync for SyncUnsafeCell<T> {}

impl<T> From<T> for SyncUnsafeCell<T> {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}
