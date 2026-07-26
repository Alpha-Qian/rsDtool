//!定义分块下载的并发结构体
//!
use std::cell::UnsafeCell;
use std::hint::unreachable_unchecked;
use std::marker::PhantomData;
use std::ops::Deref;
use std::task::Waker;
use std::{
    ops::{Index, IndexMut},
    ptr,
};

use crate::base::segment::Segment;

use super::family::{Lockable, RefCounted, RefCounter, ThreadModel};

///一个可以看作多生产者多消费者的数据结构
///线程模型通用
///这个结构体相当于消费者
///接收者持有的结构体
#[derive(Clone)]
#[repr(transparent)]
pub struct DownloadGroup<'a, F, E>(F::RefCounter<GroupShared<'a, F, E>>)
where
    F: ThreadModel,
    E: GroupParts<F>;

/// 每个下载分块向下载组报告状态的结构体
/// 这个结构体是生产者也是消费者
#[derive(Debug, Clone)]
pub struct Reporter<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    pub(crate) group: F::RefCounter<GroupShared<'a, F, E>>,
    pub(crate) slot_share: F::RefCounter<SlotShare<'a, F, E>>,
}

//------------------------Guard struct----------------------------

pub struct GroupGuard<'t, 'a, F, P>(pub(crate) &'t DownloadGroup<'a, F, P>)
where
    F: ThreadModel,
    P: GroupParts<F>;

#[repr(transparent)]
pub struct ReporterGuard<'t, 'a, F, P>(&'t Reporter<'a, F, P>)
where
    F: ThreadModel,
    P: GroupParts<F>;

// -----------Busy and Idle struct  -------------------
#[repr(transparent)]
pub struct BusyGroup<'t, 'a, F, P>(pub(crate) GroupGuard<'t, 'a, F, P>)
where
    F: ThreadModel,
    P: GroupParts<F>;

#[repr(transparent)]
pub struct IdleGroup<'t, 'a, F, P>(GroupGuard<'t, 'a, F, P>)
where
    F: ThreadModel,
    P: GroupParts<F>;

#[repr(transparent)]
pub struct BusyReporter<'t, 'a, F, P>(ReporterGuard<'t, 'a, F, P>)
where
    F: ThreadModel,
    P: GroupParts<F>;

#[repr(transparent)]
pub struct IdleReporter<'t, 'a, F, P>(ReporterGuard<'t, 'a, F, P>)
where
    F: ThreadModel,
    P: GroupParts<F>;

//-------------impl

///可以访问：&GroupShareExt, Lock
///获取锁后返回GroupGuard
impl<'a, F, P> DownloadGroup<'a, F, P>
where
    F: ThreadModel,
    P: GroupParts<F>,
{
    pub fn new_idle(group: P::StaticData<'a>, idle_data: P::Result<'a>) -> Self {
        let in_lock_shared = InLockShared {
            data: group,
            state: State::Idle(IdleSlot { data: idle_data }),
        };

        Self(F::RefCounter::new(GroupShared::with_raw(
            group,
            in_lock_shared,
        )))
    }

    pub(crate) fn new_busy(group: P::StaticData<'a>, busy_data: P::Data<'a>) -> Self {
        let in_lock_shared = InLockShared {
            data: group,
            state: State::Running(BusySlot {
                slots: SlotVec::new(),
                data: busy_data,
            }),
        };

        Self(F::RefCounter::new(GroupShared::with_raw(
            group,
            in_lock_shared,
        )))
    }

    pub fn lock(&self) -> GroupGuard<'_, 'a, F, P> {
        self.0.mutex.acquire();
        unsafe { GroupGuard::new_unchecked(self) }
    }

    pub fn share(&self) -> &P::StaticData<'a> {
        &self.0.share
    }
}

