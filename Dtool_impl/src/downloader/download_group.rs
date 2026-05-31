//!定义分块下载的并发结构体
//!

use std::cell::UnsafeCell;
use std::hint::unreachable_unchecked;
use std::mem::{self, ManuallyDrop};
use std::ops::Deref;
use std::task::Waker;
use std::{
    ops::{Index, IndexMut},
    ptr,
};

use crate::downloader::segment::Segment;

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

///可以访问：&GroupShareExt, Lock
///获取锁后返回GroupGuard
impl<'a, F, E> DownloadGroup<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    pub fn new_idle(
        group: E::GroupShare<'a>,
        data: E::Data<'a>,
        idle_data: E::IdleData<'a>,
    ) -> Self {
        let in_lock_shared = InLockShared {
            data,
            state: State::Idle(IdleSlot { data: idle_data }),
        };

        Self(F::RefCounter::new(GroupShared::with_raw(
            group,
            in_lock_shared,
        )))
    }

    pub(crate) fn new_busy(
        group: E::GroupShare<'a>,
        data: E::Data<'a>,
        busy_data: E::BusyData<'a>,
        waker: E::Waker<'a>,
    ) -> Self {
        let in_lock_shared = InLockShared {
            data,
            state: State::Busy(BusySlot {
                slots: SlotVec::new(),
                data: busy_data,
                waker
            }),
        };

        Self(F::RefCounter::new(GroupShared::with_raw(
            group,
            in_lock_shared,
        )))
    }

    pub fn lock(self) -> GroupGuard<'a, F, E> {
        GroupGuard::new(self)
    }

    pub fn share(&self) -> &E::GroupShare<'a> {
        &self.0.share
    }
}

/// 每个下载分块向下载组报告状态的结构体
/// 这个结构体是生产者也是消费者
pub struct Reporter<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    group: F::RefCounter<GroupShared<'a, F, E>>,
    slot_share: F::RefCounter<SlotShare<'a, F, E>>,
}

///可以访问：&GroupShareExt, &SlotShareExt, Lock
/// 解锁后可以获取ResporterGuard
impl<'a, F, E> Reporter<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
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
        &self.group.share
    }

    ///slot ext
    pub fn slot_ext(&self) -> &E::SlotShare<'a> {
        &self.slot_share.ext
    }
}

///groupWriteGuard
#[repr(transparent)]
pub struct GroupGuard<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    group: F::RefCounter<GroupShared<'a, F, E>>,
}

