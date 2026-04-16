use std::{future, marker::PhantomData, ops::{Deref, DerefMut}, task::{self, Poll, Waker}};
use std::future::poll_fn;
use std::task::Poll::{Ready, Pending};
use futures::task::AtomicWaker;
use tokio::task::AbortHandle;

use crate::downloader::{download_group::{DownloadGroup, GroupExt, GroupGuard, Reporter, ReporterGuard}, family::{RefCounted, ThreadModel}, httprequest::RequestInfo, segment::Segment};

async fn clone_waker() -> Waker{
    future::poll_fn(|c| task::Poll::Ready(c.waker().clone())).await
}

#[derive(Clone, Copy)]
struct Ext;
impl<F: ThreadModel> GroupExt<F> for Ext {
    type ShareExt<'a> = GroupShareExt<F>;
    type InLockExt<'a> = InLockShareExt;
    type SlotInlockExt<'a> = SlotExt;//end
    type SlotShareExt<'a> = SlotShareExt<F>;//remain
}

struct GroupShareExt<F: ThreadModel>{
    info: RequestInfo,
    process: F::AtomicCell<u64>
}
struct InLockShareExt{
    segments: Vec<Segment>,
    waker: Option<Waker>,
}

struct SlotExt{
    end: u64
}

struct SlotShareExt<F: ThreadModel>{
    remain: F::RefCounter<u64>,
    abort: AbortHandle,
}

///
struct AsyncGroup<F: ThreadModel>{
    group: DownloadGroup<'static, F, Ext>,
    length: u64
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

    fn lock(&self) -> AsyncGroupGuard<'_, F>{
        AsyncGroupGuard::new(self.group.lock())
    }
    
    fn join_all(&self) {
        self.lock().join_all();
    }

}

struct AsyncGroupGuard<'a, F: ThreadModel>{
    guard: GroupGuard<'a, 'static, F, Ext>,
}

impl<'a, F: ThreadModel> AsyncGroupGuard<'a, F> {

    fn new(guard: GroupGuard<'a, 'static, F, Ext>) -> Self{
        Self{guard}
    }

    async fn new_reporter(&mut self) -> Option<AsyncReporter<F>>{
        if self.guard.in_lock_ext().aborting{ return None;}
        let waker = clone_waker().await;
        let a = self.guard.new_reporter(0, <F::RefCounter<u64> as RefCounted>::new(0));
        AsyncReporter{reporter: a, waker}.into()
    }

    fn join_all(&mut self) -> impl Future {
        poll_fn(|c| {
            if self.guard.slots_mut().is_empty() {
                Ready(())
            } else {
                let a: &mut DownloadGroup<'_, F, Ext> = self.guard.group_mut();
                a.inner.waker.register(c.waker());
                Pending
            }
        })
    }

    fn set_waker(&mut self, waker: Waker) -> Waker{
        self.guard.
    }

    fn abort_all(&mut self) {// todo: 移动到
        //let slots = self.guard.slots_optional();

        for i in self.guard.slots().unwrap(){
            i.share().abort.abort();
        }
        self.guard.slots_optional().set_empty();
    }
}

struct DownloadWorker<'data, F: ThreadModel>{
    reporter: Reporter<'data, F, Ext>
}

impl<'data, F: ThreadModel> DownloadWorker<'data, F> {
    fn exit(&self) {
        let mut guard = self.reporter.lock();
        guard.remove_me();
        let waker = guard.inlock_ext().waker.take();
        //先释放再唤醒，避免竞争
        drop(guard);

        if let Some(waker) = waker{
            waker.wake();
        }
    }
}



async fn download<'data, F>(download_info: RequestInfo, reporter: Reporter<'data, F, Ext>, end: u64) 
where F: ThreadModel
{
    let a: &<Ext as GroupExt<F>>::SlotShareExt<'data> = &reporter.slot_share().ext;
    let process = (&a.remain)

}