///可以访问：&GroupShareExt, &SlotShareExt, Lock
/// 解锁后可以获取ResporterGuard
impl<'a, F, P> Reporter<'a, F, P>
where
    F: ThreadModel,
    P: GroupParts<F>,
{
    unsafe fn from_raw(
        group: F::RefCounter<GroupShared<'a, F, P>>,
        slot_share: RefSlotShare<'a, F, P>,
    ) -> Self {
        Self { group, slot_share }
    }

    pub fn lock(&self) -> ReporterGuard<'_, 'a, F, P> {
        self.group.mutex.acquire();
        unsafe { ReporterGuard::new_unchecked(self) }
    }

    ///GroupExt
    pub fn group(&self) -> &P::StaticData<'a> {
        &self.group.share
    }

    ///slot ext
    pub fn slot(&self) -> &P::SlotShare<'a> {
        &self.slot_share.ext
    }
}

impl<'t, 'a, F, P> GroupGuard<'t, 'a, F, P>
where
    F: ThreadModel,
    P: GroupParts<F>,
{
    unsafe fn new_unchecked(group: &'t DownloadGroup<'a, F, P>) -> Self {
        Self(group)
    }

    pub fn state_data(&self) -> &State<BusySlot<'a, F, P>, IdleSlot<'a, F, P>> {
        unsafe { &(*self.0.0.locked.get()).state }
    }
    pub fn state_data_mut(&mut self) -> &mut State<BusySlot<'a, F, P>, IdleSlot<'a, F, P>> {
        unsafe { &mut (*self.0.0.locked.get()).state }
    }

    pub fn data(&self) -> &P::Data<'a> {
        unsafe { &(*self.0.0.locked.get()).data }
    }
    pub fn data_mut(&mut self) -> &mut P::Data<'a> {
        unsafe { &mut (*self.0.0.locked.get()).data }
    }

    pub fn state(self) -> State<BusyGroup<'t, 'a, F, P>, IdleGroup<'t, 'a, F, P>> {
        let state = unsafe { &(*self.0.0.locked.get()).state };
        match state {
            State::Running(_) => return State::Running(BusyGroup(self)),
            State::Idle(_) => return State::Idle(IdleGroup(self)),
        }
    }
}

impl<'t, 'a, F, P> ReporterGuard<'t, 'a, F, P>
where
    F: ThreadModel,
    P: GroupParts<F>,
{
    unsafe fn new_unchecked(reporter: &'t Reporter<'a, F, P>) -> Self {
        Self(reporter)
    }

    fn state_slot(&self) -> &State<BusySlot<'a, F, P>, IdleSlot<'a, F, P>> {
        unsafe { &(*self.0.group.locked.get()).state }
    }
    fn state_slot_mut(&mut self) -> &mut State<BusySlot<'a, F, P>, IdleSlot<'a, F, P>> {
        unsafe { &mut (*self.0.group.locked.get()).state }
    }

    pub fn state(self) -> State<BusyReporter<'t, 'a, F, P>, IdleReporter<'t, 'a, F, P>> {
        let state = unsafe { &(*self.0.group.locked.get()).state };
        match state {
            State::Running(_) => State::Running(BusyReporter(self)),
            State::Idle(_) => State::Idle(IdleReporter(self)),
        }
    }

    pub fn index(&self) -> &usize {
        //安全性：已解锁
        unsafe { &*self.0.slot_share.index.get() }
    }

    pub fn index_mut(&mut self) -> &mut usize {
        unsafe { &mut *self.0.slot_share.index.get() }
    }

    fn raw(&self) -> &Reporter<'a, F, P> {
        self.0
    }
}

impl<'t, 'a, F, P> Drop for GroupGuard<'t, 'a, F, P>
where
    F: ThreadModel,
    P: GroupParts<F>,
{
    fn drop(&mut self) {
        self.0.0.mutex.release();
    }
}
impl<'t, 'a, F, P> Drop for ReporterGuard<'t, 'a, F, P>
where
    F: ThreadModel,
    P: GroupParts<F>,
{
    fn drop(&mut self) {
        self.0.group.mutex.release();
    }
}