/// 可以访问&GroupShareExt, &mut InLockExt, unsafe &mut SlotVector
impl<'a, F, E> GroupGuard<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
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
        &self.group.share
    }

    /// data
    pub fn data(&self) -> &E::Data<'a> {
        &self.locked().data
    }
    pub fn data_mut(&mut self) -> &mut E::Data<'a> {
        &mut self.locked_mut().data
    }

    //busy or idle data
    pub fn state_data(&self) -> State<&E::BusyData<'a>, &E::IdleData<'a>> {
        match &self.locked().state {
            State::Busy(busy) => State::Busy(&busy.data),
            State::Idle(idle) => State::Idle(&idle.data),
        }
    }
    pub fn state_data_mut(&mut self) -> State<&mut E::BusyData<'a>, &mut E::IdleData<'a>> {
        match &mut self.locked_mut().state {
            State::Busy(busy) => State::Busy(&mut busy.data),
            State::Idle(idle) => State::Idle(&mut idle.data),
        }
    }

    //unwarp datas
    pub fn unwarp_slots(&self) -> &SlotVec<'a, F, E> {
        match &self.locked().state {
            State::Busy(busy) => &busy.slots,
            State::Idle(_) => panic!("unwarp busy failed"),
        }
    }
    pub unsafe fn unwarp_slots_mut(&mut self) -> &mut SlotVec<'a, F, E> {
        match &mut self.locked_mut().state {
            State::Busy(busy) => &mut busy.slots,
            State::Idle(_) => panic!("unwarp busy failed"),
        }
    }
    pub fn unwarp_busy(&self) -> &E::BusyData<'a> {
        match &self.locked().state {
            State::Busy(busy) => &busy.data,
            State::Idle(_) => panic!("unwarp busy failed"),
        }
    }
    pub fn unwarp_idle(&self) -> &E::IdleData<'a> {
        match &self.locked().state {
            State::Busy(_) => panic!("unwarp idle failed"),
            State::Idle(idle) => &idle.data,
        }
    }

    pub fn unwarp_busy_mut(&mut self) -> &mut E::BusyData<'a> {
        match &mut self.locked_mut().state {
            State::Busy(busy) => &mut busy.data,
            State::Idle(_) => panic!("unwarp busy failed"),
        }
    }
    pub fn unwarp_idle_mut(&mut self) -> &mut E::IdleData<'a> {
        match &mut self.locked_mut().state {
            State::Busy(_) => panic!("unwarp idle failed"),
            State::Idle(idle) => &mut idle.data,
        }
    }

    //priv locked
    fn locked(&self) -> &InLockShared<'a, F, E> {
        unsafe { &*self.group.locked.get() }
    }
    fn locked_mut(&mut self) -> &mut InLockShared<'a, F, E> {
        unsafe { &mut *self.group.locked.get() }
    }

    //state
    pub fn get_state(self) -> GroupGuardState<'a, F, E> {
        match self.locked().state {
            State::Busy(_) => State::Busy(BusyGroup(self)),
            State::Idle(_) => State::Idle(IdleGroup(self)),
        }
    }

    pub fn as_state(&self) -> State<&BusyGroup<'a, F, E>, &IdleGroup<'a, F, E>> {
        unsafe {
            match self.locked().state {
                State::Busy(_) => State::Busy(mem::transmute(self)),
                State::Idle(_) => State::Idle(mem::transmute(self)),
            }
        }
    }
    pub fn as_state_mut(&mut self) -> State<&mut BusyGroup<'a, F, E>, &mut IdleGroup<'a, F, E>> {
        unsafe {
            match self.locked().state {
                State::Busy(_) => State::Busy(mem::transmute(self)),
                State::Idle(_) => State::Idle(mem::transmute(self)),
            }
        }
    }

    pub fn ignore_guard(&self) -> &DownloadGroup<'a, F, E>{
        unsafe{
            mem::transmute(self)
        }
    }
    pub fn ignore_guard_mut(&mut self) -> &mut DownloadGroup<'a, F, E> {
        unsafe{
            mem::transmute(self)
        }
    }
}

// impl<'a, F, E> UpCast<DownloadGroup<'a, F, E>> for GroupGuard<'a, F, E>
// where
//     F: ThreadModel,
//     E: GroupParts<F>,
// {
//     fn upcast(&self) -> &mut DownloadGroup<'a, F, E> {
//         unsafe { mem::transmute(self) }
//     }
//     fn upcast_mut(&mut self) -> &mut DownloadGroup<'a, F, E> {
//         unsafe { mem::transmute(self) }
//     }
// }

impl<'a, F, E> Drop for GroupGuard<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    fn drop(&mut self) {
        self.group.mutex.release();
    }
}
///reporter WriteGuard
pub struct ReporterGuard<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    slot: F::RefCounter<SlotShare<'a, F, E>>,
    group: F::RefCounter<GroupShared<'a, F, E>>,
}

/// 可以访问 &GroupExt, &MySlotExt, &mut InLockExt, &mut MySlotInLockExt, &mut SlotVector(unsafe)
impl<'a, F, E> ReporterGuard<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    fn new(reporter: Reporter<'a, F, E>) -> Self {
        reporter.group.mutex.acquire();
        unsafe { Self::from_raw(reporter.group, reporter.slot_share) }
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

