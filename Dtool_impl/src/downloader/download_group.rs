//!定义分块下载的并发结构体
//!

use std::cell::UnsafeCell;
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
pub struct DownloadGroup<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    //leak &mut of this field is safe
    pub inner: F::RefCounter<GroupShared<'data, F, E>>, //different from state reporter, this field can be pub
}

impl<'data, F, E> DownloadGroup<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    pub fn new(share_ext: E::GroupShareExt<'data>, inlock_ext: E::InLockShareExt<'data>) -> Self{
        Self::from_raw(F::RefCounter::new(GroupShared::new(share_ext, inlock_ext)))
    }
    pub(crate) fn from_raw(inner: F::RefCounter<GroupShared<'data, F, E>>) -> Self {
        Self { inner }
    }

    pub fn lock<'a>(&'a self) -> GroupGuard<'a, 'data, F, E> {
        let guard = self.inner.locked.lock();
        unsafe { GroupGuard::from_raw(&self, guard) }
    }

    pub fn share_ext(&self) -> &E::GroupShareExt<'data> {
        &self.inner.ext
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
        //only read
        &self.slot_share
    }

    pub(crate) fn group(&self) -> &DownloadGroup<'data, F, E> {
        &self.group
    }

    pub(crate) fn group_share(&self) -> &GroupShared<'data, F, E> {
        &self.group.inner
    }

    pub fn share_ext(&self) -> &E::GroupShareExt<'data> {
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
pub struct GroupGuard<'a, 'data, F, E>
where
    //'data: 'a,
    <F as ThreadModel>::Mutex<InLockShared<'data, F, E>>: 'a, // 满足 Lockable Trait 的 GAT 约束
    F: ThreadModel,
    E: GroupExt<F>,
{
    //leak &mut of this field will cause UB
    group: &'a DownloadGroup<'data, F, E>,
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
        group: &'a DownloadGroup<'data, F, E>,
        guard: InLockSharedGuard<'a, 'data, F, E>,
    ) -> Self {
        Self { group, guard }
    }

    ///move to lockedgroup
    pub fn new_reporter(
        &mut self,
        ext: E::SlotExt<'data>,
        ext_share: E::SlotShareExt<'data>,
    ) -> Option<Reporter<'data, F, E>> {
        let mut slots = self.slots_mut()?;

        let (slot_share1, slot_share2) =
            SlotShare::<F, E>::new_pair(slots.len(), ext_share);
        let slot = Slot::with_raw(slot_share1, ext);

        //安全性：Slot来源于group内部
        unsafe{ slots.push(slot);}
        //安全性：slot_share来源于group内部
        unsafe { Reporter::from_raw(self.group.clone(), slot_share2) }.into()
    }

    pub fn in_lock_ext(&self) -> &E::InLockShareExt<'data> {
        &self.guard.ext
    }

    pub fn in_lock_ext_mut(&mut self) -> &mut E::InLockShareExt<'data> {
        &mut self.guard.ext
    }

    pub fn slots(&self) -> Option<&Vec<Slot<'data, F, E>>> {
        self.guard.slots.as_ref()
    }

    pub fn slots_mut(&mut self) -> Option<SlotVectorMut<'_, 'data, F, E>> {
        //self.guard.slots.as_mut().map(SlotVectorMut)
        self.slots_optional().take()
    }

    pub fn slots_optional(&mut self) -> OptionalSlotVecMut<'_, 'data, F, E> {
        OptionalSlotVecMut(&mut self.guard.slots)
    }
}

///reporter WriteGuard
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

    //---------------------从GroupGuard中继承的方法：--------------

    pub fn new_reporter(
        &mut self,
        ext: E::SlotExt<'data>,
        ext_share: E::SlotShareExt<'data>,
    ) -> Option<Reporter<'data, F, E>> {
        self.group_guard.new_reporter(ext, ext_share)
    }

    pub fn in_lock_ext(&self) -> &E::InLockShareExt<'data> {
        self.group_guard.in_lock_ext()
    }
    pub fn in_lock_ext_mut(&mut self) -> &mut E::InLockShareExt<'data> {
        self.group_guard.in_lock_ext_mut()
    }

    pub(crate) fn slots(&self) -> Option<&Vec<Slot<'data, F, E>>> {
        self.group_guard.slots()
    }

    fn slots_mut<'tmp>(&'tmp mut self) -> Option<SlotVectorMut<'tmp, 'data, F, E>> {
        self.group_guard.slots_mut()
    }
    //-------------------------独有方法----------------------------------

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

    fn get_my_slot(&mut self) -> SlotMut<'_, 'data, F, E> {
        let index = *self.my_index_mut();
        self.slots_mut()
            .unwrap()
            .into_slot_slice_mut()
            .into_element_mut(index)
            .unwrap()
    }
}