impl<'t, 'a, F, P> BusyGroup<'t, 'a, F, P>
where
    F: ThreadModel,
    P: GroupParts<F>,
{
    // pub fn from_busy() -> Self {
    //     todo!()
    // }

    pub unsafe fn new_unchecked(groupguard: GroupGuard<'t, 'a, F, P>) -> Self {
        Self(groupguard)
    }

    pub fn slots(&self) -> &SlotVec<'a, F, P> {
        unsafe { &self.0.state_data().busy().unwrap_unchecked().slots }
    }
    pub fn slots_mut(&mut self) -> &mut SlotVec<'a, F, P> {
        unsafe { &mut self.0.state_data().busy().unwrap_unchecked().slots }
    }

    pub fn busy_data(&self) -> &P::Data<'a> {
        unsafe { &self.0.state_data().busy().unwrap_unchecked().data }
    }
    pub fn busy_data_mut(&self) -> &mut P::Data<'a> {
        unsafe { &mut self.0.state_data().busy().unwrap_unchecked().data }
    }

    pub fn into_idle(
        self,
        idle_data: P::Result<'a>,
    ) -> (IdleGroup<'t, 'a, F, P>, SlotVec<'a, F, P>, P::Data<'a>) {
        todo!()
    }

    fn push_slot(&self, slot: Slot<'a, F, P>) {
        self.slots_mut().push_slot(slot);
    }
}

impl<'t, 'a, F, P> IdleGroup<'t, 'a, F, P>
where
    F: ThreadModel,
    P: GroupParts<F>,
{
    // pub fn from_busy() -> Self {
    //     todo!()
    // }
    pub unsafe fn new_unchecked(guard: GroupGuard<'t, 'a, F, P>) -> Self {
        Self(guard)
    }

    pub fn idle_data(&self) -> &P::Result<'a> {
        unsafe { &self.0.state_data().idle().unwrap_unchecked().data }
    }

    pub fn idle_data_mut(&mut self) -> &mut P::Result<'a> {
        unsafe { &mut self.0.state_data().idle().unwrap_unchecked().data }
    }

    // pub unsafe fn into_busy(
    //     self,
    //     busy_data: P::Data<'a>, //slots: SlotVec<'a, F, P>,
    // ) -> (BusyGroup<'t, 'a, F, P>, P::Result<'a>) {
    //     // *self.0.state_data_mut() = State::Busy(BS)
    //     todo!()
    // }
}

impl<'t, 'a, F, P> BusyReporter<'t, 'a, F, P>
where
    F: ThreadModel,
    P: GroupParts<F>,
{
    pub unsafe fn new_unchecked(guard: ReporterGuard<'t, 'a, F, P>) -> Self {
        Self(guard)
    }

    pub fn as_group(&self) -> &BusyGroup<'t, 'a, F, P> {
        todo!()
    }

    pub fn slots(&self) -> &SlotVec<'a, F, P> {
        todo!()
    }
    pub fn slots_mut(&mut self) -> &mut SlotVec<'a, F, P> {
        todo!()
    }

    pub fn busy_data(&self) -> &P::Data<'a> {
        todo!()
    }
    pub fn busy_data_mut(&mut self) -> &mut P::Data<'a> {
        todo!()
    }

    pub fn my_slot_data(&self) -> &P::SlotData<'a> {
        todo!()
    }

    pub fn my_slot_data_mut(&mut self) -> &mut P::SlotData<'a> {
        todo!()
    }

    pub fn index(&self) -> &usize {
        //安全性：已解锁
        unsafe { &*self.0.0.slot_share.index.get() }
    }

    pub fn index_mut(&mut self) -> &mut usize {
        //安全性： 已解锁
        unsafe { &mut *self.0.0.slot_share.index.get() }
    }

    ///子任务中止整个下载组
    pub fn into_idle(self, idle: P::Result<'a>) -> IdleReporter<'t, 'a, F, P> {
        let guard = self.0;
        let locked = unsafe { &mut *guard.0.group.locked.get() };
        *locked.state = State::Idle(todo!());
        IdleReporter(guard)
    }
}

