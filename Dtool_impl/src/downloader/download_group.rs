//!定义分块下载的并发结构体
//!

use std::cell::UnsafeCell;
use std::error::Error;
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
#[repr(transparent)]
pub struct DownloadGroup<'a, F, E>(pub F::RefCounter<GroupShared<'a, F, E>>)
where
    F: ThreadModel,
    E: GroupExt<F>;

//可以访问：&GroupShareExt, Lock
impl<'a, F, E> DownloadGroup<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    // pub fn new(share_ext: E::GroupShare<'a>, inlock_ext: E::InLockExt<'a>) -> Self {
    //     Self(F::RefCounter::new(GroupShared::with_raw(share_ext, inlock_ext)))
    // }

    pub fn new(group: E::GroupShare<'a>, data: E::Data<'a>, idle_data: E::IdleData<'a>) -> Self {
        let inlockshared = InLockShared{
            data,
            state: State::Idle(Idle{data: idle_data})
        };

        Self(F::RefCounter::new(GroupShared::with_raw(group, inlockshared)))
    }

    pub fn lock(self) -> GroupGuard<'a, F, E> {
        GroupGuard::new(self)
    }

    pub fn share(&self) -> &E::GroupShare<'a> {
        &self.0.share
    }
}

///每个下载分块向下载组报告状态的结构体
/// 这个结构体是生产者也是消费者
pub struct Reporter<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    group: F::RefCounter<GroupShared<'a, F, E>>,
    slot_share: F::RefCounter<SlotShare<'a, F, E>>,
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
    //<F as ThreadModel>::Mutex<InLockShared<'a, F, E>>: 'a, // 满足 Lockable Trait 的 GAT 约束
    F: ThreadModel,
    E: GroupExt<F>,
{
    group: F::RefCounter<GroupShared<'a, F, E>>,
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
        &self.group.share
    }

    // data
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
            State::Idle(idle) => State::Idle(&idle.data)
        }
    }
    pub fn state_data_mut(&mut self) -> State<&mut E::BusyData<'a>, &mut E::IdleData<'a>> {
        match &mut self.locked_mut().state {
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

    //state
    pub fn get_state(self) -> GroupGuardState<'a, F, E> {
        match self.locked().state {
            State::Busy(_) => State::Busy(BusyGroup(self)),
            State::Idle(_) => State::Idle(IdleGroup(self))
        }
    }
    
    pub fn as_state(&self) -> State<&BusyGroup<'a, F, E>, &IdleGroup<'a, F, E>> {
        unsafe{
            match self.locked().state {
                State::Busy(_) => State::Busy(mem::transmute(self)),
                State::Idle(_) => State::Idle(mem::transmute(self))
            }
        }
    }
    pub fn as_state_mut(&mut self) ->State<&mut BusyGroup<'a, F, E>, &mut IdleGroup<'a, F, E>> {
        unsafe {
            match self.locked().state {
                State::Busy(_) => State::Busy(mem::transmute(self)),
                State::Idle(_) => State::Idle(mem::transmute(self))
            }
        }
    }


}

impl<'a, F: ThreadModel, E: GroupExt<F>> Drop for GroupGuard<'a, F, E> {
    fn drop(&mut self) {
        self.group.mutex.release();
    }
}
///reporter WriteGuard
pub struct ReporterGuard<'a, F, E>
where 
    F: ThreadModel, 
    E: GroupExt<F> 
{
    slot: F::RefCounter<SlotShare<'a, F, E>>,
    group: F::RefCounter<GroupShared<'a, F, E>>,
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


    //state
    pub fn get_state(self) -> ReporterGuardState<'a, F, E> {
        match self.locked().state {
            State::Busy(_) => State::Busy(ReporterBusy(self)),
            State::Idle(_) => State::Idle(ReporterIdle(self))
        }
    }
    pub fn as_state(&self) -> State<&ReporterBusy<'a, F, E>, &ReporterIdle<'a, F, E>> {
        unsafe{
            match self.locked().state {
                State::Busy(_) => State::Busy(mem::transmute(self)),
                State::Idle(_) => State::Idle(mem::transmute(self))
            }
        }
    }
    pub fn fetech_state_mut(&mut self) ->State<&mut ReporterBusy<'a, F, E>, &mut ReporterIdle<'a, F, E>> {
        unsafe {
            match self.locked().state {
                State::Busy(_) => State::Busy(mem::transmute(self)),
                State::Idle(_) => State::Idle(mem::transmute(self))
            }
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
        match &self.locked().state{
            State::Busy(busy) => State::Busy(&busy.data),
            State::Idle(idle) => State::Idle(&idle.data)
        }
    }
    pub fn state_data_mut(&mut self) -> State<&mut E::BusyData<'a>, &mut E::IdleData<'a>> {
        match &mut self.locked_mut().state {
            State::Busy(busy) => State::Busy(&mut busy.data),
            State::Idle(idle) => State::Idle(&mut idle.data)
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
    
    //transmute
    pub fn as_group(&self) -> &GroupGuard<'a, F, E> {
        unsafe { mem::transmute(&self.group)}
    }
    pub fn as_group_mut(&mut self) -> &mut GroupGuard<'a, F, E> {
        unsafe { mem::transmute(&mut self.group)}
    }
    
}

