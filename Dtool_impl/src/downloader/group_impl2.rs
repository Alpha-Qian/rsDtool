use std::

{
    future::poll_fn, io::IsTerminal, ops::{ControlFlow, Deref}, result, sync::atomic::Ordering, task::{Poll, Waker}
};

use radium::Radium;
use tokio::sync::mpsc::OwnedPermit;

use crate::downloader::{
    download_group::{BusyGroup, BusyReporter, DownloadGroup, GroupGuard, GroupParts, IdleGroup, IdleSlot, Reporter, ReporterBusy, ReporterGuard, Slot, SlotShare, State}, error::{Aborted, SubError, SuperError}, family::ThreadModel, group_impl::DownloadStream, httprequest::RequestInfo, pwriter::BufWriter, segment::{self, Segment}
};

struct AsyncParts;

impl<F: ThreadModel> GroupParts<F> for AsyncParts{
    type GroupShare<'a> = GroupShareData;

    type Data<'a> = ();
    type Result<'a> = IdleData;//运行结果
    type Waker<'a> = BusyData;//唤醒器
    type SlotData<'a> = SlotData;//结束位置

    type SlotShare<'a> = SlotShareData<F>;//进度，取消标志

    type Waker<'a> = Waker;//唤醒器
}

struct GroupShareData {
    info: RequestInfo,
}

struct BusyData {
    waker: Waker,
}

struct IdleData {
    result: Result<(), SuperError<(),()>>,
}

struct SlotData{
    end: u64,
}

struct SlotShareData<F: ThreadModel>{
    abort: F::AtomicCell<bool>,
    remain: F::AtomicCell<u64>,
}



struct Builder<I>{
    info: Option<RequestInfo>,
    segments: Option<I>,
}

impl<I> Builder<I>
where
    I: IntoIterator<Item = Segment>,
{
    async fn build(self) -> DownloadGroup<'static, F, AsyncParts> {

    }
}




impl<F> DownloadGroup<'static, F, AsyncParts>
where
    F: ThreadModel
{

    async fn async_with<T, A>(info: RequestInfo, f: A ) -> T
    where
        A: AsyncFnOnce(&mut Self) -> T
    {
        let this = Self::new_busy(
            GroupShareData{
                info,
            }, (),
            BusyData { waker: clone_waker().await }
        );

        let r = f().await;

        yield_until_wake().await;

        r
    }

    // ///
    // fn submit_waker(group: RequestInfo) -> impl Future<Output = Self> {
    //     poll_fn(|cx| {
    //         let waker = cx.waker();
    //         let group = Self::new_busy(group, (), BusyData { waker });
    //         Poll::Ready(group)
    //     })
    // }

    // async fn submit_waker(&mut self)




    async fn join_all(&mut self) -> Result<(), SuperError<impl DownloadStream, impl BufWriter>> {
        loop {
            let guard = self.lock();
            match guard.state_data() {
                State::Idle(s) => return s.data.result,
                State::Busy(s) => {
                    if s.slots().len() == 0{
                        return Ok(())
                    }
                    //应对虚假唤醒
                    yield_until_wake().await;
                }
            }
        }
    }

    fn emit_result<S, W>(&self, result: Result<(), SuperError<S, W>>)
    where
        S: DownloadStream,
        W: BufWriter,
    {
        let guard = self.lock();
        let State::Busy(mut busy_group) = guard.state() else {panic!()};
        let waker: Waker = busy_group.set_result(result);
        todo!("设置result");
        drop(guard);
        waker.wake();
    }
}

impl<'t, F> BusyGroup<'t, 'static, F, AsyncParts>
where
    F: ThreadModel,
{
    fn handle_result<S, W>(&self, result: Result<Result<(), SuperError<S, W>>, Aborted> {
        match result {
            Ok(Ok(())) => {
                if self.slots().len() == 1 {
                    self.emit_result(Ok(()));
                }
            },
            Ok(Err(error)) => {
                for i in self.slots().0 {
                    i.share.ext.abort.store(true, std::sync::atomic::Ordering::Relaxed);
                }

            },
            Err(_aborted) => {

                for i in self.slots_mut().0{
                    i.share.ext.abort.store(true, std::sync::atomic::Ordering::Relaxed);
                }
                busy.slots_mut().0.clear();
            }
        }
    }

}

impl<'t, F> BusyGroup<'t, 'static, F, AsyncParts>
where
    F: ThreadModel
{
    pub fn set_result<S, W>(&mut self, result: Result<(), SuperError<S, W>>) -> Waker{
        todo!()
    }


    fn new_segment(&mut self, segment: Segment) -> Reporter<'a, F, AsyncParts> {
        let slot_data = SlotData{
            end: segment.end(),
        };
        let slot_share = SlotShareData{
            abort: <F::AtomicCell<bool> as Radium>::new(false),
            remain: <F::AtomicCell<u64> as Radium>::new(segment.remain.get()),
        };
    }

    fn extend_from_iter<I>(&mut self, segment_iter: I) -> impl Iterator<Item = Reporter<'a, F, AsyncParts>>
    where
        I: IntoIterator<Item = Segment>
    {
        let iter = segment_iter.into_iter();

        let size_hit = self.slots().0.iter().size_hint();
        self.slots_mut().0.reserve(size_hit.0);
        iter.map(|segment| {
            self.new_segment(segment)
        })
    }
}