impl<'t, 'a, F, P> IdleReporter<'t, 'a, F, P>
where
    F: ThreadModel,
    P: GroupParts<F>,
{
    pub unsafe fn new_unchecked(guard: ReporterGuard<'t, 'a, F, P>) -> Self {
        Self(guard)
    }

    pub fn idle_data(&self) -> &P::Result<'a> {
        todo!()
    }
    pub fn idle_data_mut(&mut self) -> &mut P::Result<'a> {
        todo!()
    }
}

//#[derive(Clone, Debug, Default)]
pub struct SlotVec<'a, F, E>(pub Vec<Slot<'a, F, E>>)
where
    F: ThreadModel,
    E: GroupParts<F>;

impl<'a, F, E> SlotVec<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn swap_remove_and_update_index(&mut self, index: usize) -> Slot<'a, F, E> {
        let removed = self.0.swap_remove(index);

        if index != self.0.len() {
            self.update_index(index);
        }

        removed
    }

    pub fn push_slot(&mut self, slot: Slot<'a, F, E>) {
        self.0.push(slot);
    }

    pub fn update_index(&mut self, index: usize) {
        *self[index].index_mut() = index;
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn into_raw(self) -> Vec<Slot<'a, F, E>> {
        self.0
    }

    // pub fn into_segment_iter(self) -> impl Iterator<Item = Segment> {
    //     self.0.into_iter().map(|s| {
    //         let end = s.
    //     })
    // }

    //pub fn from_segments()
}

// impl<'a, F: ThreadModel, E: GroupParts<F>> FromIterator<Segment> for SlotVec<'a, F, E> {
//     fn from_iter<T: IntoIterator<Item = Segment>>(iter: T) -> Self {
//         iter.into_iter().map(|segment| {unsafe {Slot::}})
//     }
// }

///Index(Mut) for SlotVec
impl<'a, F, E> Index<usize> for SlotVec<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    type Output = Slot<'a, F, E>;
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}
impl<'a, F, E> IndexMut<usize> for SlotVec<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}
// impl<'a, F, E> AsRef<Vec<Slot<'a, F, E>>> for SlotVec<'a, F, E>
// where
//     F: ThreadModel,
//     E: GroupExt<F>,
// {
//     fn as_ref(&self) -> &Vec<Slot<'a, F, E>> {
//         &self.0
//     }
// }

// ///AsRef(Mut) for SlotVec
// impl<'a, F, E> AsMut<Vec<Slot<'a, F, E>>> for SlotVec<'a, F, E>
// where
//     F: ThreadModel,
//     E: GroupExt<F>,
// {
//     fn as_mut(&mut self) -> &mut Vec<Slot<'a, F, E>> {
//         &mut self.0
//     }
// }

///可以访问my_index, SlotShareExt, SlotInLockShareExt
impl<'a, F, E> Slot<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    // unsafe fn new(slot_share: SlotShare<'a, F, E>, slot_data: E::SlotData<'a>) -> Self{
    //     let share
    // }
    //安全性：承诺Slot逻辑上拥有SlotShare内index字段的所有权
    unsafe fn with_raw(share: RefCounter<F, SlotShare<'a, F, E>>, ext: E::SlotData<'a>) -> Self {
        Self { share, data: ext }
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
    pub fn slot_ext(&self) -> &E::SlotShare<'a> {
        &self.share.ext
    }

    ///&mut SlotInLockShareExt
    pub fn slot_inlock_ext(&self) -> &E::SlotData<'a> {
        &self.data
    }
    pub fn slot_inlock_ext_mut(&mut self) -> &mut E::SlotData<'a> {
        &mut self.data
    }
}

//-----------------------------------------inner struct-----------------------------------

///专为下载任务特化的任务管理器，运行时无关
pub(crate) struct GroupShared<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    share: E::StaticData<'a>,

    mutex: F::Mutex,
    locked: SyncUnsafeCell<InLockShared<'a, F, E>>,
}

impl<'a, F: ThreadModel, E: GroupParts<F>> GroupShared<'a, F, E> {
    fn with_raw(share: E::StaticData<'a>, inlock: InLockShared<'a, F, E>) -> Self {
        Self {
            share,

            mutex: F::Mutex::new(),
            locked: SyncUnsafeCell::new(inlock),
        }
    }
}
type RefGroupShared<'a, F: ThreadModel, E: GroupParts<F>> = F::RefCounter<GroupShared<'a, F, E>>;

