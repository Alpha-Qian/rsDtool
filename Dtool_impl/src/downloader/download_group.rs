//!定义分块下载的并发结构体
//!

use std::cell::UnsafeCell;
use std::hint::unreachable_unchecked;
use std::mem::{self, ManuallyDrop};
use std::ops::Deref;
use std::{
    ops::{Index, IndexMut, RangeFrom},
    ptr,
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
    pub fn new(share_ext: E::GroupShare<'data>, inlock_ext: E::InLockExt<'data>) -> Self {
        Self(F::RefCounter::new(GroupShared::with_raw(share_ext, inlock_ext)))
    }

    pub(crate) fn from_raw(inner: F::RefCounter<GroupShared<'data, F, E>>) -> Self {
        Self(inner)
    }

    pub fn lock<'a>(self) -> GroupGuard<'data, F, E> {
        GroupGuard::new(self)
    }

    pub fn share_ext(&self) -> &E::GroupShare<'data> {
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
    group: F::RefCounter<GroupShared<'data, F, E>>,
    slot_share: F::RefCounter<SlotShare<'data, F, E>>,
}

//可以访问：&GroupShareExt, &SlotShareExt, Lock
impl<'a, F, E> Reporter<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    unsafe fn from_raw(
        group: F::RefCounter<GroupShared<'a, F, E>>,
        slot_share: RefSlotShare<'a, F, E>,
    ) -> Self {
        Self { group, slot_share }
    }

    ///Aquare Lock
    pub fn lock(self) -> ReporterGuard<'a, F, E> {
        ReporterGuard::new(self)
    }

    ///GroupExt
    pub fn group(&self) -> &E::GroupShare<'a> {
        &self.group.ext
    }

    ///slot ext
    pub fn slot_ext(&self) -> &E::SlotShare<'a> {
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
impl<'a, F, E> GroupGuard<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    pub fn new(group: DownloadGroup<'a, F, E>) -> Self {
        group.0.mutex.acquire();
        Self { group: group.0 }
    }

    pub fn release_lock(self) -> DownloadGroup<'a, F, E> {
        let group = unsafe { ptr::read(&self.group) };
        ManuallyDrop::new(self);
        group.mutex.release();
        DownloadGroup(group)
    }

    ///GroupExt
    pub fn group_ext(&self) -> &E::GroupShare<'a> {
        &self.group.ext
    }

    //busy or idle data
    pub fn data(&self) -> State<&E::BusyData<'a>, &E::IdleData<'a>> {
        match self.locked() {
            State::Busy(busy) => State::Busy(&busy.data),
            State::Idle(idle) => State::Idle(&idle.data)
        }
    }
    pub fn data_mut(&mut self) -> State<&mut E::BusyData<'a>, &mut E::IdleData<'a>> {
        match self.locked_mut() {
            State::Busy(busy) => State::Busy(&mut busy.data),
            State::Idle(idle) => State::Idle(&mut idle.data)
        }
    }

    //priv locked
    fn locked(&self) -> &InLockShared<'a, F, E> {
        unsafe  { & *self.group.locked.get()}
    }
    fn locked_mut(&mut self) -> &mut InLockShared<'a, F, E> {
        unsafe { &mut *self.group.locked.get()}
    }

    pub fn get_state(self) -> GroupGuardState<'a, F, E> {
        match self.locked() {
            State::Busy(_) => State::Busy(GuardBusy(self)),
            State::Idle(_) => State::Idle(GuardIdle(self))
        }
    }


}

impl<'data, F: ThreadModel, E: GroupExt<F>> Drop for GroupGuard<'data, F, E> {
    fn drop(&mut self) {
        self.group.mutex.release();
    }
}
///reporter WriteGuard
pub struct ReporterGuard<'data, F, E>
where 
    F: ThreadModel, 
    E: GroupExt<F> 
{
    slot: F::RefCounter<SlotShare<'data, F, E>>,
    group: F::RefCounter<GroupShared<'data, F, E>>,
}

