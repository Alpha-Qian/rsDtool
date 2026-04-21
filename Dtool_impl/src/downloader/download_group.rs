//!定义分块下载的并发结构体
//!

use std::cell::UnsafeCell;
use std::mem::{self, ManuallyDrop};
use std::ops::Deref;
use std::{
    ops::{Index, IndexMut, RangeFrom},
    ptr,
    slice::SliceIndex,
};

use super::family::{Lockable, RefCounted, RefCounter, ThreadModel};
use radium::Radium;

///一个可以看作多生产者多消费者的数据结构
///线程模型通用
///这个结构体相当于消费者
#[derive(Clone)]
pub struct DownloadGroup<'data, F, E>(pub F::RefCounter<GroupShared<'data, F, E>>)
where
    F: ThreadModel,
    E: GroupExt<F>;

//可以访问：&GroupShareExt, Lock
impl<'data, F, E> DownloadGroup<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    pub fn new(share_ext: E::GroupExt<'data>, inlock_ext: E::InLockExt<'data>) -> Self {
        Self(F::RefCounter::new(GroupShared::new(share_ext, inlock_ext)))
    }

    pub(crate) fn from_raw(inner: F::RefCounter<GroupShared<'data, F, E>>) -> Self {
        Self(inner)
    }

    pub fn lock<'a>(self) -> GroupGuard<'data, F, E> {
        GroupGuard::new(self)
    }

    pub fn share_ext(&self) -> &E::GroupExt<'data> {
        &self.0.ext
    }
}
///安全性：保证share指向slot
unsafe fn new_reporter<'data, F, E>(
    share: &F::RefCounter<GroupShared<'data, F, E>>,
    slots: &mut SlotVec<'data, F, E>,
    slot_ext: E::SlotExt<'data>,
    slot_inlock: E::SlotInlockExt<'data>,
) -> Reporter<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    let (share1, share2) = SlotShare::<F, E>::new_pair(slots.len(), slot_ext);
    let slot = unsafe { Slot::with_raw(share1, slot_inlock) };
    slots.push_slot(slot);
    unsafe { Reporter::from_raw(share.clone(), share2) }
}
///每个下载分块向下载组报告状态的结构体
/// 这个结构体是生产者也是消费者
pub struct Reporter<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    group: F::RefCounter<GroupShared<'data, F, E>>,
    slot_share: F::RefCounter<SlotShare<'data, F, E>>,
}

//可以访问：&GroupShareExt, &SlotShareExt, Lock
impl<'data, F, E> Reporter<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    unsafe fn from_raw(
        group: F::RefCounter<GroupShared<'data, F, E>>,
        slot_share: RefSlotShare<'data, F, E>,
    ) -> Self {
        Self { group, slot_share }
    }

    ///Aquare Lock
    pub fn lock(self) -> ReporterGuard<'data, F, E> {
        ReporterGuard::new(self)
    }

    ///GroupExt
    pub fn group(&self) -> &E::GroupExt<'data> {
        &self.group.ext
    }
    pub fn slot_ext(&self) -> &E::SlotExt<'data> {
        &self.slot_share.ext
    }
}

///groupWriteGuard
pub struct GroupGuard<'data, F, E>
where
    //<F as ThreadModel>::Mutex<InLockShared<'data, F, E>>: 'a, // 满足 Lockable Trait 的 GAT 约束
    F: ThreadModel,
    E: GroupExt<F>,
{
    group: F::RefCounter<GroupShared<'data, F, E>>,
}

//fn new_reporter()
/// 可以访问&GroupShareExt, &mut InLockExt, unsafe &mut SlotVector
impl<'data, F, E> GroupGuard<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{

    pub fn new(group: DownloadGroup<'data, F, E>) -> Self{
        group.0.mutex.acquire();
        Self { group: group.0 }
    }

    pub fn release_lock(self) -> DownloadGroup<'data, F, E> {
        let group = unsafe { ptr::read(&self.group) };
        ManuallyDrop::new(self);
        group.mutex.release();
        DownloadGroup(group)
    }

    ///move to lockedgroup
    pub fn new_reporter(
        &mut self,
        slot_inlock: E::SlotInlockExt<'data>,
        slot_ext: E::SlotExt<'data>,
    ) -> Reporter<'data, F, E> {
        unsafe {
            new_reporter(
                &self.group,
                &mut (*self.group.locked.get()).slots,
                slot_ext,
                slot_inlock,
            )
        }
    }

    ///GroupExt
    pub fn group_ext(&self) -> &E::GroupExt<'data> {
        &self.group.ext
    }

    ///InLockExt
    pub fn inlock_ext(&self) -> &E::InLockExt<'data> {
        unsafe { &(*self.group.locked.get()).ext }
    }
    pub fn inlock_ext_mut(&mut self) -> &mut E::InLockExt<'data> {
        unsafe { &mut (*self.group.locked.get()).ext }
    }

    ///SlotVec
    pub fn slot_vec(&self) -> &SlotVec<'data, F, E> {
        unsafe { &(*self.group.locked.get()).slots }
    }
    pub unsafe fn slot_vec_mut(&mut self) -> &mut SlotVec<'data, F, E> {
        unsafe { &mut (*self.group.locked.get()).slots }
    }
}