// impl<'a, F: ThreadModel, E: GroupExt<F>> AsRef<GroupGuard<'a, F, E>> for ReporterGuard<'a, F, E> {
//     fn as_ref(&self) -> &GroupGuard<'a, F, E> {
//         unsafe{ mem::transmute(&self.group)}
//     }
// }

impl<'a, F, E> Drop for ReporterGuard<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn drop(&mut self) {
        self.group.mutex.release();
    }
}



// -----------Busy and Idle API -------------------

#[repr(transparent)]
pub struct BusyGroup<'a, F, E>(
    GroupGuard<'a, F, E>
)
where 
    F: ThreadModel,
    E: GroupExt<F>;

impl<'a, F, E> BusyGroup<'a, F, E>
where 
    F: ThreadModel,
    E: GroupExt<F>,
{

    //priv
    fn busy(&self) -> &Busy<'a, F, E> {
        unsafe{
            let inlock = & *self.0.group.locked.get();
            match & inlock.state {
                State::Busy(data) => return data,
                _ => unreachable!()
            }
        }
    }
    fn busy_mut(&mut self) -> &mut Busy<'a, F, E> {
        unsafe{
            let inlock = &mut *self.0.group.locked.get();
            match &mut inlock.state {
                State::Busy(data) => return data,
                _ => unreachable!()
            }
        }
    }

    pub fn new_reporter(&mut self, slot_ext: E::SlotShare<'a>, slot_inlock: E::SlotInlock<'a>) -> Reporter<'a, F, E>{
        unsafe{
            new_reporter(&self.0.group, &mut self.busy_mut().slots, slot_ext, slot_inlock)
        }
    }

    pub fn swap_state(mut self, idle: Idle<'a, F, E>) -> (IdleGroup<'a, F, E>, Busy<'a, F, E>) {
        let mut output = State::Idle(idle);
        let inlock = &mut self.0.locked_mut().state;
        mem::swap(inlock, &mut output);
        match output {
            State::Busy(busy) => {
                return (IdleGroup(self.0), busy)
            }
            _ => unsafe{ unreachable_unchecked() }
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

    //pub fn as_raw(&self) -> 
}

#[repr(transparent)]
pub struct IdleGroup<'a, F, E>(
    GroupGuard<'a, F, E>
)
where 
    F: ThreadModel,
    E: GroupExt<F>;

impl<'a, F, E> IdleGroup<'a, F, E>
where 
    F: ThreadModel,
    E: GroupExt<F>,
{


    //priv
    fn idle(&self) -> &Idle<'a, F, E> {
        unsafe{
            let inlock = & *self.0.group.locked.get();
            match &inlock.state {
                State::Idle(data) => return data,
                _ => unreachable!()
            }
        }
    }
    fn idle_mut(&mut self) -> &mut Idle<'a, F, E>{
        unsafe{
            let inlock = &mut *self.0.group.locked.get();
            match &mut inlock.state {
                State::Idle(data) => return data,
                _ => unreachable!() 
            }
        }
    }

    ///busy中必须是有效数据
    pub unsafe fn swap_state(mut self, busy: Busy<'a, F, E>) -> (BusyGroup<'a, F, E>, Idle<'a, F, E>) {
        let mut output = State::Busy(busy);
        mem::swap(&mut self.0.locked_mut().state, &mut output);
        match output {
            State::Idle(idle) => return (BusyGroup(self.0), idle),
            _ => unreachable!()
        }

    }

    ///安全性：f不发生panic
    pub unsafe fn to_busy(mut self, f: impl FnOnce(Idle<'a, F, E>) -> Busy<'a, F, E>) -> BusyGroup<'a, F, E> {
        unsafe{ self.0.locked_mut().state.idle_to_busy_unchecked(f) };
        BusyGroup(self.0)
    }

    //Data
    pub fn data(&self) -> & E::Data<'a> {
        &self.0.locked().data
    }
    pub fn data_mut(&mut self) -> &mut E::Data<'a> {
        &mut self.0.locked_mut().data
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