// type GroupGuardState<'a, F: ThreadModel, E: GroupParts<F>> =
//     State<BusyGroup<'a, F, E>, IdleGroup<'a, F, E>>;
// type ReporterGuardState<'a, F: ThreadModel, E: GroupParts<F>> =
//     State<ReporterBusy<'a, F, E>, ReporterIdle<'a, F, E>>;

struct InLockShared<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    data: E::Data<'a>,
    state: State<BusySlot<'a, F, E>, IdleSlot<'a, F, E>>,
}

pub enum State<B, I> {
    Running(B),
    Idle(I),
}

impl<B, I> State<B, I> {
    fn is_busy(&self) -> bool {
        matches!(self, Self::Running(_))
    }

    fn is_idle(&self) -> bool {
        matches!(self, Self::Idle(_))
    }

    ///安全性：
    /// self 为busy变体，且f不会产生panic
    pub unsafe fn busy_to_idle_unchecked(&mut self, f: impl FnOnce(B) -> I) {
        unsafe {
            let this = self as *mut Self;
            match self {
                Self::Running(busy) => {
                    ptr::write(this, State::Idle(f(ptr::read::<B>(busy as *const B))))
                }
                _ => unreachable_unchecked(),
            }
        }
    }

    ///安全性：
    /// self为idle变体，且f不会产生panic
    pub unsafe fn idle_to_busy_unchecked(&mut self, f: impl FnOnce(I) -> B) {
        unsafe {
            let this = self as *mut Self;
            match self {
                Self::Idle(idle) => {
                    ptr::write(this, State::Running(f(ptr::read::<I>(idle as *const I))))
                }
                _ => unreachable_unchecked(),
            };
        }
    }

    pub fn idle(self) -> Option<I> {
        match self {
            Self::Idle(idle) => Some(idle),
            _ => None,
        }
    }

    pub fn busy(self) -> Option<B> {
        match self {
            Self::Running(busy) => Some(busy),
            _ => None,
        }
    }

    pub fn map_busy<R>(self, f: impl FnOnce(B) -> R) -> State<R, I> {
        match self {
            Self::Running(r) => State::Running(f(r)),
            Self::Idle(t) => Self::Idle(t),
        }
    }

    pub fn map_idle<R>(self, f: impl FnOnce(B) -> R) -> State<B, R> {
        match self {
            Self::Idle(i) => State::Idle(f(i)),
            State::Running(r) => State::Running(r),
        }
    }

    pub fn running_and_then<R>(self, f: impl FnOnce(B) -> State<R, I>) -> State<R, I> {
        match self {
            State::Running(r) => f(r),
            State::Idle(i) => State::Idle(i),
        }
    }

    pub fn idle_and_then<R, E>(self, f: impl FnOnce(I) -> State<R, E>) -> State<R, E> {
        match self {
            State::Idle(i) => f(i),
            State::Running(r) => State::Running(r),
        }
    }
}

//TODO: into priv
struct BusySlot<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    pub slots: SlotVec<'a, F, E>,
    pub data: E::Data<'a>,
}

impl<'a, F, E> BusySlot<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    pub fn empty(data: E::Data<'a>, waker: E::Data<'a>) -> Self {
        Self {
            slots: SlotVec(Vec::new()),
            data,
        }
    }

    pub unsafe fn with_raw(
        slots: SlotVec<'a, F, E>,
        data: E::Data<'a>,
        waker: E::Data<'a>,
    ) -> Self {
        Self { slots, data, waker }
    }

    pub fn slots(&self) -> &SlotVec<'a, F, E> {
        &self.slots
    }
    pub unsafe fn slots_mut(&mut self) -> &mut SlotVec<'a, F, E> {
        &mut self.slots
    }

    pub fn data(&self) -> &E::Data<'a> {
        &self.data
    }
    pub fn data_mut(&mut self) -> &mut E::Data<'a> {
        &mut self.data
    }

    pub fn into_raw(self) -> (SlotVec<'a, F, E>, E::Data<'a>, E::Data<'a>) {
        (self.slots, self.data, self.waker)
    }
}

