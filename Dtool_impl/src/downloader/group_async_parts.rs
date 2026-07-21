use std::{
    marker::PhantomData, num::NonZero, ops::ControlFlow, sync::atomic::Ordering, task::Waker,
};

use radium::Radium;

use crate::base::{
    base_error::{Aborted, MayAbort, RawError, SuperError},
    family::ThreadModel,
    group_construct::{
        BusyGroup, BusyReporter, DownloadGroup, GroupGuard, GroupParts, IdleGroup, IdleReporter,
        Reporter, ReporterBusy, ReporterGuard, Slot, SlotShare, SlotVec,
    },
    request_info::RequestInfo,
    segment::Segment,
};

pub type DownloadGroup2<E, M> = DownloadGroup<'static, M, AsyncParts<E>>;
pub type Reporter2<E, M> = Reporter<'static, M, AsyncParts<E>>;

pub type GroupGuard2<'a, E, M> = GroupGuard<'a, 'static, M, AsyncParts<E>>;
pub type ReporterGuard2<'a, E, M> = ReporterGuard<'a, 'static, M, AsyncParts<E>>;

pub type BusyGroup2<'a, E, M> = BusyGroup<'a, 'static, M, AsyncParts<E>>;
pub type IdleGroup2<'a, E, M> = IdleGroup<'a, 'static, M, AsyncParts<E>>;

pub type BusyReporter2<'a, E, M> = BusyReporter<'a, 'static, M, AsyncParts<E>>;
pub type IdleReporter2<'a, E, M> = IdleReporter<'a, 'static, M, AsyncParts<E>>;

pub type Slot2<E, M> = Slot<'static, M, AsyncParts<E>>;
pub type SlotVec2<E, M> = SlotVec<'static, M, AsyncParts<E>>;

pub struct AsyncParts<E>(PhantomData<E>);

impl<F: ThreadModel, E> GroupParts<F> for AsyncParts<E> {
    type StaticData<'a> = GroupShareData<F>;

    type Result<'a> = Option<Residual<E>>; //运行结果
    type Data<'a> = RunningData; //唤醒器
    type SlotData<'a> = SlotData; //结束位置

    type SlotShare<'a> = SlotShareData<F>; //进度，取消标志
}

pub struct GroupShare<M: ThreadModel> {
    pub abort_single: M::AtomicCell<bool>,
}

impl<M: ThreadModel> GroupShare<M> {
    pub fn new() -> Self {
        Self {
            abort_single: M::AtomicCell::new(false),
        }
    }
}

struct GroupShareData<F: ThreadModel> {
    info: RequestInfo,
    progress: F::AtomicCell<u64>,
}

pub struct RunningData {
    pub(crate) waker: Waker,
    pub(crate) info: RequestInfo,

    // 延迟结束线程
    // 在创建新线程时，将此值-1就不用实际创建线程了
    pub(crate) lazy_cancel_count: usize,
}

///上次运行失败的残留值
pub struct Residual<E> {
    error_or_aborted: Option<E>, //None说明被取消
    error_segment: Segment,
    segments: Vec<Segment>,
}

struct SlotData {
    end: u64,
}

struct SlotShareData<F: ThreadModel> {
    pub(crate) abort: F::AtomicCell<bool>,
    pub(crate) remain: F::AtomicCell<u64>,
}

impl<E, M: ThreadModel> Slot2<E, M> {
    pub fn save_as_segment(&self) -> Segment {
        let end = self.data.end;
        let remain = self.share.ext.remain.load(Ordering::Relaxed);
        Segment::new(end - remain, NonZero::new(remain).unwrap())
    }
}

impl<E, M: ThreadModel> Slot2<E, M> {
    fn load_remain(&self, order: Ordering) -> u64 {
        self.share.ext.remain.load(order)
    }
    fn remain(&self) -> &M::AtomicCell<u64> {
        &self.share.ext.remain
    }
}

impl<'t, E, M: ThreadModel> BusyGroup2<'t, E, M> {
    ///任务窃取
    pub fn task_stealing(&self, min: u64) -> Option<Reporter2<E, M>> {
        self.split_the_biggest_slot(min)
            .map(|s| self.submit_segment(s))
    }

    pub fn submit_segment(&self, segment: Segment) -> Reporter2<E, M> {
        let index = self.slots().len();

        let slot_data = SlotData { end: segment.end() };
        let slot_share = M::RefCounter::new(SlotShare {
            index: index.into(),
            ext: SlotShareData {
                abort: M::AtomicCell::new(false),
                remain: M::AtomicCell::new(segment.remain.get()),
            },
        });

        let slot = Slot2::<E, M> {
            share: slot_share.clone(),
            data: slot_data,
        };
        self.slots_mut().push_slot(slot);

        let reporter = Reporter2 {
            slot_share,
            group: self.0.0.clone(),
        };
        reporter
    }

    ///priv
    fn split_the_biggest_slot(&self, min: u64) -> Option<Segment> {
        let max_slot = self
            .slots()
            .0
            .iter_mut()
            .map(|s| (s, s.load_remain(Ordering::Relaxed)))
            .max_by_key(|(s, remain)| remain);

        max_slot
            .filter(|(_, remain)| *remain > min * 2)
            .map(|(slot, remain)| {
                let new_remain = remain / 2;
                slot.data.end -= new_remain;
                slot.remain().fetch_sub(new_remain, Ordering::Relaxed);
                todo!()
            })
        // max_slot.map(|(slot, remain)| {
        //     let new_remain = remain / 2;
        //     slot.remain().fetch_sub(remain / 2, Ordering::Relaxed);
        // })
    }

    fn find_biggest_slot(&self) -> Option<&Slot2<E, M>> {
        self.slots()
            .0
            .iter()
            .max_by_key(|slot| slot.share.ext.remain.load(Ordering::Relaxed))
    }
}

// impl<E, M: ThreadModel> SlotVec2<E, M> {
//     fn push_
// }