//---------------------------------封装的公开api-------------------
struct OptionalSlotVecMut<'a, 'data, F, E>(&'a mut Option<Vec<Slot<'data, F, E>>>) where F: ThreadModel, E: GroupExt<F>;

impl<'a, 'data, F, E> OptionalSlotVecMut<'a, 'data, F, E> 
where F: ThreadModel, E: GroupExt<F>
{
    fn take(self) -> Option<SlotVectorMut<'a, 'data, F, E>>{
        match self.0 {
            Some(r) => SlotVectorMut(r).into(),
            None => None
        }
    }

    fn set_none(self) {
        *self.0 = None
    }

    fn set_empty(self) {
        *self.0 = Some(Vec::new())
    }

    unsafe fn into_mut(self) -> &'a mut Option<Vec<Slot<'data, F, E>>>{
        self.0
    }

    fn and_then(self, f: impl FnOnce(Option<SlotVectorMut<'a, 'data,F, E>>) -> Option<SlotVectorMut<'a, 'data,F, E>>) -> Self {
        let p = f(self.take()).map(|p| unsafe {
            p.into_mut()
        });
        Self(p)
    }
}

struct SlotVectorMut<'a, 'data, F: ThreadModel, E: GroupExt<F>>(&'a mut Vec<Slot<'data, F, E>>);

impl<'a, 'data, F, E> SlotVectorMut<'a, 'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn into_ref(self) -> &'a Vec<Slot<'data, F, E>> {
        self.0
    }

    fn into_slot_slice_mut(self) -> SlotSliceMut<'a, 'data, F, E> {
        SlotSliceMut(&mut *self.0)
    }

    fn swap_remove(&mut self, index: usize) -> Slot<'data, F, E> {
        self.0.swap_remove(index)
    }

    fn swap_remove_and_update_index(mut self, index: usize) -> Slot<'data, F, E> {
        let removed = self.swap_remove(index);

        if index != self.len() {
            *self.into_slot_index_mut(index) = index;
        }
        removed
    }

    fn into_slot_index_mut(self, index: usize) -> &'a mut usize {  //TODO: 移动到MutSliceSlot
        self.into_slot_slice_mut().get_slot_index_mut(index)
    }
    
    fn get_slot_index_ref(&self, index: usize) -> &usize {
        get_slot_index_ref(self.0.as_slice(), index)
    }

    ///安全性：将其他来源的Slot推入列表会触发UB
    unsafe fn push(&mut self, value: Slot<'data, F, E>) {
        self.0.push(value);
    }


    ///安全性：不将&mut T指针中的内容移出
    unsafe fn into_mut(self) -> &'a mut Vec<Slot<'data, F, E>> {
        self.0
    }
}

impl<'a, 'data, F, E> Deref for SlotVectorMut<'a, 'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    type Target = Vec<Slot<'data, F, E>>;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}
struct SlotSliceMut<'a, 'data, F, E>(&'a mut [Slot<'data, F, E>])
where
    F: ThreadModel,
    E: GroupExt<F>;