#[repr(transparent)]
pub struct ReporterBusy<'a, F, E>(
    ReporterGuard<'a, F, E>
)
where 
    F: ThreadModel,
    E: GroupExt<F>;


impl<'a, F, E> ReporterBusy<'a, F, E>
where 
    F: ThreadModel,
    E: GroupExt<F>
{

    //priv
    fn busy(&self) -> &Busy<'a, F, E> {
        unsafe {
            let inlock = & *self.0.group.locked.get();
            match &inlock.state {
                State::Busy(busy) => return busy,
                _ => unreachable!(),
            }
        }
    }
    fn busy_mut(&mut self) -> &mut Busy<'a, F, E> {
        unsafe {
            let inlock = &mut *self.0.group.locked.get();
            match &mut inlock.state {
                State::Busy(busy) => return busy,
                _ => unreachable!(),
            }
        }
    }

    pub fn new_reporter(&mut self, slot_ext: E::SlotShare<'a>, slot_inlock: E::SlotInlock<'a>) -> Reporter<'a, F, E> {
        unsafe{
            new_reporter(&self.0.group, &mut self.busy_mut().slots, slot_ext, slot_inlock)
        }
    }
    
    pub fn swap_state(mut self, idle: Idle<'a, F, E>) -> (ReporterIdle<'a, F, E>, Busy<'a, F, E>) {

        let mut output = State::Idle(idle);
        mem::swap(&mut self.0.locked_mut().state, &mut output);
        match output {
            State::Busy(busy) => return (ReporterIdle(self.0), busy),
            _ => unreachable!()
        }
    }

    ///安全性：f不发生panic
    pub unsafe fn to_idle(mut self, f: impl FnOnce(Busy<'a, F, E>) -> Idle<'a, F, E>) -> ReporterIdle<'a, F, E> {
        unsafe{ self.0.locked_mut().state.busy_to_idle_unchecked(f) };
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

    pub fn into_raw(self) -> ReporterGuard<'a, F, E> {
        self.0
    }
    
    //transmute
    pub fn as_group(&self) -> &BusyGroup<'a, F, E> {
        unsafe { mem::transmute(&self.0.group)}
    }
    pub fn as_group_mut(&mut self) -> &mut BusyGroup<'a, F, E> {
        unsafe { mem::transmute(&mut self.0.group)}
    }
}

#[repr(transparent)]
pub struct ReporterIdle<'a, F, E>(
    ReporterGuard<'a, F, E>
)
where 
    F: ThreadModel,
    E: GroupExt<F>;

impl<'a, F, E> ReporterIdle<'a, F, E>
where 
    F: ThreadModel,
    E: GroupExt<F>,
{

    //priv Idle
    fn idle(&self) -> &Idle<'a, F, E> {
        unsafe{
            let inlock = & *self.0.group.locked.get();
            match &inlock.state {
                State::Idle(data) => return data,
                _ => unreachable!()
            }
        }
    }
    fn idle_mut(&mut self) -> &mut Idle<'a, F, E> {
        unsafe{
            let inlock = &mut  *self.0.group.locked.get();
            match &mut inlock.state {
                State::Idle(data) => return data,
                _ => unreachable!()
            }
        }
    }

    ///安全性：busy中是有效数据
    pub unsafe fn swap_state(mut self, busy: Busy<'a, F, E>) -> (ReporterBusy<'a, F, E>, Idle<'a, F, E>) {
        let mut output = State::Busy(busy);
        mem::swap(&mut self.0.locked_mut().state, &mut output);
        match output {
            State::Idle(idle) => return (ReporterBusy(self.0), idle),
            _ => unreachable!()
        }
    }

    ///安全性：确保f不会Panic
    pub unsafe fn to_busy(mut self, f: impl FnOnce(Idle<'a, F, E>) -> Busy<'a, F, E>) -> ReporterBusy<'a, F, E> {
        unsafe{ self.0.locked_mut().state.idle_to_busy_unchecked(f) };
        ReporterBusy(self.0)
    }

    // pub unsafe fn update_state(mut self, f: impl FnOnce(Idle<'a, F, E>) -> Busy<'a, F, E>) -> ReporterGuardBusy<'a, F, E>{
    //     //ptr::write(self.idle_mut() as *mut _, f());
    // }

    //Data
    pub fn data(&self) -> & E::Data<'a> {
        &self.0.locked().data
    }
    pub fn data_mut(&mut self) -> &mut E::Data<'a> {
        &mut self.0.locked_mut().data
    }

    //Idle Data
    pub fn idle_data(&self) -> & E::IdleData<'a> {
        &self.idle().data
    }
    pub fn idle_data_mut(&mut self) -> &mut E::IdleData<'a> {
        &mut self.idle_mut().data
    }

    pub fn into_raw(self) -> ReporterGuard<'a, F, E> {//earse state
        self.0
    }

    //transmute
    pub fn as_group(&self) -> &IdleGroup<'a, F, E> {
        unsafe { mem::transmute(&self.0.group)}
    }
    pub fn as_group_mut(&mut self) -> &mut IdleGroup<'a, F, E> {
        unsafe { mem::transmute(&mut self.0.group)}
    }

}


