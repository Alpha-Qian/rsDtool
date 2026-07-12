//!每个线程的执行者

use std::{error::Error, ops::ControlFlow, sync::atomic::Ordering};

use radium::Radium;

use crate::base::{
    family::ThreadModel,
    group_async_parts::AsyncParts,
    group_construct::{Reporter, State},
    segment::Segment,
};

struct DownloadSegmentError {
    segment: Segment,
}

struct Worker<M: ThreadModel> {
    reporter: Reporter<'static, M, AsyncParts>,

    //group decard
    abort_now: M::RefCounter<M::AtomicCell<bool>>,
}

impl<M: ThreadModel> Worker<M> {
    ///发生关键错误
    /// abort all and exit
    fn on_unresumable_error(self, error: impl Error) {
        let aborted = self.abort_now.swap(true, Ordering::Relaxed);
        if !aborted {
            let guard = self.reporter.lock();
            match guard.state() {
                State::Busy(busy) => {
                    todo!("状态转为idle")
                }
                State::Idle(idle) => {
                    return;
                }
            }
        }
        // if self.0.slot().abort.load(Ordering::Relaxed) {
        //     //被取消时会有人替我们处理数据
        //     return;
        // }

        // let guard = self.0.lock();
        // match guard.state() {
        //     State::Busy(busy) => {
        //         for i in busy.slots().0 {
        //             i.share.ext.abort.store(true, Ordering::Relaxed);
        //         }

        //         todo!("设置错误残留值");
        //         return;
        //     }
        //     State::Idle(idle) => return,
        // }
    }

    /// 在单个Segment下载完成时运行
    /// Break -> 退出
    /// Continue -> 继续用新的Reporter下载
    fn on_segment_downloaded_ok(self) -> ControlFlow<(), Self> {
        let reporter = self.0;

        if reporter.slot().abort.load(Ordering::Relaxed) {
            //快路径
            return ControlFlow::Break(());
        }

        let guard = reporter.lock();
        match guard.state() {
            State::Busy(busy) => {
                if *busy.busy_data().lazy_cancel_count != 0 {
                    *busy.busy_data_mut() -= 0;
                    return ControlFlow::Break(());
                }
                todo!("删除当前slot");
                todo!("任务窃取，创建新Reporter并返回")
            }
            State::Idle(idle) => return ControlFlow::Break(()),
        }
    }
}
