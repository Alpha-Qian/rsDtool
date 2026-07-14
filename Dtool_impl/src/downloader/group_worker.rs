//!每个线程的执行者

use std::{error::Error, ops::ControlFlow, sync::atomic::Ordering};

use futures::TryStream;
use radium::Radium;

use crate::downloader::group_async_parts::AsyncParts;
use crate::{
    base::{
        family::ThreadModel,
        group_construct::{Reporter, State},
        pwriter::BufWriter,
        segment::Segment,
    },
    downloader::group_download_methold::DownloadMethod,
};

struct DownloadSegmentError {
    segment: Segment,
}

pub struct SegmentWorker<M: ThreadModel, D: DownloadMethod> {
    reporter: Reporter<'static, M, AsyncParts<D>>,

    //group decard
    abort_now: M::RefCounter<M::AtomicCell<bool>>,
}

impl<M: ThreadModel, D: DownloadMethod> SegmentWorker<M, D> {
    ///发生关键错误
    /// abort all and exit
    fn on_unresumable_error(self, error: impl Error) {
        let aborted = self.abort_now.swap(true, Ordering::Relaxed);

        if !aborted {
            let guard = self.reporter.lock();
            match guard.state() {
                State::Busy(busy) => {
                    let idle = busy.into_idle(todo!());
                    return;
                }
                State::Idle(idle) => {
                    return;
                }
            }
        }
    }

    /// 在单个Segment下载完成时运行
    /// Break -> 退出
    /// Continue -> 继续用新的Reporter下载
    fn on_segment_downloaded_ok(self) -> ControlFlow<(), Self> {
        let reporter = self.reporter;

        if self.abort_now.load(Ordering::Relaxed) {
            return ControlFlow::Break(());
        }

        let guard = reporter.lock();
        match guard.state() {
            State::Busy(busy) => {
                if *busy.busy_data().lazy_cancel_count != 0 {
                    *busy.busy_data_mut() -= 0;
                    return ControlFlow::Break(());
                };
                let index = *busy.index();
                let _my_slot = busy.slots_mut().swap_remove_and_update_index(index);

                let reporter = todo!();

                todo!("任务窃取，创建新Reporter并返回");
                return ControlFlow::Continue(todo!());
            }
            State::Idle(idle) => return ControlFlow::Break(()),
        }
    }
}

async fn working_example<D: DownloadMethod, M: ThreadModel>(mut worker: SegmentWorker<M, D>) {
    //segment loop
    loop {
        let result = D::download_segment(&mut worker).await;
        match result {
            Ok(ControlFlow::Continue(())) => {
                //分段完成

                //尝试任务窃取
                let ControlFlow::Continue(worker) = worker.on_segment_downloaded_ok() else {
                    return;
                };
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
