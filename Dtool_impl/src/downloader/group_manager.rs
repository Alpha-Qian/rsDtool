//!组下载管理器

use std::{
    error::Error,
    future::poll_fn,
    mem::{self, MaybeUninit},
    ops::Deref,
    sync::atomic::{AtomicU64, Ordering},
    task::{Poll, Waker},
};

use futures::task::noop_waker;
use radium::Radium;
use reqwest::blocking::Client;

use crate::{
    base::{
        family::ThreadModel,
        group_construct::{
            BusyGroup, DownloadGroup, GroupGuard, GroupParts, IdleGroup, Reporter, State,
        },
        request_info::RequestInfo,
        segment::Segment,
    },
    downloader::{
        group_async_parts::{DownloadGroup2, GroupShare, IdleGroup2, Residual},
        group_download_methold::{RawDownloadUnInjected, SegmentDownload},
        group_init::ManagerInitExt,
        group_worker::SegmentWorker,
    },
};

use crate::downloader::group_async_parts::{AsyncParts, IdleData, RunningData};

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
    share: M::RefCounter<GroupShare<M>>,
}

impl<M, E> RunningManager<E, M>
where
    M: ThreadModel,
{
    fn new_with<I: ManagerInitExt>(initer: I) -> Self {
        IdleManager::new().init_with(initer)
    }

    fn id(&self) -> usize {
        self.share.deref() as *const _ as usize
    }

    fn take_residual(self) -> MapRunning<(), E, M> {
        self.map_running(|_| {})
    }

    fn set_waker(self, waker: Waker) -> MapRunning<(), E, M> {
        self.map_running(|busy| {
            *busy.busy_data_mut().waker = waker;
        })
    }

    fn abort_all(self) -> MapRunning<(), E, M> {
        self.map_running(|busy| {
            self.abort.store(true, Ordering::Relaxed);
            let (_, b, c) = busy.into_idle(None);
            todo!()
        })
    }

    fn clone_segments(self) -> MapRunning<impl Iterator, E, M> {
        self.map_running(|busy| {
            let mut segments = Vec::with_capacity(busy.slots().len());
            for i in busy.slots().0 {
                segments.push(todo!());
            }
            return segments.into_iter();
        })
    }

    fn new_download_task<D: RawDownloadUnInjected<Error = E>>(self) -> MapRunning<(), E, M> {
        self.map_running(|busy| todo!())
    }

    fn add_thread(self, client: Client) {
        self.map_running(|busy| {
            busy.slots_mut().push_slot(slot);
        })
    }

    ///最基本操作
    fn map_running<T>(self, f: impl FnOnce(BusyGroup2<'_, E, M>) -> T) -> MapRunning<T, E, M> {
        let guard: GroupGuard<'_, 'static, M, AsyncParts<E>> = self.0.lock();
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
            share: M::RefCounter::new(GroupShare::new()),
        }
    }
}

trait ManagerVisit {
    async fn visit<M: ThreadModel, E>(
        manager: RunningManager<E, M>,
    ) -> State<RunningManager<E, M>, IdleManager<E, M>>;
}

enum Manager<E, M: ThreadModel> {
    Running(RunningManager<E, M>),
    Idle(IdleManager<E, M>),
}
