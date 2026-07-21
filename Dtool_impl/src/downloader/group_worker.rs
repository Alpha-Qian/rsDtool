//!每个线程的执行者

use std::ops::Deref;
use std::{error::Error, ops::ControlFlow, sync::atomic::Ordering};

use radium::Radium;

use crate::base::group_construct::BusyReporter;
use crate::base::{
    family::ThreadModel,
    group_construct::{Reporter, State},
    segment::Segment,
};
use crate::downloader::group_async_parts::{AsyncParts, GroupShare, Reporter2, Residual};
use crate::downloader::group_download_methold::{
    DownloadContext, Downloader, RawDownloadUnInjected,
};

struct DownloadSegmentError {
    segment: Segment,
}

///Woker只有一个典型的状态，所以省略busy
pub struct SegmentWorker<E, M: ThreadModel> {
    reporter: Reporter<'static, M, AsyncParts<E>>,
    share: M::RefCounter<GroupShare<M>>,
    progress: u64,
}

impl<M: ThreadModel, E> SegmentWorker<E, M> {
    fn new(reporter: Reporter2<E, M>, share: M::RefCounter<GroupShare<M>>, progress: u64) -> Self {
        Self {
            reporter,
            share,
            progress,
        }
    }

    fn aborted(&mut self) -> bool {
        self.share.abort_single.load(Ordering::Relaxed)
    }

    ///发生关键错误
    /// abort all and exit
    fn on_unresumable_error(self, error: impl Error) {
        if self.share.abort_single.swap(true, Ordering::Relaxed) {
            return;
        }

        let guard = self.reporter.lock();
        match guard.state() {
            State::Running(busy) => {
                let segments = self.reporter;
                let error_segment = todo!();

                let residual = Residual::<E> {
                    error: None,
                    segments,
                    error_segment,
                };
                let idle = busy.into_idle(todo!());
                return;
            }
            State::Idle(idle) => {
                return;
            }
        }
    }

    /// 在单个Segment下载完成时运行
    /// Break -> 退出
    /// Continue -> 继续用新的Reporter下载
    fn on_segment_downloaded_ok(mut self) -> ControlFlow<(), Self> {
        if self.aborted() {
            return ControlFlow::Break(());
        }

        let reporter = self.reporter;

        let guard = reporter.lock();
        match guard.state() {
            State::Running(mut busy) => {
                if self.aborted() {
                    return ControlFlow::Break(());
                }

                if busy.busy_data().lazy_cancel_count != 0 {
                    busy.busy_data_mut().lazy_cancel_count -= 0;
                    return ControlFlow::Break(());
                };
                let index = *busy.index();
                let _my_slot = busy.slots_mut().swap_remove_and_update_index(index);

                let reporter = todo!("");

                todo!("任务窃取，创建新Reporter并返回");
                return ControlFlow::Continue(todo!());
            }
            State::Idle(idle) => return ControlFlow::Break(()),
        }
    }

    async fn working<D: RawDownloadUnInjected<Error = E>>(self) {
        working_example(self).await
    }

    fn id(&self) -> usize {
        self.share.deref() as *const _ as us
    }

    async fn inject_self(mut self, downloader: impl Downloader<Self>) {
        let result = downloader.download(self).await;
    }
}

async fn working_example<E, M: ThreadModel, D: RawDownloadUnInjected<Error = E>>(
    mut worker: SegmentWorker<E, M>,
    downloader: impl Downloader<SegmentWorker<E, M>>,
) {
    //segment loop
    loop {
        let result = D::download_segment(&mut worker).await;
        match result {
            Ok(ControlFlow::Continue(())) => {
                //分段完成

                //尝试任务窃取
                let ControlFlow::Continue(w) = worker.on_segment_downloaded_ok() else {
                    return;
                };
                worker = w;
            }
            Ok(ControlFlow::Break(())) => {
                //被取消
                return;
            }
            Err(e) => {
                worker.on_unresumable_error(e);
            }
        }
    }
}

impl<E, M: ThreadModel> DownloadContext for SegmentWorker<E, M> {
    fn reporter_downloaded(&self, length: usize) -> i64 {
        self.reporter
            .slot_share
            .ext
            .remain
            .fetch_sub(length as u64, Ordering::Release) as i64
    }

    fn is_aborted(&self) -> bool {
        self.share.abort_single.load(Ordering::Relaxed)
    }
}