//#[derive(Clone, Debug, Default)]
struct SlotVec<'a, F, E>(pub Vec<Slot<'a, F, E>>)
where
    F: ThreadModel,
    E: GroupExt<F>;

impl<'a, F, E> SlotVec<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
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
}

//Index(Mut) for SlotVec
impl<'a, F, E> Index<usize> for SlotVec<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    type Output = Slot<'a, F, E>;
    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}
impl<'a, F, E> IndexMut<usize> for SlotVec<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.0[index]
    }
}
impl<'a, F, E> AsRef<Vec<Slot<'a, F, E>>> for SlotVec<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn as_ref(&self) -> &Vec<Slot<'a, F, E>> {
        &self.0
    }
}

///AsRef(Mut) for SlotVec
impl<'a, F, E> AsMut<Vec<Slot<'a, F, E>>> for SlotVec<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    fn as_mut(&mut self) -> &mut Vec<Slot<'a, F, E>> {
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
    share: E::GroupShare<'a>,

    mutex: F::Mutex,
    locked: SyncUnsafeCell<InLockShared<'a, F, E>>,
}

impl<'a, F: ThreadModel, E: GroupExt<F>> GroupShared<'a, F, E> {
    fn with_raw(share: E::GroupShare<'a>, inlock: InLockShared<'a, F, E>) -> Self {
        Self {
            share,

            mutex: F::Mutex::new(),
            locked: SyncUnsafeCell::new(inlock),
        }
    }
}
type RefGroupShared<'a, F: ThreadModel, E: GroupExt<F>> =
    F::RefCounter<GroupShared<'a, F, E>>;

//type InLockShared<'a, F: ThreadModel, E: GroupExt<F>> = State<Busy<'a, F, E>, Idle<'a, F, E>>;
type GroupGuardState<'a, F:ThreadModel, E: GroupExt<F>> = State<BusyGroup<'a, F, E>, IdleGroup<'a, F, E>>;
type ReporterGuardState<'a, F: ThreadModel, E:GroupExt<F>> = State<ReporterBusy<'a, F, E>, ReporterIdle<'a, F, E>>;

struct InLockShared<'a, F:ThreadModel, E:GroupExt<F>> {
    data: E::Data<'a>,
    state: State<Busy<'a, F, E>, Idle<'a, F, E>>
}



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

struct SlotShare<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
{
    index: SyncUnsafeCell<usize>,
    //leak &mut of this field is inposeable
    pub ext: E::SlotShare<'a>,
}
type RefSlotShare<'a, F: ThreadModel, E: GroupExt<F>> = F::RefCounter<SlotShare<'a, F, E>>;

impl<'a, F, E> SlotShare<'a, F, E>
where
    F: ThreadModel,
    E: GroupExt<F>,
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

pub trait GroupExt<F: ThreadModel> {

    type GroupShare<'a>;

    //GroupInLock
    type Data<'a>;
    type IdleData<'a>;
    type BusyData<'a>;
    
    type SlotShare<'a>;
    type SlotInlock<'a>;

    //type GroupError<'a> : Error; //Retry Info
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



// trait StateFamily: 'static + Copy{
//     type Busy<'a, F, E>;
//     type Idle<'a, F, E>;
// }

// type StateKind<'a, F, E, S: StateFamily> = State<S::Busy<'a, F, E>, S::Idle<'a, F, E>>;

// struct GroupType;
// impl StateFamily for GroupType {
//     type Busy<'a, F, E> = BusyGroup<'a, F, E>;
//     type Idle<'a, F, E> = IdleGroup<'a, F, E>;
// }

// struct ReporterType;
// impl StateFamily for ReporterType {
//     type Busy<'a, F, E> = ReporterBusy<'a, F, E>;
//     type Idle<'a, F, E> = ReporterIdle<'a, F, E>;
// }