    //state
    pub fn get_state(self) -> ReporterGuardState<'a, F, E> {
        match self.locked().state {
            State::Busy(_) => State::Busy(ReporterBusy(self)),
            State::Idle(_) => State::Idle(ReporterIdle(self)),
        }
    }
    pub fn as_state(&self) -> State<&ReporterBusy<'a, F, E>, &ReporterIdle<'a, F, E>> {
        unsafe {
            match self.locked().state {
                State::Busy(_) => State::Busy(mem::transmute(self)),
                State::Idle(_) => State::Idle(mem::transmute(self)),
            }
        }
    }
    pub fn fetech_state_mut(
        &mut self,
    ) -> State<&mut ReporterBusy<'a, F, E>, &mut ReporterIdle<'a, F, E>> {
        unsafe {
            match self.locked().state {
                State::Busy(_) => State::Busy(mem::transmute(self)),
                State::Idle(_) => State::Idle(mem::transmute(self)),
            }
        }
    }

    //unwarp datas
    pub fn unwarp_slots(&self) -> &SlotVec<'a, F, E> {
        match &self.locked().state {
            State::Busy(busy) => &busy.slots,
            State::Idle(_) => panic!("unwarp busy failed"),
        }
    }
    pub unsafe fn unwarp_slots_mut(&mut self) -> &mut SlotVec<'a, F, E> {
        match &mut self.locked_mut().state {
            State::Busy(busy) => &mut busy.slots,
            State::Idle(_) => panic!("unwarp busy failed"),
        }
    }
    pub fn unwarp_busy(&self) -> &E::BusyData<'a> {
        match &self.locked().state {
            State::Busy(busy) => &busy.data,
            State::Idle(_) => panic!("unwarp busy failed"),
        }
    }
    pub unsafe fn unwarp_busy_mut(&mut self) -> &mut E::BusyData<'a> {
        match &mut self.locked_mut().state {
            State::Busy(busy) => &mut busy.data,
            State::Idle(_) => panic!("unwarp busy failed"),
        }
    }
    pub fn unwarp_idle(&self) -> &E::IdleData<'a> {
        match &self.locked().state {
            State::Busy(_) => panic!("unwarp idle failed"),
            State::Idle(idle) => &idle.data,
        }
    }
    pub unsafe fn unwarp_idle_mut(&mut self) -> &mut E::IdleData<'a> {
        match &mut self.locked_mut().state {
            State::Busy(_) => panic!("unwarp idle failed"),
            State::Idle(idle) => &mut idle.data,
        }
    }

    pub unsafe fn swap_slot(&mut self, reporter: &mut Reporter<'a, F, E>) {
        debug_assert_eq!(
            self.group.deref() as *const _,
            reporter.group.deref() as *const _
        );

        mem::swap(&mut self.slot, &mut reporter.slot_share);
    }

    //data
    pub fn data(&self) -> &E::Data<'a> {
        &self.locked().data
    }
    pub fn data_mut(&mut self) -> &mut E::Data<'a> {
        &mut self.locked_mut().data
    }
    //busy or idle data
    pub fn state_data(&self) -> State<&E::BusyData<'a>, &E::IdleData<'a>> {
        match &self.locked().state {
            State::Busy(busy) => State::Busy(&busy.data),
            State::Idle(idle) => State::Idle(&idle.data),
        }
    }
    pub fn state_data_mut(&mut self) -> State<&mut E::BusyData<'a>, &mut E::IdleData<'a>> {
        match &mut self.locked_mut().state {
            State::Busy(busy) => State::Busy(&mut busy.data),
            State::Idle(idle) => State::Idle(&mut idle.data),
        }
    }

    ///GroupExt
    pub fn group(&self) -> &E::GroupShare<'a> {
        &self.group.share
    }

    ///MySlotExt
    pub fn my_slot_ext(&self) -> &E::SlotShare<'a> {
        &self.slot.ext
    }

    //priv locked
    fn locked(&self) -> &InLockShared<'a, F, E> {
        unsafe { &*self.group.locked.get() }
    }
    fn locked_mut(&mut self) -> &mut InLockShared<'a, F, E> {
        unsafe { &mut *self.group.locked.get() }
    }

    ///MyIndex
    pub fn my_index(&self) -> &usize {
        unsafe { &*self.slot.index.get() }
    }
    pub fn my_index_mut(&mut self) -> &mut usize {
        unsafe { &mut *self.slot.index.get() }
    }

    //transmute
    pub fn as_group(&self) -> &GroupGuard<'a, F, E> {
        unsafe { mem::transmute(&self.group) }
    }
    pub fn as_group_mut(&mut self) -> &mut GroupGuard<'a, F, E> {
        unsafe { mem::transmute(&mut self.group) }
    }

    pub fn ignore_guard(&self) -> &Reporter<'a, F, E>
}