impl<'t, F> IdleGroup<'t, 'static, F, AsyncParts>
where
    F: ThreadModel,
{
    async fn submit_waker(self) -> BusyGroup<'t, 'a, F, AsyncParts> {
        let waker = clone_waker().await;
        todo!()
    }
}

impl<F> Reporter<'static, F, AsyncParts>
where
    F: ThreadModel,
{
    async fn execute<A, S, W>(&self, f: A) -> Result<T, SuperError<S, W>>
    where
        A: AsyncFnOnce(&Self) -> Result<Result<T, SuperError<S, W>, Aborted>>,
        S: DownloadStream,
        W: BufWriter,
    {
        let Ok(reuslt) = f(self).await else {
            //aborted
            //啥都不干
        };

    }



    }
}

impl<F> ReporterGuard<'static, F, AsyncParts>
where
    F: ThreadModel
{

    //仅在非abort退出
    fn super_done(&mut self, idle_data: ()) {
        let state = self.as_state();
        let busy: ReporterBusy<'a, F, AsyncParts> = match state {
            State::Idle(_) => panic!(""),
            State::Busy(b) => b
        };
        //取出waker

        //修改状态为Idle

        //释放锁

        //唤醒waker
    }

    fn done_error(&mut self) {
        //取消所有线程

        //调用super_done
        self.super_done();
    }

    fn done_success(&mut self) {
        //检查自己是否为最后一个线程

        //若是，调用super_done
        self.super_done();
    }

    fn abort_exit() {
        //啥都不干
    }

    ///为每个异步任务附加的上下文
    async fn execute(&mut self, f: impl AsyncFnOnce(&mut self) -> Result<Result<(), SuperError<>>, Aborted>) {
        let r = f(self).await;
        match r {
            Ok(Ok(())) => self.super_done(()),
            Ok(Err(e)) => self.done_error(),
            Err(_) => self.abort_exit(),
        }
    }

    ///异步轮询式执行，只在没主动让出时间中止
    async fn execute_unabortable(&mut self, f: impl AsyncFnMut(&mut Self) -> ControlFlow<Result<(), >>) {

    }
}







impl<F> Slot<'static, F, AsyncParts>
where
    F: ThreadModel
{
    unsafe fn from_semgent(segment: Segment, index: usize) -> Self {
        let (start, remain) = segment.into_raw();
        let end = start + remain.get();
        let share = RefCounter::new(SlotShare{
            index: SyncUnsafeCell::new(index),
            ext: SlotShareData{
                abort: F::AtomicCell::new(false),
                remain: F::AtomicCell::new(remain),
            },
        });
        Self {
            data: SlotData{
                end,
            },
            share,
        }
    }
}








trait Strategy<E: GroupParts<F>, F: ThreadModel>{
    //TODO

    //fn on_report_unlock(share: &mut E::)
}





fn yield_until_wake() -> impl Future<Output = ()> {
    let mut yielded = false;
    poll_fn(|cx| {
        if !yielded {
            yielded = true;
            return Poll::Pending;
        }
        Poll::Ready(())
    })
}

fn clone_waker() -> impl Future<Output = Waker> {
    poll_fn(|cx| {
        return Poll::Ready(cx.waker().clone())
    })
}


// struct OwnedGuard<'a, F: ThreadModel, P: GroupParts<F>> (DownloadGroup<'a, F, P>);

// impl<'a, F: ThreadModel, P: GroupParts<F>> OwnedGuard<'a, F, P> {
//     fn deref(&self) -> GroupGuard<'a, F, P>
// }




// fn deref_value<T, F>(value: T, f: F) -> {

// }

// struct Guard<T>(T);

// impl<T> Deref for Guard<T> {
//     type Target = T;
//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// impl