/// 可以访问 &GroupExt, &MySlotExt, &mut InLockExt, &mut MySlotInLockExt, &mut SlotVector(unsafe)
impl<'a, F, E> ReporterGuard<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn new(reporter: Reporter<'a, F, E>) -> Self {
        reporter.group.mutex.acquire();
        unsafe { Self::from_raw(reporter.group, reporter.slot_share) }
    }
    ///安全性：确保group和slot是成对的
    /// 确保已解锁
    unsafe fn from_raw(
        group: F::RefCounter<GroupShared<'a, F, E>>,
        slot: F::RefCounter<SlotShare<'a, F, E>>,
    ) -> Self {
        Self {
            group,
            slot,
        }
    }

    pub fn release_lock(self) -> Reporter<'a, F, E> {
        unsafe {
            let slot_share = ptr::read(&self.slot);
            let group_share = ptr::read(&self.group);
            ManuallyDrop::new(self);
            group_share.mutex.release();
            Reporter::from_raw(group_share, slot_share)
        }
    }

    pub fn get_state(self) -> ReporterGuardState<'a, F, E> {
        match self.locked() {
            State::Busy(_) => State::Busy(ReporterGuardBusy(self)),
            State::Idle(_) => State::Idle(ReporterGuardIdle(self))
        }
    }
    
    pub unsafe fn swap_slot(&mut self, reporter: &mut Reporter<'a, F, E>) {
        debug_assert_eq!(
            self.group.deref() as *const _,
            reporter.group.deref() as *const _
        );

        mem::swap(&mut self.slot, &mut reporter.slot_share);
    }

    //busy or idle data
    pub fn data(&self) -> State<&E::BusyData<'a>, &E::IdleData<'a>> {
        match self.locked() {
            State::Busy(busy) => State::Busy(&busy.data),
            State::Idle(idle) => State::Idle(&idle.data)
        }
    }
    pub fn data_mut(&mut self) -> State<&mut E::BusyData<'a>, &mut E::IdleData<'a>> {
        match self.locked_mut() {
            State::Busy(busy) => State::Busy(&mut busy.data),
            State::Idle(idle) => State::Idle(&mut idle.data)
        }
    }


    ///GroupExt
    pub fn group(&self) -> &E::GroupShare<'a> {
        &self.group.ext
    }

    ///MySlotExt
    pub fn my_slot_ext(&self) -> &E::SlotShare<'a> {
        &self.slot.ext
    }

    //priv locked
    fn locked(&self) -> &InLockShared<'a, F, E> {
        unsafe{ & *self.group.locked.get() }
    }
    fn locked_mut(&mut self) -> &mut InLockShared<'a, F, E> {
        unsafe{ &mut *self.group.locked.get() }
    }

    ///MyIndex
    pub fn my_index(&self) -> &usize {
        unsafe {
            & *self.slot.index.get()
        }
    }
    pub fn my_index_mut(&mut self) -> &mut usize{
        unsafe {
            &mut *self.slot.index.get()
        }
    }
    
    
}

impl<'data, F, E> Drop for ReporterGuard<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn drop(&mut self) {
        self.group.mutex.release();
    }
}



// -----------Busy and Idle API -------------------


struct GuardBusy<'a, F, E>(
    GroupGuard<'a, F, E>
)
where 
    F: ThreadModel,
    E: GroupExt<F>;

impl<'a, F, E> GuardBusy<'a, F, E>
where 
    F: ThreadModel,
    E: GroupExt<F>,
{

    //priv
    fn busy(&self) -> &Busy<'a, F, E> {
        unsafe{
            let inlock = & *self.0.group.locked.get();
            match inlock {
                InLockShared::Busy(data) => return data,
                _ => unreachable!()
            }
        }
    }
    pub fn busy_mut(&mut self) -> &mut Busy<'a, F, E> {
        unsafe{
            let inlock = &mut *self.0.group.locked.get();
            match inlock {
                InLockShared::Busy(data) => return data,
                _ => unreachable!()
            }
        }
    }

    pub fn new_reporter(&mut self, slot_ext: E::SlotShare<'a>, slot_inlock: E::SlotInlock<'a>) -> Reporter<'a, F, E>{
        unsafe{
            new_reporter(&self.0.group, &mut self.busy_mut().slots, slot_ext, slot_inlock)
        }
    }

    pub fn swap_state(mut self, idle: Idle<'a, F, E>) -> (GuardIdle<'a, F, E>, Busy<'a, F, E>) {
        let mut output = State::Idle(idle);
        let inlock = self.0.locked_mut();
        mem::swap(inlock, &mut output);
        match output {
            State::Busy(busy) => {
                return (GuardIdle(self.0), busy)
            }
            _ => unsafe{ unreachable_unchecked() }
        }
    }

    pub fn to_idle(mut self, f: impl FnOnce(Busy<'a, F, E>) -> Idle<'a, F, E>) -> GuardIdle<'a, F, E> {
        unsafe{ self.0.locked_mut().busy_to_idle_unchecked(f) };
        GuardIdle(self.0)
    }

    //Slots
    pub unsafe fn slots(&self) -> &SlotVec<'a, F, E> {
        &self.busy().slots
    }
    pub fn slots_mut(&mut self) -> &mut SlotVec<'a, F, E> {
        &mut self.busy_mut().slots
    }

    //Busy Data
    pub fn busy_data(&self) -> &E::BusyData<'a> {
        &self.busy().data
    }
    pub fn busy_data_mut(&mut self) -> &mut E::BusyData<'a> {
        &mut self.busy_mut().data
    }

    pub fn erase_state(self) -> GroupGuard<'a, F, E> {
        self.0
    }
}