impl<'data, F: ThreadModel, E: GroupExt<F>> Drop for GroupGuard<'data, F, E> {
    fn drop(&mut self) {
        self.group.mutex.release();
    }
}
///reporter WriteGuard
pub struct ReporterGuard<'data, F: ThreadModel, E: GroupExt<F>> {
    slot_share: F::RefCounter<SlotShare<'data, F, E>>,
    group_share: F::RefCounter<GroupShared<'data, F, E>>,
}

/// 可以访问 &GroupExt, &MySlotExt, &mut InLockExt, &mut MySlotInLockExt, &mut SlotVector(unsafe)
impl<'data, F, E> ReporterGuard<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn new(reporter: Reporter<'data, F, E>) -> Self{
        reporter.group.mutex.acquire();
        unsafe{ Self::from_raw(reporter.group, reporter.slot_share) }
    }
    ///安全性：确保group和slot是成对的
    /// 确保已解锁
    unsafe fn from_raw<'a>(
        group_share: F::RefCounter<GroupShared<'data, F, E>>,
        slot_share: F::RefCounter<SlotShare<'data, F, E>>,
        //guard: InLockSharedGuard<'a, 'data, F, E>,
    ) -> Self {
        Self {
            group_share,
            slot_share,
        }
    }

    pub fn release_lock(self) -> Reporter<'data, F, E> {
        unsafe {
            let slot_share = ptr::read(&self.slot_share);
            let group_share = ptr::read(&self.group_share);
            ManuallyDrop::new(self);
            group_share.mutex.release();
            Reporter::from_raw(group_share, slot_share)
        }
    }
    pub fn new_reporter(
        &mut self,
        slot_inlock: E::SlotInlockExt<'data>,
        slot_ext: E::SlotExt<'data>,
    ) -> Reporter<'data, F, E> {
        unsafe {
            new_reporter(
                &self.group_share,
                &mut (*self.group_share.locked.get()).slots,
                slot_ext,
                slot_inlock,
            )
        }
    }
    /// # example
    /// '''rust
    /// reporter_guard_1.move_guard(reporter2)
    /// '''
    pub unsafe fn swap_slot(&mut self, reporter: &mut Reporter<'data, F, E>) {
        debug_assert_eq!(self.group_share.deref() as *const _, reporter.group.deref() as *const _);

        mem::swap(&mut self.slot_share, &mut reporter.slot_share);
    }

    ///GroupExt
    pub fn group(&self) -> &E::GroupExt<'data> {
        &self.group_share.ext
    }

    ///MySlotExt
    pub fn my_slot_ext(&self) -> &E::SlotExt<'data> {
        &self.slot_share.ext
    }

    ///InlockExt
    pub fn in_lock_ext(&self) -> &E::InLockExt<'data> {
        unsafe { &(*self.group_share.locked.get()).ext }
    }
    pub fn in_lock_ext_mut(&mut self) -> &mut E::InLockExt<'data> {
        unsafe { &mut (*self.group_share.locked.get()).ext }
    }

    ///MySlotInlockExt
    pub fn my_slot_in_lock(&self) -> &E::SlotInlockExt<'data> {
        //安全性：已经获得了锁
        unsafe {
            let index = *self.slot_share.index.get();
            &(*self.group_share.locked.get()).slots[index].ext
        }
    }
    pub fn my_slot_in_lock_ext_mut(&mut self) -> &mut E::SlotInlockExt<'data> {
        unsafe {
            let index = *self.slot_share.index.get();
            &mut (*self.group_share.locked.get()).slots[index].ext
        }
    }

    ///SlotVec
    pub fn slots(&self) -> &SlotVec<'data, F, E> {
        unsafe { &(*self.group_share.locked.get()).slots }
    }
    pub unsafe fn slots_mut(&mut self) -> &mut SlotVec<'data, F, E> {
        unsafe { &mut (*self.group_share.locked.get()).slots }
    }
}

impl<'data, F, E> Drop for ReporterGuard<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn drop(&mut self) {
        self.group_share.mutex.release();
    }
}
//#[derive(Clone, Debug, Default)]
///安全性；不得添加非法内容
struct SlotVec<'data, F, E>(pub Vec<Slot<'data, F, E>>)
where
    F: ThreadModel,
    E: GroupExt<F>;

impl<'data, F, E> SlotVec<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    pub fn swap_remove_and_update_index(&mut self, index: usize) -> Slot<'data, F, E> {
        let removed = self.0.swap_remove(index);

        if index != self.0.len() {
            self.update_index(index);
        }