impl<'a, F, E> Drop for ReporterGuard<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    fn drop(&mut self) {
        self.group.mutex.release();
    }
}

impl<'a, F, E> AsRef<Reporter<'a, F, E>> for ReporterGuard<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    fn as_ref(&self) -> &Reporter<'a, F, E> {
        todo!()
    }
}

impl<'a, F, E> AsMut<Reporter<'a, F, E>> for ReporterGuard<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    fn as_mut(&mut self) -> &mut Reporter<'a, F, E> {
        todo!()
    }
}
// -----------Busy and Idle API -------------------

#[repr(transparent)]
pub struct BusyGroup<'a, F, E>(GroupGuard<'a, F, E>)
where
    F: ThreadModel,
    E: GroupParts<F>;

impl<'a, F, E> BusyGroup<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    pub fn new_reporter(
        &mut self,
        slot_share: E::SlotShare<'a>,
        slot_data: E::SlotData<'a>,
    ) -> Reporter<'a, F, E> {
        unsafe {
            new_reporter(
                &self.0.group,
                &mut self.busy_mut().slots,
                slot_share,
                slot_data,
            )
        }
    }

    pub fn swap_state(
        mut self,
        idle: IdleSlot<'a, F, E>,
    ) -> (IdleGroup<'a, F, E>, BusySlot<'a, F, E>) {
        let mut output = State::Idle(idle);
        let inlock = &mut self.0.locked_mut().state;
        mem::swap(inlock, &mut output);
        match output {
            State::Busy(busy) => return (IdleGroup(self.0), busy),
            _ => unsafe { unreachable_unchecked() },
        }
    }

    //pub fn swap_state2(mut self, idle_data: E::IdleData<'a>) ->

    //todo
    // pub unsafe fn into_idle(mut self, f: impl FnOnce(SlotVec<'a, F, E>, E::BusyData<'a>) -> E::IdleData<'a>) -> IdleGroup<'a, F, E>{
    //     unsafe{ self.0.locked_mut().state.busy_to_idle_unchecked(|busy| Idle(f(busy.slots, busy.data)))};
    //     IdleGroup(self.0)
    // }

    //Data
    pub fn data(&self) -> &E::Data<'a> {
        &self.0.locked().data
    }
    pub fn data_mut(&mut self) -> &mut E::Data<'a> {
        &mut self.0.locked_mut().data
    }

    //Slots
    pub fn slots(&self) -> &SlotVec<'a, F, E> {
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

    //priv
    fn busy(&self) -> &BusySlot<'a, F, E> {
        unsafe {
            let inlock = &*self.0.group.locked.get();
            match &inlock.state {
                State::Busy(data) => return data,
                _ => unreachable!(),
            }
        }
    }
    fn busy_mut(&mut self) -> &mut BusySlot<'a, F, E> {
        unsafe {
            let inlock = &mut *self.0.group.locked.get();
            match &mut inlock.state {
                State::Busy(data) => return data,
                _ => unreachable!(),
            }
        }
    }

    pub fn erase_state(self) -> GroupGuard<'a, F, E> {
        self.0
    }

    //pub fn as_raw(&self) ->
}

#[repr(transparent)]
pub struct IdleGroup<'a, F, E>(GroupGuard<'a, F, E>)
where
    F: ThreadModel,
    E: GroupParts<F>;