struct GuardIdle<'a, F, E>(
    GroupGuard<'a, F, E>
)
where 
    F: ThreadModel,
    E: GroupExt<F>;

impl<'a, F, E> GuardIdle<'a, F, E>
where 
    F: ThreadModel,
    E: GroupExt<F>,
{


    fn idle(&self) -> &Idle<'a, F, E> {
        unsafe{
            let inlock = & *self.0.group.locked.get();
            match inlock {
                InLockShared::Idle(data) => return data,
                _ => unreachable!()
            }
        }
    }
    fn idle_mut(&self) -> &mut Idle<'a, F, E>{
        unsafe{
            let inlock = &mut *self.0.group.locked.get();
            match inlock {
                InLockShared::Idle(data) => return data,
                _ => unreachable!() 
            }
        }
    }

    ///busy中必须是有效数据
    pub unsafe fn swap_state(mut self, busy: Busy<'a, F, E>) -> (GuardBusy<'a, F, E>, Idle<'a, F, E>) {
        let mut output = State::Busy(busy);
        mem::swap(self.0.locked_mut(), &mut output);
        match output {
            State::Idle(idle) => return (GuardBusy(self.0), idle),
            _ => unreachable!()
        }

    }

    ///安全性：f不发生panic
    pub unsafe fn to_busy(mut self, f: impl FnOnce(Idle<'a, F, E>) -> Busy<'a, F, E>) -> GuardBusy<'a, F, E> {
        unsafe{ self.0.locked_mut().idle_to_busy_unchecked(f) };
        GuardBusy(self.0)
    }

    //Idle Data
    pub fn idle_data(&self) -> & E::IdleData<'a> {
        &self.idle().data
    }
    pub fn idle_data_mut(&mut self) -> &mut E::IdleData<'a> {
        &mut self.idle_mut().data
    }

    pub fn into_raw(self) -> GroupGuard<'a, F, E> {
        self.0
    }
}

struct ReporterGuardBusy<'a, F, E>(
    ReporterGuard<'a, F, E>
)
where 
    F: ThreadModel,
    E: GroupExt<F>;


impl<'a, F, E> ReporterGuardBusy<'a, F, E>
where 
    F: ThreadModel,
    E: GroupExt<F>
{

    //Busy
    pub fn busy(&self) -> &Busy<'a, F, E> {
        unsafe {
            let inlock = & *self.0.group.locked.get();
            match inlock {
                InLockShared::Busy(busy) => return busy,
                _ => unreachable!(),
            }
        }
    }
    pub fn busy_mut(&mut self) -> &mut Busy<'a, F, E> {
        unsafe {
            let inlock = &mut *self.0.group.locked.get();
            match inlock {
                InLockShared::Busy(busy) => return busy,
                _ => unreachable!(),
            }
        }
    }

    pub fn new_reporter(&mut self, slot_ext: E::SlotShare<'a>, slot_inlock: E::SlotInlock<'a>) -> Reporter<'a, F, E> {
        unsafe{
            new_reporter(&self.0.group, &mut self.busy_mut().slots, slot_ext, slot_inlock)
        }
    }
    
    pub fn swap_state(mut self, idle: Idle<'a, F, E>) -> (ReporterGuardIdle<'a, F, E>, Busy<'a, F, E>) {

        let mut output = State::Idle(idle);
        mem::swap(self.0.locked_mut(), &mut output);
        match output {
            State::Busy(busy) => return (ReporterGuardIdle(self.0), busy),
            _ => unreachable!()
        }
    }

    ///安全性：f不发生panic
    pub unsafe fn to_idle(mut self, f: impl FnOnce(Busy<'a, F, E>) -> Idle<'a, F, E>) -> ReporterGuardIdle<'a, F, E> {
        unsafe{ self.0.locked_mut().busy_to_idle_unchecked(f) };
        ReporterGuardIdle(self.0)
    }

    //Slots
    pub unsafe fn slots(&self) -> &SlotVec<'a, F, E> {
        &self.busy().slots
    }
    pub fn slots_mut(&mut self) -> &mut SlotVec<'a, F, E> {
        &mut self.busy_mut().slots
    }

    //Data
    pub fn busy_data(&self) -> &E::BusyData<'a> {
        &self.busy().data
    }
    pub fn busy_data_mut(&mut self) -> &mut E::BusyData<'a> {
        &mut self.busy_mut().data
    }

    pub fn into_raw(self) -> ReporterGuard<'a, F, E> {
        self.0
    }
}