        removed
    }

    pub fn push_slot(&mut self, slot: Slot<'data, F, E>) {
        self.0.push(slot);
    }

    pub fn update_index(&mut self, index: usize) {
        *self[index].index_mut() = index;
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'data, F, E> Index<usize> for SlotVec<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    type Output = Slot<'data, F, E>;
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

impl<'data, F, E> IndexMut<usize> for SlotVec<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}
impl<'data, F, E> AsRef<Vec<Slot<'data, F, E>>> for SlotVec<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn as_ref(&self) -> &Vec<Slot<'data, F, E>> {
        &self.0
    }
}

impl<'data, F, E> AsMut<Vec<Slot<'data, F, E>>> for SlotVec<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn as_mut(&mut self) -> &mut Vec<Slot<'data, F, E>> {
        &mut self.0
    }
}

///可以访问my_index, SlotShareExt, SlotInLockShareExt
impl<'a, F, E> Slot<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    //安全性：承诺Slot逻辑上拥有SlotShare内index字段的所有权
    unsafe fn with_raw(
        share: RefCounter<F, SlotShare<'a, F, E>>,
        ext: E::SlotInlockExt<'a>,
    ) -> Self {
        Self { share, ext }
    }

    ///Index
    fn index(&self) -> &usize {
        //安全性：因为with_raw是unsafe的
        unsafe { &*self.share.index.0.get() }
    }
    fn index_mut(&mut self) -> &mut usize {
        unsafe { &mut *self.share.index.0.get() }
    }

    ///&SlotShareExt
    pub fn slot_ext(&self) -> &E::SlotExt<'a> {
        &self.share.ext
    }

    ///&mut SlotInLockShareExt
    pub fn slot_inlock_ext(&self) -> &E::SlotInlockExt<'a> {
        &self.ext
    }
    pub fn slot_inlock_ext_mut(&mut self) -> &mut E::SlotInlockExt<'a> {
        &mut self.ext
    }
}

//-----------------------------------------inner impl-----------------------------------

///专为下载任务特化的任务管理器，运行时无关
struct GroupShared<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    mutex: F::Mutex,
    locked: SyncUnsafeCell<InLockShared<'a, F, E>>,

    //leak &mut of this field is Safe
    pub ext: E::GroupExt<'a>,
}

impl<'a, F: ThreadModel, E: GroupExt<F>> GroupShared<'a, F, E> {
    fn new(share_ext: E::GroupExt<'a>, inlock_ext: E::InLockExt<'a>) -> Self {
        let t = InLockShared {
            slots: SlotVec(Vec::new()),
            ext: inlock_ext,
        };
        Self {
            mutex: F::Mutex::new(),
            locked: SyncUnsafeCell::new(t),
            ext: share_ext,
        }
    }

    // fn get_locked(&self) -> *mut InLockShared<'a, F, E> {
    //     self.locked.get()
    // }
}
type RefGroupShared<'data, F: ThreadModel, E: GroupExt<F>> =
    F::RefCounter<GroupShared<'data, F, E>>;

struct InLockShared<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    slots: SlotVec<'a, F, E>, // or Box<[Slot]>?
    pub ext: E::InLockExt<'a>,
}

impl<'a, F, E> InLockShared<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn new(inlock_ext: E::InLockExt<'a>) -> Self {
        Self {
            slots: SlotVec(Vec::new()),
            ext: inlock_ext,
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
    pub ext: E::SlotExt<'data>,
}
type RefSlotShare<'data, F: ThreadModel, E: GroupExt<F>> = F::RefCounter<SlotShare<'data, F, E>>;

impl<'data, F, E> SlotShare<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn new_pair(
        index: usize,
        ext: E::SlotExt<'data>,
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
    type GroupExt<'data>;
    type InLockExt<'data>;
    type SlotExt<'data>;
    type SlotInlockExt<'data>;
}

///还不知道具体怎么用
trait ProcessRecordKind {
    type State;
    type Downloaded<T>: Radium<Item = T>;
    type Writed<T>: Radium<Item = T>;

    fn report_downloaded_len(len: u64);
}

type ExtElement<'a, F, E: GroupExt<F>> = (E::SlotInlockExt<'a>, E::SlotExt<'a>);

struct ExtHander<'a, E: GroupExt<F>, F: ThreadModel> {
    group_share: &'a E::GroupExt<'a>,
    slot_share: &'a E::SlotExt<'a>,
}

///标准库SyncUnsafeCell还未稳定
struct SyncUnsafeCell<T>(UnsafeCell<T>);

impl<T> SyncUnsafeCell<T> {
    fn new(t: T) -> Self {
        Self(UnsafeCell::new(t))
    }
    fn get(&self) -> *mut T {
        self.0.get()
    }
}

unsafe impl<T: Sync> Sync for SyncUnsafeCell<T> {}

impl<T> From<T> for SyncUnsafeCell<T> {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}
