//!组下载管理器

use std::{num::NonZero, ops::Deref, sync::atomic::Ordering, task::Waker};

use radium::Radium;
use reqwest::blocking::Client;

use crate::{
    base::{
        family::ThreadModel,
        group_construct::{DownloadGroup, GroupGuard, Reporter, State},
        segment::Segment,
    },
    downloader::{
        group_async_parts::{DownloadGroup2, IdleGroup2, Residual, TaskShare},
        group_download_methold::{Downloader, SegmentResume},
        group_init::ManagerInitExt,
        group_worker::SegmentWorker,
    },
};

use crate::downloader::group_async_parts::{AsyncParts, Residual, RunningData};

use crate::downloader::group_async_parts::BusyGroup2;

// 状态
// 初始：管理器空闲，下载组空闲 -创建下载-> 下载中；
// 下载中：管理器运行中，下载器繁忙 -下载完成-> 完成；
// 完成：管理器运行中，下载器空闲 -管理器收到waker消息-> 初始(可供复用）；
//
// 未指定：管理器空闲，下载器繁忙
pub(crate) struct RunningManager<E, M>
where
    M: ThreadModel,
{
    group: DownloadGroup<'static, M, AsyncParts<E>>,
    //abort: M::RefCounter<M::AtomicCell<bool>>,
    task: M::RefCounter<TaskShare<M>>,
}

