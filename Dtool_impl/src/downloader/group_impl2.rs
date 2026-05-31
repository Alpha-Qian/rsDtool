use std::

{
    future::poll_fn, io::IsTerminal, ops::ControlFlow, task::{Poll, Waker}
};

use crate::downloader::{
    download_group::{BusyGroup, DownloadGroup, GroupParts, IdleSlot, Reporter, ReporterBusy, ReporterGuard, Slot, SlotShare}, error::{Aborted, SubError, SuperError}, family::ThreadModel, group_impl::DownloadStream, httprequest::RequestInfo, pwriter::BufWriter, segment::{self, Segment}
};

struct AsyncParts;

impl GroupParts<F> for AsyncParts<C> {
    type GroupShare<'a> = GroupShareData;

    type Data<'a> = ();
    type IdleData<'a> = IdleData;
    type BusyData<'a> = BusyData;
    type SlotData<'a> = SlotData;

    type SlotShare<'a> = SlotShareData;

    type Waker<'a> = Waker;
}

struct GroupShareData {
    info: RequestInfo,
}

struct BusyData {
    waker: Waker,
}

struct IdleData {
    result: Result<(), SuperError>,
}

struct SlotData{
    end: u64,
}

struct SlotShareData<F: ThreadModel>{
    abort: F::AtomicCell<bool>,
    remain: F::AtomicCell<u64>,
}

// ///Builder
// struct NameSpaced;

// impl NameSpaced {
//     async fn async_with(){todo!()}


// }






impl<'a, F: ThreadModel> DownloadGroup<'a, F, AsyncParts> {
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

    fn submit_waker(group: GroupShareData) -> impl Future<Output = Self> {
        poll_fn(|cx| {
            let waker = cx.waker();
            let group = Self::new_busy(group, (), BusyData { waker });
            Poll::Ready(group)
        })
    }


    async fn join_all(&mut self) -> Result<(), SuperError<impl DownloadStream, impl BufWriter>> {
        //get lock
        //

    }




}

impl<'a, F:ThreadModel> Reporter<'a, F, AsyncParts> {
    async fn execute<A, S, W>(&self, f: A) -> Result<T, SuperError<S, W>>
    where
        A: AsyncFnOnce(&Self) -> Result<Result<T, SuperError<S, W>, Aborted>>,
        S: DownloadStream,
        W: BufWriter,
    {
        let result = f(self);

    }
}

impl<'a, F: ThreadModel> ReporterGuard<'a, F, AsyncParts> {

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


impl<'a, F: ThreadModel> ReporterBusy<'a, F, AsyncParts> {

    fn new_segment(&mut self, segment: Segment) -> Reporter<'a, F, AsyncParts> {
        let (start, remain) = segment.into_raw();
        let end = start + remain.get();
    }

    ///emit the result
    fn emit_result<S, W>(self, result: Result<(), SuperError<S, W>>)
    where
        S: DownloadStream,
        W: BufWriter,
    {
        let (iter_reporterm, busy)= self.swap_state(IdleSlot{ data: IdleData{ result}} );
        let (slots, data) = busy.into_raw();
        slots.into_raw().iter().for_each(|| { todo!("abort")});

        todo!("wake waker here")
    }

}


impl<'a, F> BusyGroup<'a, F, AsyncParts>
where
    F: ThreadModel
{
    fn new_segment(&mut self, segment: Segment) -> Reporter<'a, F, AsyncParts> {
        let slot_data = SlotData{
            end: segment.end(),
        };
        let slot_share = SlotShareData{
            abort: F::AtomicCell::<bool>::new(false),
            remain: F::AtomicCell::<u64>::new(segment.remain.get()),
        };
        self.new_reporter(slot_ext, slot_inlock)
    }

    ///
    fn load_segments<I>(&mut self, segment_iter: I) -> LoadSegments<'_, 'a, I::IntoIter, F>
    where
        I: IntoIterator<Item = Segment>
    {
        let iter = segment_iter.into_iter();

        let size_hit = self.slots().0.iter().size_hint();
        self.slots_mut().0.reserve(size_hit.0);
        LoadSegments::new(self, segments)
    }


    fn load_segments2<I>(&mut self, segment_iter: I) -> impl Iterator<Item = Reporter<'a, F, AsyncParts>>
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




struct LoadSegments<'a, 'b, I, F: ThreadModel>{
    busy: &'a mut BusyGroup<'b, F, AsyncParts>,
    segments: I
}

impl<'a, 'b, I, F> LoadSegments<'a, 'b, I, F>
where
    I: Iterator<Item = Segment>,
    F: ThreadModel,
{
    fn new(busy: &'a mut BusyGroup<'b, F, E>, segments: I) -> Self {
        Self { busy, segments }
    }

    fn test(busy: &'a mut BusyGroup<'b, F, E>, iter: T) -> Self
    where
        T: IntoIterator<IntoIter = I>
    {
        Self::new(busy, iter.into_iter())
    }
}

impl<'a, 'b, I, F> Iterator for LoadSegments<'a,'b, I, F>
where
    F: ThreadModel,
    I: Iterator<Item = Segment>,
{

    type Item = Reporter<'a, F, E>;

    fn next(&mut self) -> Option<Self::Item> {
        let segment = self.segments.next()?;
        Some(self.busy.new_segment(segment))
    }
}





impl<'a, F> Slot<'a, F, AsyncParts> {
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








trait Strategy<E: GroupParts>{
    //TODO

    //fn on_report_unlock(share: &mut E::)
}





fn yield_until_wake() -> impl Future<Output = ()> {
    yielded = false;
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
impl<'a, C: FnOnce(), F: ThreadModel> DownloadGroup<'a, F, AsyncParts<C>>
