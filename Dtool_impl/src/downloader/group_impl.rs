use std::{future, marker::PhantomData, ops::{Deref, DerefMut}, task::{self, Poll, Waker}};
use std::future::poll_fn;
use std::task::Poll::{Ready, Pending};
use futures::task::AtomicWaker;

use crate::downloader::{download_group::{DownloadGroup, GroupExt, GroupGuard, Reporter, ReporterGuard}, family::{RefCounted, ThreadModel}, segment::Segment};

async fn clone_waker() -> Waker{
    future::poll_fn(|c| task::Poll::Ready(c.waker().clone())).await
}
struct Ext;
impl<F: ThreadModel> GroupExt<F> for Ext {
    type GroupShareExt<'a> = GroupShareExt<F>;
    type InLockShareExt<'a> = InLockShareExt;
    type SlotExt<'a> = SlotExt;//end
    type SlotShareExt<'a> = SlotShareExt<F>;//remain
}

struct GroupShareExt<F: ThreadModel>{process: F::AtomicCell<u64>}
struct InLockShareExt{
    segments: Vec<Segment>,
    waker: Option<Waker>
}

struct SlotExt{end: u64}
struct SlotShareExt<F: ThreadModel>{remain: F::RefCounter<u64>}


struct AsyncGroup<F: ThreadModel>{
    pub raw: DownloadGroup<'static, F, Ext>,
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
        AsyncGroupGuard::new(self.raw.lock())
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

    async fn new_reporter(&mut self) -> AsyncReporter<F>{
        let waker = clone_waker().await;
        let a = self.guard.new_reporter(0, <F::RefCounter<u64> as RefCounted>::new(0));
        AsyncReporter{reporter: a, waker}
    }

    fn join_all(&mut self) -> impl Future {
        poll_fn(|c| {
            if self.guard.slots().is_empty() {
                Ready(())
            } else {
                let a: &mut DownloadGroup<'_, F, Ext> = self.guard.group();
                a.share.waker.register(c.waker());
                Pending
            }

        })
    }

    fn set_waker(&mut self, waker: Waker) -> Waker{
        self.guard.
    }
}


// struct AsyncReporter<F: ThreadModel> {
//     reporter: Reporter<'static, F, Ext>,
//     waker: Waker
// }

// impl<F: ThreadModel> AsyncReporter<F> {
//     fn new(reporter: Reporter<'static, F, Ext>, waker: Waker) -> Self{
//         Self { reporter, waker }
//     }

//     fn lock(&self) -> ReporterGuard<'_, 'static, F, Ext>{
//         self.reporter.lock()
//     }

//     fn on_exit(self) {
//         let mut reporter: ReporterGuard<'_, 'static, F, Ext> = self.lock();
//         reporter.remove_me();
        
//         if reporter.slots().is_empty() {self.reporter.share().waker.wake();}
//     }
// }

// // impl<F: ThreadModel> Drop for AsyncReporter<F> {
// //     fn drop(&mut self) {
// //         let mut reporter: ReporterGuard<'_, 'static, F, Ext> = self.lock();
// //         reporter.remove_me();
// //         if reporter.slots().is_empty() {self.waker.wake_by_ref();}
// //     }
// // }


// struct WakerExt<F, E>{
//     a: PhantomData<F>,
//     b: PhantomData<E>
// }

// impl<F: ThreadModel, E: GroupExt<F>> GroupExt<F> for WakerExt<F, E> {
//     type GroupShareExt<'a> = E::GroupShareExt<'a>;
//     type InLockShareExt<'a> = WakerWith<E::InLockShareExt<'a>>;
//     type SlotExt<'a> = E::SlotExt<'a>;
//     type SlotShareExt<'a> = E::SlotShareExt<'a>;


// }

// struct WakerWith<T>{
//     waker: Option<Waker>,
//     data: T
// }

// impl<T> WakerWith<T> {
//     pub fn replace(&mut self, waker: Waker) -> Option<Waker>{
//         self.waker.replace(waker)
//     }

//     pub fn take(&mut self) -> Option<Waker>{
//         self.waker.take()
//     }
// }

// impl<T> Deref for WakerWith<T> {
//     type Target = T;
//     fn deref(&self) -> &Self::Target {
//         &self.data
//     }
// }

// impl<T> DerefMut for WakerWith<T> {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.data
//     }
// }

// struct ParkExt<F, E>{
//     a: PhantomData<F>,
//     b: PhantomData<E>
// }

// use std::thread::Thread;
// impl<F: ThreadModel, E: GroupExt<F>> GroupExt<F> for ParkExt<F, E> {
//     type InLockShareExt<'b> = ;
// }


// struct CanWake<W, T>{
//     waker: Option<W>,
//     data: T
// }

// impl<W: Wake, T> CanWake<W, T> {
//     pub fn replace_waker(&mut self, new: Option<W>) -> Option<W> {
//         std::mem::replace(&mut self.waker, new)
//     }
// }

// trait Wake{
//     fn wake(self);
// }