impl<M, E> RunningManager<E, M>
where
    M: ThreadModel,
{
    pub fn new_with<I: ManagerInitExt>(initer: I) -> Self {
        IdleManager::new().init_with(initer)
    }

    fn id(&self) -> usize {
        self.task.deref() as *const _ as usize
    }

    pub fn clone_share(&self) -> M::RefCounter<TaskShare<M>> {
        self.task.clone()
    }

    ///检查下载是否完成
    pub fn take_residual(self) -> MapRunning<(), E, M> {
        self.map_group(|_| {})
    }

    ///设置唤醒器
    pub fn set_waker(self, waker: Waker) -> MapRunning<(), E, M> {
        self.map_group(|busy| {
            *busy.busy_data_mut().waker = waker;
        })
    }

    ///取消全部
    pub fn abort_all(self) -> MapRunning<Vec<Segment>, E, M> {
        self.map_group(|busy| {
            self.task.abort_single.store(true, Ordering::Relaxed);
            let (_, b, c) = busy.into_idle(None);
            let segments =
                b.0.iter()
                    .map(|slot| {
                        let end = slot.data.end;
                        let remain = slot.share.ext.remain.load(Ordering::Relaxed);
                        return Segment::new(end - remain, NonZero::new(remain).unwrap());
                    })
                    .collect();
            segments
        })
    }

    ///克隆分段信息
    pub fn clone_segments(self) -> MapRunning<impl Iterator, E, M> {
        self.map_group(|mut busy| {
            let mut segments = Vec::with_capacity(busy.slots().len());
            for i in busy.slots_mut().0 {
                segments.push(todo!());
            }
            return segments.into_iter();
        })
    }

    // ///执行任务窃取
    // pub fn do_stealing_work<D: Downloader<SegmentWorker<E, M>>>(
    //     self,
    //     downloader: D,
    // ) -> MapRunning<Option<impl Future>, E, M> {
    //     self.map_group(|busy| {
    //         let Some((max, remain)) = busy.find_max_remain() else {
    //             return None;
    //         };
    //         let new_remain = remain / 2;
    //         let new_end = max.data.end;
    //         max.share
    //             .ext
    //             .remain
    //             .fetch_sub(new_remain, Ordering::Relaxed);
    //         max.data.end -= new_remain;
    //         let new_segment = Segment::new(new_end - new_remain, NonZero::new(new_remain).unwrap());
    //         let reporter = busy.submit_segment(new_segment);
    //         let ctx = SegmentWorker::new(reporter, self.task.clone());
    //         let future = ctx.work_send_to_executer(downloader);
    //         return Some(future);
    //     })
    // }

    ///执行任务窃取
    pub fn stealing_work<D: Downloader<SegmentWorker<E, M>>>(
        self,
        downloader: D,
    ) -> MapRunning<Option<impl Future>, E, M> {
        self.map_manager(|busy, task| {
            let Some((max, remain)) = busy.find_max_remain() else {
                return None;
            };
            let new_remain = remain / 2;
            let new_end = max.data.end;
            max.share
                .ext
                .remain
                .fetch_sub(new_remain, Ordering::Relaxed);
            max.data.end -= new_remain;
            let new_segment = Segment::new(new_end - new_remain, NonZero::new(new_remain).unwrap());
            let reporter = busy.submit_segment(new_segment);
            (reporter, task)
        })
        .map_result(|(reporter, task)| {
            let ctx = SegmentWorker::new(reporter, task.clone());
            let future = ctx.work_send_to_executer(downloader);
            return Some(future);
        })
    }

    ///从恢复策略中创建任务
    pub fn resume_work<R: SegmentResume>(self, resumer: R) -> MapRunning<impl Future, E, M> {
        let (segment, downloader) = resumer.resume();

        self.map_manager(|group, task| {
            let reporter = group.submit_segment(segment);
            (reporter, task)
        })
        .map_result(|(r, task)| {
            let ctx = SegmentWorker::new(r, task.clone());
            ctx.work_send_to_executer(downloader)
        })
    }

    ///创建新的下载句柄
    pub fn new_segment_worker(self, segment: Segment) -> MapRunning<SegmentWorker<E, M>, E, M> {
        self.map_group(|busy| {
            let reporter = busy.submit_segment(segment);
            SegmentWorker::new(reporter, self.clone_share())
        })
    }

    ///最基本操作
    pub fn map_group<T>(self, f: impl FnOnce(BusyGroup2<'_, E, M>) -> T) -> MapRunning<T, E, M> {
        let guard: GroupGuard<'_, 'static, M, AsyncParts<E>> = self.group.lock();
        match guard.state() {
            State::Running(r) => {
                let t = f(r);
                return MapRunning::Running((self, t));
            }
            State::Idle(mut i) => {
                let residual = i.idle_data_mut().take();
                let idle = IdleManager(self.0);
                return MapRunning::Idle(RunResult::new(idle, residual));
            }
        }
    }

    pub fn map_manager<T>(
        self,
        f: impl FnOnce(BusyGroup2<'_, E, M>, &M::RefCounter<TaskShare<M>>) -> T,
    ) -> MapRunning<T, E, M> {
        let share = &self.task;
        let guard = self.lock_guard();
        match guard.state() {
            State::Running(r) => {
                let t = f(r, share);
                return MapRunning::Running((self, t));
            }

            State::Idle(mut i) => {
                let residual = i.idle_data_mut().take();
                let idle = IdleManager(self.group);
                return MapRunning::Idle(RunResult::new(idle, residual));
            }
        }
    }

    ///
    pub fn lock_guard(&self) -> GroupGuard<'_, 'static, M, AsyncParts<E>> {
        self.0.lock()
    }
}

type MapRunning<R, E, M> = State<(RunningManager<E, M>, R), RunResult<E, M>>;
type MapState<E, M> = MapRunning<(), E, M>;

impl<T, E, M> MapRunning<T, E, M>
where
    M: ThreadModel,
{
    fn and_then<U>(
        self,
        f: impl FnOnce(RunningManager<E, M>, T) -> Self<U, E, M>,
    ) -> MapRunning<U, E, M> {
        match self {
            State::Running(t) => return f(t),
            State::Idle(i) => return State::Idle(i),
        }
    }

    fn map_result<U>(self, f: impl FnOnce(T) -> U) -> MapRunning<U, E, M> {
        match self {
            State::Running((m, t)) => State::Running((m, f(t))),
            State::Idle(i) => State::Idle(i),
        }
    }
}

impl<E, M> MapRunning<(), E, M>
where
    M: ThreadModel,
{
    fn from_running(manager: RunningManager<E, M>) -> Self {
        Self::Running((manager, ()))
    }

    fn ingore_void_result(self) -> State<RunningManager<E, M>, RunResult<E, M>> {
        self.map_busy(|(m, _)| m)
    }
}

impl<T, E, M> MapRunning<T, E, M>
where
    M: ThreadModel,
{
    fn from_running_result(manager: RunningManager<E, M>, result: T) -> Self {
        Self::Running((manager, result))
    }

    fn from_idle(group: DownloadGroup2<E, M>, residual: Option<Residual<E>>) -> Self {
        Self::Idle(RunResult::new(IdleManager(group), residual))
    }
}

struct RunResult<E, M>
where
    M: ThreadModel,
{
    manager: IdleManager<E, M>,
    residual: Option<Residual<E>>,
}

impl<E, M: ThreadModel> RunResult<E, M> {
    fn new(manager: IdleManager<E, M>, residual: Option<Residual<E>>) -> Self {
        Self { manager, residual }
    }
    fn into_raw(self) -> (IdleManager<E, M>, Option<Residual<E>>) {
        (self.manageger, self.residual)
    }

    fn take_residual(&mut self) -> Option<Residual<E>> {
        self.residual
    }

    fn manager(self) -> IdleManager<E, M> {
        self.manager
    }
}

/// 主要进行下载任务的初始化
///
/// 表示未运行且已取出结果的Group
#[repr(transparent)]
pub(crate) struct IdleManager<E, M>(DownloadGroup<'static, M, AsyncParts<E>>)
where
    M: ThreadModel;

impl<M, E> IdleManager<E, M>
where
    M: ThreadModel,
{
    fn new() -> Self {
        let idle = IdleData { error_info: None };
        let group = DownloadGroup::new_idle((), idle);
        Self(group)
    }

    fn init_with<I: ManagerInitExt>(self, initer: I) -> RunningManager<E, M> {
        let guard = self.0.lock();
        match guard.state() {
            State::Idle(i) => {
                todo!()
            }
            State::Running(_) => {
                panic!("Manager is Idle，but Group is Running")
            }
        }
    }

    fn lock_guard(&self) -> GroupGuard<'_, 'static, M, AsyncParts<E>> {
        self.0.lock()
    }

    pub(crate) fn unwarp_idle_group(&self) -> IdleGroup2<E, M> {
        self.0.lock().state().idle().unwrap()
    }

    pub fn into_done_running_manager(self) -> RunningManager<E, M> {
        //因为IdleManager的结果为None，所以能直接转换为RunningManager
        RunningManager {
            group: self.0,
            task: M::RefCounter::new(TaskShare::new()),
        }
    }
}

// struct RunningManagerGuard<'a, E, M: ThreadModel> {
//     group: BusyGroup2<'a, E, M>,
//     share: &'a M::RefCounter<GroupShare<M>>,
// }

// impl<'a, E, M: ThreadModel> RunningManagerGuard<'a, E, M> {
//     fn create_new_worker(&self) -> SegmentWorker<E, M> {
//         todo!()
//     }
// }

// struct ManagerHaveResult<'a, E, M: ThreadModel> {
//     group: BusyGroup2<'a, E, M>,
// }

// // trait ManagerVisit {
// //     async fn visit<M: ThreadModel, E>(
// //         manager: RunningManager<E, M>,
// //     ) -> State<RunningManager<E, M>, IdleManager<E, M>>;
// // }

enum Manager<E, M: ThreadModel> {
    Running(RunningManager<E, M>),
    Idle(IdleManager<E, M>),
}
