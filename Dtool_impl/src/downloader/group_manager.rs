//!组下载管理器

use std::{
    error::Error,
    mem::MaybeUninit,
    sync::atomic::{AtomicU64, Ordering},
    task::Waker,
};

use futures::task::noop_waker;
use radium::Radium;
use reqwest::blocking::Client;

use crate::{
    base::{
        family::ThreadModel,
        group_construct::{DownloadGroup, GroupParts, Reporter, State},
        request_info::RequestInfo,
        segment::Segment,
    },
    downloader::group_download_methold::DownloadMethod,
};

use crate::downloader::group_async_parts::{AsyncParts, IdleData, RunningData};
// 状态
// 初始：管理器空闲，下载组空闲 -创建下载-> 下载中；
// 下载中：管理器运行中，下载器繁忙 -下载完成-> 完成；
// 完成：管理器运行中，下载器空闲 -管理器收到waker消息-> 初始；
//
// 未指定：管理器空闲，下载器繁忙

#[repr(transparent)]
struct RunningManager<M, D>(DownloadGroup<'static, M, AsyncParts<D>>)
where
    M: ThreadModel,
    D: DownloadMethod;

impl<M, D> RunningManager<M, D>
where
    M: ThreadModel,
    D: DownloadMethod,
{
    // ///创建一个已经完成但还没被join的RunningManager
    // fn new_unjoined(info: RequestInfo) -> Self {
    //     let busy_data = new_busy_data(info);
    //     let group = DownloadGroup::new_busy((), busy_data);

    //     Self(group)
    // }

    // // fn check_done(&self) -> (Option<impl Error>, &IdleManager<M>) {
    // //     let guard = self.0.lock();
    // fn new_with_waker(info: RequestInfo, waker: Waker) -> Self {
    //     todo!()
    // }

    fn new<I: Iterator<Item = Segment>>(
        info: RequestInfo,
        mut segments: I,
        waker: Option<Waker>,
    ) -> Self {
        let Some(segment) = segments.next() else {
            let data = IdleData { error_info: None };
            let group = DownloadGroup::new_idle((), data);
            //虽然返回的是RunningLoop，但实际上已经设置为完成状态了
            return Self(group);
        };
        let waker = waker.unwrap_or(noop_waker());
        todo!()
    }

    fn try_set_waker(self, waker: Waker) -> Result<(), (IdleManager<M, D>, Option<impl Error>)> {
        let guard = self.0.lock();
        match guard.state() {
            State::Busy(busy) => {
                *busy.busy_data_mut().waker = waker;
                return Ok(());
            }
            State::Idle(idle) => {
                let result = idle.idle_data_mut().error_info.take();
                let idle_manager = IdleManager(self.0);
                return Err(todo!());
            }
        }
    }

    fn try_take_result(self) -> Result<(IdleManager<M, D>, Option<impl Error>), Self> {
        let guard = self.0.lock();
        match guard.state() {
            State::Busy(_) => {
                return Err(self);
            }
            State::Idle(idle) => {
                let result = idle.idle_data_mut().error_info.take();
                let idle_manager = IdleManager(self.0);
                return Ok(todo!());
            }
        }
    }
    // // }
    // async fn join_all(self) -> (Option<impl Error>, IdleManager<M>) {
    //     let group = self.0;
    //     let guard = group.lock();
    //     match guard.state() {
    //         State::Busy(busy) => {
    //             busy.busy_data_mut();
    //             todo!("设置Waker并等待唤醒")
    //         }
    //         State::Idle(idle) => {
    //             let data = idle.idle_data_mut();
    //             let error = data.error_info.take();
    //         }
    //     }
    // }
    fn abort_all(&self) {
        let guard = self.0.lock();
        match guard.state() {
            State::Busy(busy) => {
                for i in busy.slots().0 {
                    i.share.ext.abort.store(true, Ordering::Relaxed);
                }
            }
            State::Idle(idle) => {
                //fast path
                return todo!();
            }
        };
    }

    fn save_and_abort(&self) -> Option<()> {
        let guard = self.0.lock();
        match guard.state() {
            State::Busy(busy) => {}
        }
    }

    // async fn shutdown(self) -> (Option<impl Error>, IdleManager<M>) {
    //     let guard = self.0.lock();
    //     match guard.state() {
    //         State::Busy(busy) => {
    //             for i in busy.slots().0 {
    //                 i.share.ext.abort.store(true, Ordering::Relaxed);
    //             }
    //         }
    //         State::Idle(idle) => {
    //             //fast path
    //             return todo!();
    //         }
    //     };
    //     drop(guard);

    //     self.join_all().await
    // }

    fn get_resume_info(&self) -> Result<impl Iterator<Item = Segment>, impl Error> {
        let guard = self.0.lock();
        match guard.state() {
            State::Busy(busy) => {}
            State::Idle(idle) => return Ok(Vec![]),
        }
    }

    fn add_thread(client: Client) {
        todo!()
    }

    fn sub_thread_lazy() {
        todo!()
    }

    fn sub_thread() {
        todo!()
    }
}

///主要进行下载任务的初始化
#[repr(transparent)]
struct IdleManager<M, D>(DownloadGroup<'static, M, AsyncParts<D>>)
where
    M: ThreadModel,
    D: DownloadMethod;

impl<M, D> IdleManager<M, D>
where
    M: ThreadModel,
    D: DownloadMethod,
{
    fn new() -> Self {
        let idle = IdleData { error_info: None };
        let group = DownloadGroup::new_idle((), idle);
        Self(group)
    }

    fn run_from_raw<I: Iterator<Item = Segment>>(
        self,
        info: RequestInfo,
        mut segments: I,
    ) -> RunningManager<M> {
        //let running = RunningManager(self.0);
        let guard = self.0.lock();
        let Some(first) = segments.next() else {
            return RunningManager(self.0);
        };
        todo!()
    }

    fn from_sniffing_response(self) -> RunningManager<M> {
        todo!()
    }

    fn run_first_response(self, info: RequestInfo, client: Client) -> RunningManager<M> {
        todo!()
    }

    fn into_done_running_manager(self) -> RunningManager<M> {
        todo!()
    }
}

//struct Executer<M: ThreadModel>(reporter: Reporter<'static, M, AsyncParts>);

struct GroupResume<I> {
    info: RequestInfo,
    segments: I,
}

///创建一个已经完成的BusyData
fn new_busy_data<M: ThreadModel>(
    info: RequestInfo,
) -> <AsyncParts as GroupParts<M>>::Data<'static> {
    let running_data = RunningData {
        waker: noop_waker(),
        info,
        lazy_cancel_count: 0,
    };
    running_data
}

trait ShareData {
    fn ref_init(&self);
}