impl<'a, F, E> IdleGroup<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    //priv
    fn idle(&self) -> &IdleSlot<'a, F, E> {
        unsafe {
            let inlock = &*self.0.group.locked.get();
            match &inlock.state {
                State::Idle(data) => return data,
                _ => unreachable!(),
            }
        }
    }
    fn idle_mut(&mut self) -> &mut IdleSlot<'a, F, E> {
        unsafe {
            let inlock = &mut *self.0.group.locked.get();
            match &mut inlock.state {
                State::Idle(data) => return data,
                _ => unreachable!(),
            }
        }
    }

    ///busy中必须是有效数据
    pub unsafe fn swap_state(
        mut self,
        busy: BusySlot<'a, F, E>,
    ) -> (BusyGroup<'a, F, E>, IdleSlot<'a, F, E>) {
        let mut output = State::Busy(busy);
        mem::swap(&mut self.0.locked_mut().state, &mut output);
        match output {
            State::Idle(idle) => return (BusyGroup(self.0), idle),
            _ => unreachable!(),
        }
    }

    ///安全性：f不发生panic
    pub unsafe fn to_busy(
        mut self,
        f: impl FnOnce(IdleSlot<'a, F, E>) -> BusySlot<'a, F, E>,
    ) -> BusyGroup<'a, F, E> {
        unsafe { self.0.locked_mut().state.idle_to_busy_unchecked(f) };
        BusyGroup(self.0)
    }

    //Data
    pub fn data(&self) -> &E::Data<'a> {
        &self.0.locked().data
    }
    pub fn data_mut(&mut self) -> &mut E::Data<'a> {
        &mut self.0.locked_mut().data
    }

    //Idle Data
    pub fn idle_data(&self) -> &E::IdleData<'a> {
        &self.idle().data
    }
    pub fn idle_data_mut(&mut self) -> &mut E::IdleData<'a> {
        &mut self.idle_mut().data
    }

    pub fn into_raw(self) -> GroupGuard<'a, F, E> {
        self.0
    }
}

#[repr(transparent)]
pub struct ReporterBusy<'a, F, E>(ReporterGuard<'a, F, E>)
where
    F: ThreadModel,
    E: GroupParts<F>;

impl<'a, F, E> ReporterBusy<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    pub fn new_reporter(
        &mut self,
        slot_ext: E::SlotShare<'a>,
        slot_inlock: E::SlotData<'a>,
    ) -> Reporter<'a, F, E> {
        unsafe {
            new_reporter(
                &self.0.group,
                &mut self.busy_mut().slots,
                slot_ext,
                slot_inlock,
            )
        }
    }

    pub fn emit_result(mut self, result: E::IdleData<'a>) -> E::Waker<'a> {
        let (idle, busy) = self.swap_state(result.into());

        let waker = busy.waker;
        let slots = busy.slots;
        let data = busy.data;
        waker
    }

    pub fn swap_state(
        mut self,
        idle: IdleSlot<'a, F, E>,
    ) -> (ReporterIdle<'a, F, E>, BusySlot<'a, F, E>) {
        let mut output = State::Idle(idle);
        mem::swap(&mut self.0.locked_mut().state, &mut output);
        match output {
            State::Busy(busy) => return (ReporterIdle(self.0), busy),
            _ => unreachable!(),
        }
    }

    ///安全性：f不发生panic
    pub unsafe fn to_idle(
        mut self,
        f: impl FnOnce(BusySlot<'a, F, E>) -> IdleSlot<'a, F, E>,
    ) -> ReporterIdle<'a, F, E> {
        unsafe { self.0.locked_mut().state.busy_to_idle_unchecked(f) };
        ReporterIdle(self.0)
    }

    //Data
    pub fn data(&self) -> &E::Data<'a> {
        &self.0.locked().data
    }
    pub fn data_mut(&mut self) -> &mut E::Data<'a> {
        &mut self.0.locked_mut().data
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

    //priv
    fn busy(&self) -> &BusySlot<'a, F, E> {
        unsafe {
            let inlock = &*self.0.group.locked.get();
            match &inlock.state {
                State::Busy(busy) => return busy,
                _ => unreachable!(),
            }
        }
    }
    fn busy_mut(&mut self) -> &mut BusySlot<'a, F, E> {
        unsafe {
            let inlock = &mut *self.0.group.locked.get();
            match &mut inlock.state {
                State::Busy(busy) => return busy,
                _ => unreachable!(),
            }
        }
    }

    pub fn into_raw(self) -> ReporterGuard<'a, F, E> {
        self.0
    }

    //transmute
    pub fn as_group(&self) -> &BusyGroup<'a, F, E> {
        unsafe { mem::transmute(&self.0.group) }
    }
    pub fn as_group_mut(&mut self) -> &mut BusyGroup<'a, F, E> {
        unsafe { mem::transmute(&mut self.0.group) }
    }
}

