//!定义分块下载的并发结构体
//!

use std::cell::UnsafeCell;
use std::mem;
use std::{
    ops::{Index, IndexMut, RangeFrom},
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
        Self::from_raw(F::RefCounter::new(GroupShared::new(share_ext, inlock_ext)))
    }
    pub(crate) fn from_raw(inner: F::RefCounter<GroupShared<'data, F, E>>) -> Self {
        Self(inner)
    }

    pub fn lock<'a>(self) -> GroupGuard<'data, F, E> {
        let share = self.0;
        let guard: InLockSharedGuard<'data, 'data, F, E> =
            unsafe { std::mem::transmute(share.locked.lock()) };
        unsafe { GroupGuard::from_raw(share, guard) }
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
    E: GroupExt<F>
{
    let (share1 , share2 ) = SlotShare::<F, E>::new_pair(slots.len(), slot_ext);
    let slot = unsafe{ Slot::with_raw(share1, slot_inlock) };
    slots.push_slot(slot);
    unsafe{ Reporter::from_raw(share.clone(), share2) }
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
    ///有&mut
    fn new_in(
        slot: &mut SlotVec<'data, F, E>,
        slot_ext: E::SlotExt<'data>,
        slot_inlock: E::SlotInlockExt<'data>,
    ) {
    }
    unsafe fn from_raw(
        group: F::RefCounter<GroupShared<'data, F, E>>,
        slot_share: RefSlotShare<'data, F, E>,
    ) -> Self {
        Self { group, slot_share }
    }

    ///Aquare Lock
    pub fn lock(self) -> ReporterGuard<'data, F, E> {
        let guard = self.group.locked.lock();
        let guard: InLockSharedGuard<'data, 'data, F, E> = unsafe { std::mem::transmute(guard) };
        unsafe { ReporterGuard::from_raw(self.group, self.slot_share, guard) }
    }

    ///GroupExt
    pub fn share_ext(&self) -> &E::GroupExt<'data> {
        &self.group.ext
    }
    pub fn slot_share_ext(&self) -> &E::SlotExt<'data> {
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
    guard: InLockSharedGuard<'data, 'data, F, E>, //<F::Mutex<InLockShare<'data, F, E>> as Lockable>::Guard<'a>,
}

//fn new_reporter()
/// 可以访问&GroupShareExt, &mut InLockExt, unsafe &mut SlotVector
impl<'data, F, E> GroupGuard<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    ///安全性：确保guard来自group内部
    pub unsafe fn from_raw<'a>(
        group: F::RefCounter<GroupShared<'data, F, E>>,
        guard: InLockSharedGuard<'a, 'data, F, E>,
    ) -> Self {
        let guard: InLockSharedGuard<'data, 'data, F, E> = std::mem::transmute(guard);
        Self { group, guard }
    }

    pub fn release_lock(self) -> DownloadGroup<'data, F, E> {
        DownloadGroup(self.group)
    }

    ///move to lockedgroup
    pub fn new_reporter(
        &mut self,
        slot_inlock: E::SlotInlockExt<'data>,
        slot_share: E::SlotExt<'data>,
    ) -> Reporter<'data, F, E> {
        let slots = unsafe { self.slot_vec_mut() };

        let (slot_share1, slot_share2) = SlotShare::<F, E>::new_pair(slots.len(), slot_share);
        let slot = unsafe { Slot::with_raw(slot_share1, slot_inlock) };

        //安全性：Slot来源于group内部
        slots.push_slot(slot);
        //安全性：slot_share来源于group内部
        unsafe { Reporter::from_raw(self.group.clone(), slot_share2) }
    }

    ///GroupExt
    pub fn group_ext(&self) -> &E::GroupExt<'data> {
        &self.group.ext
    }

    ///InLockExt
    pub fn inlock_ext(&self) -> &E::InLockExt<'data> {
        &self.guard.ext
    }
    pub fn inlock_ext_mut(&mut self) -> &mut E::InLockExt<'data> {
        &mut self.guard.ext
    }

    ///SlotVec
    pub fn slot_vec(&self) -> &SlotVec<'data, F, E> {
        &self.guard.slots
    }
    pub unsafe fn slot_vec_mut(&mut self) -> &mut SlotVec<'data, F, E> {
        &mut self.guard.slots
    }
}

///reporter WriteGuard
pub struct ReporterGuard<'data, F: ThreadModel, E: GroupExt<F>> {
    slot_share: F::RefCounter<SlotShare<'data, F, E>>,
    group_share: F::RefCounter<GroupShared<'data, F, E>>,
    guard: InLockSharedGuard<'data, 'data, F, E>,
}

/// 可以访问 &GroupExt, &MySlotExt, &mut InLockExt, &mut MySlotInLockExt, &mut SlotVector(unsafe)
impl<'data, F, E> ReporterGuard<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    unsafe fn from_raw<'a>(
        group_share: F::RefCounter<GroupShared<'data, F, E>>,
        slot_share: F::RefCounter<SlotShare<'data, F, E>>,
        guard: InLockSharedGuard<'a, 'data, F, E>,
    ) -> Self {
        Self {
            group_share,
            slot_share,
            guard: std::mem::transmute(guard),
        }
    }

    fn release_lock(self) -> Reporter<'data, F, E> {
        unsafe { Reporter::from_raw(self.group_share, self.slot_share) }
    }
    pub fn new_reporter(
        &mut self,
        slot_inlock: E::SlotInlockExt<'data>,
        slot_ext: E::SlotExt<'data>,
    ) -> Reporter<'data, F, E> {
        unsafe{ new_reporter(&self.group_share, &mut self.guard.slots, slot_ext, slot_inlock) }
    }
    /// # example
    /// '''rust
    /// reporter_guard_1.move_guard(reporter2)
    /// '''
    pub unsafe fn swap_slot_share(&mut self, reporter: &mut Reporter<'data, F, E>) {
        mem::swap(self.slot_share, &mut reporter.slot_share);
    }

    ///GroupExt
    pub fn group_ext(&self) -> &E::GroupExt<'data> {
        &self.group_share.ext
    }

    ///MySlotExt
    pub fn my_slot_ext(&self) -> &E::SlotExt<'data> {
        &self.slot_share.ext
    }

    ///InlockExt
    pub fn in_lock_ext(&self) -> &E::InLockExt<'data> {
        &self.guard.ext
    }
    pub fn in_lock_ext_mut(&mut self) -> &mut E::InLockExt<'data> {
        &mut self.guard.ext
    }

    ///MySlotInlockExt
    pub fn my_slot_in_lock_ext(&self) -> &E::SlotInlockExt<'data> {
        //安全性：已经获得了锁
        let index = unsafe { *self.slot_share.index.get() };
        &self.guard.slots[index].ext
    }
    pub fn my_slot_in_lock_ext_mut(&mut self) -> &mut E::SlotInlockExt<'data> {
        let index = unsafe { *self.slot_share.index.get() };
        &mut self.guard.slots[index].ext
    }

    ///SlotVec
    pub fn slots(&self) -> &SlotVec<'data, F, E> {
        &self.guard.slots
    }
    pub unsafe fn slots_mut(&mut self) -> &mut SlotVec<'data, F, E> {
        &mut self.guard.slots
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
    //leak &mut of this field will cause UB
    locked: F::Mutex<InLockShared<'a, F, E>>,

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
            locked: F::Mutex::new(t),
            ext: share_ext,
        }
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
    slots: SlotVec<'a, F, E>, // or Box<[Slot]>?
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
