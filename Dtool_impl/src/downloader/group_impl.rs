use std::{future, task::{self, Waker}};
use std::future::poll_fn;
use std::task::Poll::{Ready, Pending};
use crate::downloader::{download_group::{DownloadGroup, GroupExt, GroupGuard, Reporter}, family::{RefCounted, ThreadModel}, segment::Segment};

async fn clone_waker() -> Waker{
    future::poll_fn(|c| task::Poll::Ready(c.waker().clone())).await
}
struct Ext;
impl<F: ThreadModel> GroupExt<F> for Ext {
    type SlotShareExt<'a> = F::RefCounter<u64>;
    type SlotExt<'b> = u64;//end
    type InLockShareExt<'c> = Vec<Segment>;
    type GroupShareExt<'d> = ();
}

struct AsyncGroup<F: ThreadModel>{
    pub raw: DownloadGroup<'static, F, Ext>,
    //waker: Waker
}




impl<F: ThreadModel> AsyncGroup<F> {
    // async fn new(raw: DownloadGroup<'static, F, Ext>) -> Self{
    //     let waker = clone_waker().await;
    //     Self{raw, waker}
    // }

    async fn new_reporter() {
        let waker = clone_waker().await;
        todo!()
    }
}

struct AsyncGroupGuard<F: ThreadModel>{
    guard: GroupGuard<'static, F, Ext>,
}

impl<F: ThreadModel> AsyncGroupGuard<F> {
    async fn new_reporter(&mut self) -> AsyncReporter<F>{
        let waker = clone_waker().await;
        let a = self.guard.new_reporter(0, <F::RefCounter<u64> as RefCounted>::new(0));
        AsyncReporter{reporter: a, waker}
    }

    fn join_all(&mut self) -> impl Future{
        poll_fn(|c|{
            if self.guard.slots().is_empty() { Ready(())} else { Pending }
        })
    }
}


struct AsyncReporter<F: ThreadModel>{
    reporter: Reporter<'static, F, Ext>,
    waker: Waker
}