#[repr(transparent)]
pub struct ReporterIdle<'a, F, E>(ReporterGuard<'a, F, E>)
where
    F: ThreadModel,
    E: GroupParts<F>;

impl<'a, F, E> ReporterIdle<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    //priv Idle
    fn idle(&self) -> &IdleSlot<'a, F, E> {
        unsafe {
            let inlock = &*self.0.group.locked.get();
            match &inlock.state {
                State::Idle(data) => return data,
                _ => unreachable!(),
            }
        }
    }
    fn idle_mut(&mut self) -> &mut IdleSlot<'a, F, E> {
        unsafe {
            let inlock = &mut *self.0.group.locked.get();
            match &mut inlock.state {
                State::Idle(data) => return data,
                _ => unreachable!(),
            }
        }
    }

    ///安全性：busy中是有效数据
    pub unsafe fn swap_state(
        mut self,
        busy: BusySlot<'a, F, E>,
    ) -> (ReporterBusy<'a, F, E>, IdleSlot<'a, F, E>) {
        let mut output = State::Busy(busy);
        mem::swap(&mut self.0.locked_mut().state, &mut output);
        match output {
            State::Idle(idle) => return (ReporterBusy(self.0), idle),
            _ => unreachable!(),
        }
    }

    ///安全性：确保f不会Panic
    pub unsafe fn to_busy(
        mut self,
        f: impl FnOnce(IdleSlot<'a, F, E>) -> BusySlot<'a, F, E>,
    ) -> ReporterBusy<'a, F, E> {
        unsafe { self.0.locked_mut().state.idle_to_busy_unchecked(f) };
        ReporterBusy(self.0)
    }

    //Data
    pub fn data(&self) -> &E::Data<'a> {
        &self.0.locked().data
    }
    pub fn data_mut(&mut self) -> &mut E::Data<'a> {
        &mut self.0.locked_mut().data
    }

    //Idle Data
    pub fn idle_data(&self) -> &E::IdleData<'a> {
        &self.idle().data
    }
    pub fn idle_data_mut(&mut self) -> &mut E::IdleData<'a> {
        &mut self.idle_mut().data
    }

    pub fn into_raw(self) -> ReporterGuard<'a, F, E> {
        //earse state
        self.0
    }

    ///向上转型为IdleGroup
    pub fn as_group(&self) -> &IdleGroup<'a, F, E> {
        unsafe { mem::transmute(&self.0.group) }
    }
    pub fn as_group_mut(&mut self) -> &mut IdleGroup<'a, F, E> {
        unsafe { mem::transmute(&mut self.0.group) }
    }

    ///向上转型为Reporter
    pub fn as_report(&self) -> &ReporterGuard<'a, F, E> {
        &self.0
    }
    pub fn as_report_mut(&mut self) -> &mut ReporterGuard<'a, F, E> {
        &mut self.0
    }
}

//#[derive(Clone, Debug, Default)]
struct SlotVec<'a, F, E>(pub Vec<Slot<'a, F, E>>)
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
struct GroupShared<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    share: E::GroupShare<'a>,

    mutex: F::Mutex,
    locked: SyncUnsafeCell<InLockShared<'a, F, E>>,
}

impl<'a, F: ThreadModel, E: GroupParts<F>> GroupShared<'a, F, E> {
    fn with_raw(share: E::GroupShare<'a>, inlock: InLockShared<'a, F, E>) -> Self {
        Self {
            share,

            mutex: F::Mutex::new(),
            locked: SyncUnsafeCell::new(inlock),
        }
    }
}
type RefGroupShared<'a, F: ThreadModel, E: GroupParts<F>> = F::RefCounter<GroupShared<'a, F, E>>;

type GroupGuardState<'a, F: ThreadModel, E: GroupParts<F>> =
    State<BusyGroup<'a, F, E>, IdleGroup<'a, F, E>>;
type ReporterGuardState<'a, F: ThreadModel, E: GroupParts<F>> =
    State<ReporterBusy<'a, F, E>, ReporterIdle<'a, F, E>>;