impl<'a, 'data, F, E> SlotSliceMut<'a, 'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    pub fn into_ref(self) -> &'a [Slot<'data, F, E>] {
        self.0
    }

    pub fn into_element_mut(self, index: usize) -> Option<SlotMut<'a, 'data, F, E>> {
        self.0.get_mut(index).map(SlotMut)
    }

    pub fn into_iter_mut(self) -> SlotsIterMut<'a, 'data, F, E> {
        SlotsIterMut(self.0.iter_mut())
    }

    pub fn into_sub_slice_mut<R>(self, range: R) -> Self
    where
        R: SliceIndex<[Slot<'data, F, E>], Output = [Slot<'data, F, E>]>,
    {
        let a = self.0.index_mut(range);
        Self(a)
    }

    ///从逻辑上只要拥有guard就拥有内部所有index字段的所有权
    /// 这也是为什么往vec中添加元素是Unsafe操作
    fn get_slot_index_mut(self, index: usize) -> &'a mut usize {  //TODO: 移动到MutSliceSlot
        //Safety: 我们有一个独占的&mut self，
        unsafe { &mut *self[index].get_index() }
    }

    fn get_slot_index_ref(&self, index: usize) -> &usize {
        get_slot_index_ref(self.0, index)
    }

    fn get_mut<'tmp>(&'tmp mut self, index: usize) -> Option<SlotMut<'tmp, 'data, F, E>> {
        self.0.get_mut(index).map(SlotMut)
    }
}

///与get_slot_index_mut方法相同
fn get_slot_index_ref<'data, F: ThreadModel, E: GroupExt<F>>(slice: &[Slot<'data, F, E>], index: usize) -> &'data usize {
    //Safety: 我们有&self，保证没有其他的&mut self
    unsafe { &*slice[index].get_index() }
}

impl<'a, 'data, F, E> Deref for SlotSliceMut<'a, 'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    type Target = [Slot<'data, F, E>];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

use std::slice::IterMut;

struct SlotsIterMut<'a, 'data, F, E>(IterMut<'a, Slot<'data, F, E>>)
where
    F: ThreadModel,
    E: GroupExt<F>;

impl<'a, 'data, F, E> Iterator for SlotsIterMut<'a, 'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    type Item = SlotMut<'a, 'data, F, E>;
    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(SlotMut)
    }
}

impl<'a, 'data, F, E> DoubleEndedIterator for SlotsIterMut<'a, 'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        self.0.next_back().map(SlotMut)
    }
}
impl<'a, 'data, F, E> SlotsIterMut<'a, 'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn into_mut_slice(self) -> SlotSliceMut<'a, 'data, F, E> {
        // 因为IterMut的as_mut_slice方法还不稳定，我们用这个方法代替
        let slice = self.0.as_slice();
        // 获取底层指针并转为可变指针
        let ptr = slice.as_ptr() as *mut Slot<'data, F, E>;
        let len = slice.len();

        // 安全性：&mut self是独占的，且生命周期受当前 &mut self 限制
        SlotSliceMut(unsafe { std::slice::from_raw_parts_mut(ptr, len) })
    }

    fn as_mut_slice(&mut self) -> SlotSliceMut<'_, 'data, F, E> {
        // 因为IterMut的as_mut_slice方法还不稳定，我们用这个方法代替
        let slice = self.0.as_slice();
        // 获取底层指针并转为可变指针
        let ptr = slice.as_ptr() as *mut Slot<'data, F, E>;
        let len = slice.len();

        // 安全性：&mut self是独占的，且生命周期受当前 &mut self 限制
        SlotSliceMut(unsafe { std::slice::from_raw_parts_mut(ptr, len) })
    }
}

impl<'a, 'data, F, E> Deref for SlotsIterMut<'a, 'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    type Target = IterMut<'a, Slot<'data, F, E>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
// ///泛型的迭代器
// struct SlotIterMutGen<I>(I);

// impl<I> SlotIterMutGen<I> {
//     fn new(iter: I) -> Self {
//         Self(iter)
//     }
// }

// impl<'a, 'data, F, E, I> Iterator for SlotIterMutGen<I>
// where
//     'data: 'a,
//     I: Iterator<Item = &'a mut Slot<'data, F, E>>,
//     F: ThreadModel,
//     E: GroupExt<F>,
// {
//     type Item = SlotMut<'a, 'data, F, E>;
//     fn next(&mut self) -> Option<Self::Item> {
//         self.0.next().map(SlotMut)
//     }
// }

struct SlotMut<'a, 'data, F, E>(&'a mut Slot<'data, F, E>)
where
    F: ThreadModel,
    E: GroupExt<F>;