struct ReporterGuardIdle<'a, F, E>(
    ReporterGuard<'a, F, E>
)
where 
    F: ThreadModel,
    E: GroupExt<F>;

impl<'a, F, E> ReporterGuardIdle<'a, F, E>
where 
    F: ThreadModel,
    E: GroupExt<F>,
{

    //Idle
    pub fn idle(&self) -> &Idle<'a, F, E> {
        unsafe{
            let inlock = & *self.0.group.locked.get();
            match inlock {
                InLockShared::Idle(data) => return data,
                _ => unreachable!()
            }
        }
    }
    pub fn idle_mut(&mut self) -> &mut Idle<'a, F, E> {
        unsafe{
            let inlock = &mut  *self.0.group.locked.get();
            match inlock {
                InLockShared::Idle(data) => return data,
                _ => unreachable!()
            }
        }
    }

    ///安全性：busy中是有效数据
    pub unsafe fn swap_state(mut self, busy: Busy<'a, F, E>) -> (ReporterGuardBusy<'a, F, E>, Idle<'a, F, E>) {
        let mut output = State::Busy(busy);
        mem::swap(self.0.locked_mut(), &mut output);
        match output {
            State::Idle(idle) => return (ReporterGuardBusy(self.0), idle),
            _ => unreachable!()
        }
    }

    ///安全性：确保f不会Panic
    pub unsafe fn to_busy(mut self, f: impl FnOnce(Idle<'a, F, E>) -> Busy<'a, F, E>) -> ReporterGuardBusy<'a, F, E> {
        unsafe{ self.0.locked_mut().idle_to_busy_unchecked(f) };
        ReporterGuardBusy(self.0)
    }

    // pub unsafe fn update_state(mut self, f: impl FnOnce(Idle<'a, F, E>) -> Busy<'a, F, E>) -> ReporterGuardBusy<'a, F, E>{
    //     //ptr::write(self.idle_mut() as *mut _, f());
    // }

    //Data
    pub fn idle_data(&self) -> & E::IdleData<'a> {
        &self.idle().data
    }
    pub fn idle_data_mut(&mut self) -> &mut E::IdleData<'a> {
        &mut self.idle_mut().data
    }

    pub fn into_raw(self) -> ReporterGuard<'a, F, E> {//earse state
        self.0
    }
}


//#[derive(Clone, Debug, Default)]
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

//Index(Mut) for SlotVec
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

///AsRef(Mut) for SlotVec
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
        ext: E::SlotInlock<'a>,
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
    pub fn slot_ext(&self) -> &E::SlotShare<'a> {
        &self.share.ext
    }

    ///&mut SlotInLockShareExt
    pub fn slot_inlock_ext(&self) -> &E::SlotInlock<'a> {
        &self.ext
    }
    pub fn slot_inlock_ext_mut(&mut self) -> &mut E::SlotInlock<'a> {
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
    pub ext: E::GroupShare<'a>,
}

impl<'a, F: ThreadModel, E: GroupExt<F>> GroupShared<'a, F, E> {
    fn with_raw(share_ext: E::GroupShare<'a>, inlock_shared: InLockShared<'a, F, E>) -> Self {
        Self {
            mutex: F::Mutex::new(),
            locked: SyncUnsafeCell::new(inlock_shared),
            ext: share_ext,
        }
    }
}
type RefGroupShared<'data, F: ThreadModel, E: GroupExt<F>> =
    F::RefCounter<GroupShared<'data, F, E>>;

type InLockShared<'a, F: ThreadModel, E: GroupExt<F>> = State<Busy<'a, F, E>, Idle<'a, F, E>>;
type GroupGuardState<'a, F:ThreadModel, E: GroupExt<F>> = State<GuardBusy<'a, F, E>, GuardIdle<'a, F, E>>;
type ReporterGuardState<'a, F: ThreadModel, E:GroupExt<F>> = State<ReporterGuardBusy<'a, F, E>, ReporterGuardIdle<'a, F, E>>;

// struct InLockShared<'a, F:ThreadModel, E:GroupExt<F>> {
//     data: E::Data<'a>,
//     state: State<Busy<'a, F, E>, Idle<'a, F, E>>
// }
pub enum State<T, U>{
    Busy(T),
    Idle(U)
}

