//!定义分块下载的并发结构体
//!

use std::cell::UnsafeCell;
use std::ops::{Deref, DerefMut};

use std::sync::atomic::{AtomicUsize, Ordering};

//use crate::downloader::family::AtomicSwapable;

use super::family::{AtomicCell, Lockable, Mutex, RefCounted, RefCounter, ThreadModel};
use radium::Radium;

//
pub struct DownloadGroup<'data, F: ThreadModel, E: GroupExt<F>> {
    pub share: F::RefCounter<GroupShare<'data, F, E>>, //different from state reporter, this field can be pub
}

impl<'data, F: ThreadModel, E: GroupExt<F>> DownloadGroup<'data, F, E> {
    pub fn lock<'a>(&'a self) -> GroupGuard<'a, 'data, F, E> {
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
    fn clone_view(&self) -> Box<[Slot<'a, F, E>]>
    where
        Slot<'a, F, E>: Clone,
    {
        self.slots.clone().into_boxed_slice()
    }

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
#[derive(Clone)]
pub(crate) struct Slot<'a, F: ThreadModel, E: GroupExt<F>> {
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
        let share: F::RefCounter<Self> = F::RefCounter::new(SlotShare {
            index: index.into(),
            ext,
        });
        (share.clone(), share)
    }
}

//-----------------------------------------------------

///每个下载分块向下载组报告状态的结构体
pub struct Reporter<'data, F: ThreadModel, E: GroupExt<F>> {
    share: F::RefCounter<GroupShare<'data, F, E>>, //leak &mut of this field will cause UB
    slot_share: F::RefCounter<SlotShare<'data, F, E>>, //leak &mut of this field will cause UB
}

impl<'data, F: ThreadModel, E: GroupExt<F>> Reporter<'data, F, E> {
    pub fn share(&self) -> &GroupShare<'data, F, E> {
        //only read
        &*self.share
    }

    pub fn slot_share(&self) -> &SlotShare<'data, F, E> {
        //only read
        &self.slot_share
    }

    pub fn lock(&self) -> ReporterGuard<'_, 'data, F, E> {
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

type ExtElement<'a, F, E: GroupExt<F>> = (E::SlotExt<'a>, E::SlotShareExt<'a>);

struct ExtHander<'a, E: GroupExt<F>, F: ThreadModel> {
    group_share: &'a E::GroupShareExt<'a>, //leak &mut of this field will cause UB
    slot_share: &'a E::SlotShareExt<'a>,   //leak &mut of this field will cause UB
}

//--------------------------LockedGuards---------------------------

///groupWriteGuard
pub struct GroupGuard<'a, 'data, F: ThreadModel, E: GroupExt<F>> {
    //leak &mut of this field will cause UB
    group: &'a mut DownloadGroup<'data, F, E>, //改为& RefCell<GroupShare<'data, F, E>>
    //leak &mut of this field will cause UB
    guard: <F::Mutex<InLockShare<'data, F, E>> as Lockable>::Guard<'a>,
}

impl<'a, 'data, F: ThreadModel, E: GroupExt<F>> GroupGuard<'a, 'data, F, E> {
    ///move to lockedgroup
    pub fn new_reporter(
        &mut self,
        ext: E::SlotExt<'data>,
        ext_share: E::SlotShareExt<'data>,
    ) -> Reporter<'data, F, E> {
        let (share1, share2) = SlotShare::<F, E>::new_pair(self.guard.slots.len(), ext_share);
        self.guard.push(Slot::with_raw(share1, ext));
        Reporter {
            share: self.group.share.clone(),
            slot_share: share2,
        }
    }

    pub fn slots(&mut self) -> &mut Vec<Slot<'data, F, E>> {
        &mut self.guard.slots
    }

    pub fn group(&'a mut self) -> &'a mut DownloadGroup<'data, F, E> {
        self.group
    }
}

///reporter WriteGuard
pub struct ReporterGuard<'a, 'data, F: ThreadModel, E: GroupExt<F>> {
    view: &'a Reporter<'data, F, E>, //leak &mut of this field will cause UB
    guard: <F::Mutex<InLockShare<'data, F, E>> as Lockable>::Guard<'a>, //leak &mut of this field will cause UB
}

impl<'a, 'data, F: ThreadModel, E: GroupExt<F>> ReporterGuard<'a, 'data, F, E> {
    pub fn slots(&mut self) -> &mut Vec<Slot<'data, F, E>> {
        &mut self.guard.slots
    }

    pub fn inlock_ext(&mut self) -> &mut E::InLockShareExt<'data> {
        &mut self.guard.ext
    }

    pub fn my_index_mut(&mut self) -> &mut usize {
        //Safety: 我们有guard，逻辑上拥有index字段的所有权
        unsafe { &mut *self.view.slot_share.index.0.get() }
    }

    pub fn remove_me(&mut self) {
        let index = *self.my_index_mut();
        let removed = self.guard.remove(index);
    }
}

///-----------------------------静态标记空结构体-----------------------------------------
pub trait GroupExt<F: ThreadModel>: 'static {
    type GroupShareExt<'a>;
    type InLockShareExt<'a>;
    type SlotShareExt<'a>;
    type SlotExt<'a>;
}

///
impl<'a, F, E> Deref for GroupShare<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    type Target = E::GroupShareExt<'a>;
    fn deref(&self) -> &Self::Target {
        &self.ext
    }
}
impl<'a, F, E> DerefMut for GroupShare<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ext
    }
}
///
impl<'a, F, E> Deref for InLockShare<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    type Target = E::InLockShareExt<'a>;
    fn deref(&self) -> &Self::Target {
        &self.ext
    }
}

impl<'a, F, E> DerefMut for InLockShare<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ext
    }
}
///
impl<'a, F, E> Deref for Slot<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    type Target = E::SlotExt<'a>;
    fn deref(&self) -> &Self::Target {
        &self.ext
    }
}

impl<'a, F, E> DerefMut for Slot<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ext
    }
}
///
impl<'a, F, E> Deref for SlotShare<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    type Target = E::SlotShareExt<'a>;
    fn deref(&self) -> &Self::Target {
        &self.ext
    }
}

impl<'a, F, E> DerefMut for SlotShare<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ext
    }
}

///标准库SyncUnsafeCell还未稳定
struct SyncUnsafeCell<T>(UnsafeCell<T>);

unsafe impl<T: Sync> Sync for SyncUnsafeCell<T> {}

impl<T> From<T> for SyncUnsafeCell<T> {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}