impl<'a, 'data, F, E> SlotMut<'a, 'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    pub fn slot_ext(&self) -> &E::SlotExt<'data> {
        &self.0.ext
    }

    pub fn slot_ext_mut(&mut self) -> &mut E::SlotExt<'data> {
        &mut self.0.ext
    }

    pub fn slot_share_ext(&self) -> &E::SlotShareExt<'data> {
        &self.0.share.ext
    }

    fn get_my_index(&self) -> *mut usize {
        self.0.share.index.0.get()
    }

    pub fn as_ref<'tmp>(&'tmp self) -> &'tmp Slot<'data, F, E> {
        self.0
    }
}

impl<'a, 'data, F, E> From<&'a mut Slot<'data, F, E>> for SlotMut<'a, 'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn from(value: &'a mut Slot<'data, F, E>) -> Self {
        Self(value)
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
    pub ext: E::GroupShareExt<'a>,
}

impl<'a, F: ThreadModel, E: GroupExt<F>> GroupShared<'a, F, E> {
    fn new(share_ext: E::GroupShareExt<'a>, inlock_ext: E::InLockShareExt<'a>) -> Self{
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
    //leak &mut of this field will cause UB
    //leak &mut of Slot will cau
    //移出元素是安全的，但添加元素不是
    slots: Option<Vec<Slot<'a, F, E>>>, // or Box<[Slot]>?

    //leak &mut of this field is Safe
    pub ext: E::InLockShareExt<'a>,
}
//pub type InLockSharedMut<
pub type InLockSharedGuard<'a, 'data, F: ThreadModel, E: GroupExt<F>> =
    <F::Mutex<InLockShared<'data, F, E>> as Lockable>::Guard<'a>;

impl<'a, F, E> InLockShared<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{

    fn new(inlock_ext: E::InLockShareExt<'a>) -> Self{
        Self{
            slots: Some(Vec::new()),
            ext: inlock_ext
        }   
    }

    fn abort(&mut self) {
        self.slots = None;
    }
    
    fn slots(&self) -> Option<&Vec<Slot<'a, F, E>>> {
        self.slots.as_ref()
    }

    fn slots_mut(&mut self) -> Option<SlotVectorMut<'_, 'a, F, E>> {
        self.slots.as_mut().map(SlotVectorMut)
    }
}


/// 内部存储项
/// &mut of Self will cause UB
#[derive(Clone)]
pub(crate) struct Slot<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    //leak &mut of this field will cause UB
    share: RefCounter<F, SlotShare<'a, F, E>>,

    //leak &mut of this field is safe
    ext: E::SlotExt<'a>,
}

impl<'a, F, E> Slot<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn with_raw(share: RefCounter<F, SlotShare<'a, F, E>>, ext: E::SlotExt<'a>) -> Self {
        Self { share, ext }
    }

    fn get_index(&self) -> *mut usize {
        self.share.index.0.get()
    }

    pub fn share(&self) -> &RefSlotShare<'a, F, E> {
        &self.share
    }

    pub unsafe fn share_mut(&mut self) -> &mut RefSlotShare<'a, F, E> {
        &mut self.share
    }
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

mod group_ext {}
pub trait GroupExt<F: ThreadModel>: 'static + Copy {
    type GroupShareExt<'data>;
    type InLockShareExt<'data>;
    type SlotShareExt<'data>;
    type SlotExt<'data>;
}

impl<'a, F, E> Deref for GroupShared<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    type Target = E::GroupShareExt<'a>;
    fn deref(&self) -> &Self::Target {
        &self.ext
    }
}
impl<'a, F, E> DerefMut for GroupShared<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.ext
    }
}
///
impl<'a, F, E> Deref for InLockShared<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    type Target = E::InLockShareExt<'a>;
    fn deref(&self) -> &Self::Target {
        &self.ext
    }
}

impl<'a, F, E> DerefMut for InLockShared<'a, F, E>
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
///还不知道具体怎么用
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

///标准库SyncUnsafeCell还未稳定
struct SyncUnsafeCell<T>(UnsafeCell<T>);

unsafe impl<T: Sync> Sync for SyncUnsafeCell<T> {}

impl<T> From<T> for SyncUnsafeCell<T> {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}