impl<T, U> State<T, U> {
    fn is_busy(&self) -> bool {
        matches!(self, Self::Busy(_))
    }

    fn is_idle(&self) -> bool {
        matches!(self, Self::Idle(_))
    }

    ///安全性：
    /// self 为busy变体，且f不会产生panic
    pub unsafe fn busy_to_idle_unchecked(&mut self, f: impl FnOnce(T) -> U) {
        unsafe {
            let this = self as *mut Self;
            match self{
                Self::Busy(busy) => ptr::write(this, State::Idle(f(ptr::read::<T>(busy as *const T)))),
                _ => unreachable_unchecked()
            };
        }
    }

    ///安全性：
    /// self为idle变体，且f不会产生panic
    pub unsafe fn idle_to_busy_unchecked(&mut self, f: impl FnOnce(U) -> T) {
        unsafe {
            let this = self as *mut Self;
            match self{
                Self::Idle(idle) => ptr::write(this, State::Busy(f(ptr::read::<U>(idle as *const U)))),
                _ => unreachable_unchecked()
            };
        }
    }
}


struct Busy<'a, F, E> 
where
    F: ThreadModel,
    E: GroupExt<F>
{
    slots: SlotVec<'a, F, E>,
    data: E::BusyData<'a>
}

impl<'a, F, E> Busy<'a, F, E> 
where
    F: ThreadModel,
    E: GroupExt<F>
{

    pub fn empty(data: E::BusyData<'a>) -> Self {
        Self { slots: SlotVec(Vec::new()), data }
    }

    pub unsafe fn with_raw(slots: SlotVec<'a, F, E>, data: E::BusyData<'a>) -> Self {
        Self { slots, data }
    }

    pub fn slots(&self) -> &SlotVec<'a, F, E> {
        &self.slots
    }
    pub unsafe fn slots_mut(&mut self) -> &mut SlotVec<'a, F, E> {
        &mut self.slots
    }
    
    pub fn data(&self) -> &E::BusyData<'a> {
        &self.data
    }
    pub fn data_mut(&mut self) -> &mut E::BusyData<'a> {
        &mut self.data
    }
}

struct Idle<'a, F, E>
where 
    F: ThreadModel,
    E: GroupExt<F>,
{
    data: E::IdleData<'a>
}

impl<'a, F, E> Idle<'a, F, E>
where 
    F: ThreadModel,
    E: GroupExt<F>
{
    pub fn new(data: E::IdleData<'a> ) -> Self {
        Self{data}
    }

    pub fn data(&self) -> &E::IdleData<'a> {
        &self.data
    }
    pub fn data_mut(&mut self) -> &mut E::IdleData<'a> {
        &mut self.data
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

    pub ext: E::SlotInlock<'a>,
}

struct SlotShare<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    index: SyncUnsafeCell<usize>,
    //leak &mut of this field is inposeable
    pub ext: E::SlotShare<'data>,
}
type RefSlotShare<'data, F: ThreadModel, E: GroupExt<F>> = F::RefCounter<SlotShare<'data, F, E>>;

impl<'data, F, E> SlotShare<'data, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn new_pair(
        index: usize,
        ext: E::SlotShare<'data>,
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

    type GroupShare<'a>;

    //GroupInLock
    type Data<'a>;
    type IdleData<'a>;
    type BusyData<'a>;
    
    type SlotShare<'a>;
    type SlotInlock<'a>;
}

///还不知道具体怎么用
trait ProcessRecordKind {
    type State;
    type Downloaded<T>: Radium<Item = T>;
    type Writed<T>: Radium<Item = T>;

    fn report_downloaded_len(len: u64);
}


struct ExtHander<'a, E: GroupExt<F>, F: ThreadModel> {
    group_share: &'a E::GroupShare<'a>,
    slot_share: &'a E::SlotShare<'a>,
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



///安全性：保证share指向slot
unsafe fn new_reporter<'a, F, E>(
    group: *const F::RefCounter<GroupShared<'a, F, E>>,
    slots: *mut SlotVec<'a, F, E>,
    slot_ext: E::SlotShare<'a>,
    slot_inlock: E::SlotInlock<'a>,
) -> Reporter<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    unsafe{
        let slots = &mut *slots;
        let group = & *group;

        let (share1, share2) = SlotShare::<F, E>::new_pair(slots.len(), slot_ext);
        let slot = Slot::with_raw(share1, slot_inlock) ;
        slots.push_slot(slot);
        Reporter::from_raw((*group).clone(), share2)
    }
}