struct InLockShared<'a, F: ThreadModel, E: GroupParts<F>> {
    data: E::Data<'a>,
    state: State<BusySlot<'a, F, E>, IdleSlot<'a, F, E>>,
}

pub enum State<T, U> {
    Busy(T),
    Idle(U),
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
            match self {
                Self::Busy(busy) => {
                    ptr::write(this, State::Idle(f(ptr::read::<T>(busy as *const T))))
                }
                _ => unreachable_unchecked(),
            }
        }
    }

    ///安全性：
    /// self为idle变体，且f不会产生panic
    pub unsafe fn idle_to_busy_unchecked(&mut self, f: impl FnOnce(U) -> T) {
        unsafe {
            let this = self as *mut Self;
            match self {
                Self::Idle(idle) => {
                    ptr::write(this, State::Busy(f(ptr::read::<U>(idle as *const U))))
                }
                _ => unreachable_unchecked(),
            };
        }
    }

    pub fn idle(self) -> Option<U> {
        match self {
            Self::Idle(idle) => Some(idle),
            _ => None,
        }
    }

    pub fn busy(self) -> Option<T> {
        match self {
            Self::Busy(busy) => Some(busy),
            _ => None,
        }
    }
}

//TODO: into priv
struct BusySlot<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    slots: SlotVec<'a, F, E>,
    data: E::BusyData<'a>,
    waker: E::Waker<'a>,
}

impl<'a, F, E> BusySlot<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    pub fn empty(data: E::BusyData<'a>, waker: E::Waker<'a>) -> Self {
        Self {
            slots: SlotVec(Vec::new()),
            data,
            waker,
        }
    }

    pub unsafe fn with_raw(
        slots: SlotVec<'a, F, E>,
        data: E::BusyData<'a>,
        waker: E::Waker<'a>,
    ) -> Self {
        Self { slots, data, waker }
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

    pub fn into_raw(self) -> (SlotVec<'a, F, E>, E::BusyData<'a>, E::Waker<'a>) {
        (self.slots, self.data, self.waker)
    }
}

pub struct IdleSlot<'a, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    pub data: E::IdleData<'a>,
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
    type GroupShare<'a>;

    //GroupInLock
    // 访问这四项需要解锁
    type Data<'a>;
    type IdleData<'a>;
    type BusyData<'a>;
    type SlotData<'a>; //每个线程一份

    ///每个线程一份的只读数据
    type SlotShare<'a>;

    type Waker<'a>: Wake;
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

struct GroupGuardNew<'a, 'b, F, E>(&'a DownloadGroup<'b, F, E>)
where
    F: ThreadModel,
    E: GroupParts<F>;

impl<'a, 'b, F, E> GroupGuardNew<'a, 'b, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    fn new(unlocked: &'a DownloadGroup<'b, F, E>) -> Self{
        unlocked.0.mutex.acquire();
        Self(unlocked)
    }

    unsafe fn new_unchecked(unlocked: &'a DownloadGroup<'b, F, E>) -> Self {
        Self(unlocked)
    }
}

impl<'a, 'b, F, E> Drop for GroupGuardNew<'a, 'b, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    fn drop(&mut self) {
        self.0.0.mutex.release();
    }
}


struct BusyGroupNew<'a, 'b, F, E>(GroupGuardNew<'a, 'b, F, E>)
where
    F: ThreadModel,
    E: GroupParts<F>;

impl<'a, 'b, F, E> BusyGroupNew<'a, 'b, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    pub unsafe fn new_unchecked(guard: GroupGuardNew<'a, 'b, F, E>) -> Self {
        Self(guard)
    }

}

struct IdleGroupNew<'a, 'b, F, E>(GroupGuardNew<'a, 'b, F, E>)
where
    F: ThreadModel,
    E: GroupParts<F>;


impl<'a, 'b, F, E> IdleGroupNew<'a, 'b, F, E>
where
    F: ThreadModel,
    E: GroupParts<F>,
{
    pub unsafe fn new_unchecked(guard: GroupGuardNew<'a, 'b, F, E>) -> Self {
        Self(guard)
    }
}









///向上转型，获取父类
trait UpCast<Super> {
    fn upcast(&self) -> &Super;
    fn upcast_mut(&mut self) -> &mut Super;
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