pub struct IdleSlot<'a, F, P>
where
    F: ThreadModel,
    P: GroupParts<F>,
{
    pub data: P::Result<'a>,
    empty_vec: Vec<Slot<'a, F, P>>,
}

/// 内部存储项
/// &mut of Self will cause UB
pub(crate) struct Slot<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    //leak &mut of this field will cause UB
    pub share: RefCounter<F, SlotShare<'a, F, E>>,

    pub data: E::SlotData<'a>,
}

pub struct SlotShare<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    pub index: SyncUnsafeCell<usize>,
    //leak &mut of this field is inposeable
    pub ext: E::SlotShare<'a>,
}
type RefSlotShare<'a, F: ThreadModel, E: GroupParts<F>> = F::RefCounter<SlotShare<'a, F, E>>;

impl<'a, F, E> SlotShare<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    fn new_pair(
        index: usize,
        ext: E::SlotShare<'a>,
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

//--------------------------LockedGuards---------------------------

pub trait GroupParts<F: ThreadModel> {
    ///所有线程共享的只读数据
    type StaticData<'a>; //GroupShare

    //GroupInLock
    // 访问这四项需要解锁
    type Result<'a>;
    type Data<'a>;
    type SlotData<'a>; //每个线程一份

    ///每个线程一份的只读数据
    type SlotShare<'a>;
}

///标准库SyncUnsafeCell还未稳定
pub(crate) struct SyncUnsafeCell<T>(UnsafeCell<T>);

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

///安全性：保证share指向slot
unsafe fn new_reporter<'a, F, E>(
    group: *const F::RefCounter<GroupShared<'a, F, E>>,
    slots: *mut SlotVec<'a, F, E>,
    slot_ext: E::SlotShare<'a>,
    slot_inlock: E::SlotData<'a>,
) -> Reporter<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    unsafe {
        let slots = &mut *slots;
        let group = &*group;

        let (share1, share2) = SlotShare::<F, E>::new_pair(slots.len(), slot_ext);
        let slot = Slot::with_raw(share1, slot_inlock);
        slots.push_slot(slot);
        Reporter::from_raw((*group).clone(), share2)
    }
}

///唤醒机制
trait Wake {
    fn wake(self);
}

impl<T> Wake for T
where
    T: FnOnce(),
{
    fn wake(self) {
        self();
    }
}

impl Wake for Waker {
    fn wake(self) {
        self.wake();
    }
}

///限制使用lock方法的api包装器
#[derive(Debug, Clone, Copy)]
struct ReporterRef<'t, 'a, F, P>(&'t Reporter<'a, F, P>)
where
    F: ThreadModel,
    P: GroupParts<F>;

impl<'t, 'a, F, P> ReporterRef<'t, 'a, F, P>
where
    F: ThreadModel,
    P: GroupParts<F>,
{
    fn slot(self) -> &'t <P as GroupParts<F>>::SlotShare<'a> {
        self.0.slot()
    }

    fn group(self) -> &'t <P as GroupParts<F>>::StaticData<'a> {
        self.0.group()
    }
}

///限制使用lock方法的api包装器
#[derive(Debug, Clone, Copy)]
struct GroupRef<'t, 'a, F, P>(&'t DownloadGroup<'a, F, P>)
where
    F: ThreadModel,
    P: GroupParts<F>;

impl<'t, 'a, F, P> GroupRef<'t, 'a, F, P>
where
    F: ThreadModel,
    P: GroupParts<F>,
{
    fn group(self) -> &'t <P as GroupParts<F>>::StaticData<'a> {
        self.0.share()
    }
}

// struct SlotRef<'t, 'a, F, P>(&'t Slot<'a, F, P>)
// where
//     F: ThreadModel,
//     P: GroupParts<F>;

// impl  for  {

// }