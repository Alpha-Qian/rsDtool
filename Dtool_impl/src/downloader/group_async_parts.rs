use std::{
    error::Error,
    future::poll_fn,
    marker::PhantomData,
    mem::swap,
    process::Output,
    result,
    sync::atomic::Ordering,
    task::{Poll, Waker},
};

use radium::Radium;

use crate::{
    base::{
        base_error::{Aborted, MayAbort, RawError, SuperError},
        family::ThreadModel,
        group_construct::{
            BusyGroup, DownloadGroup, GroupParts, IdleGroup, IdleReporter, Reporter, ReporterBusy,
            ReporterGuard, Slot, SlotShare, State,
        },
        pwriter::BufWriter,
        request_info::RequestInfo,
        segment::Segment,
        subcontext::RemainWriter,
    },
    downloader::group_download_methold::DownloadMethod,
};

pub struct AsyncParts<D>(PhantomData<D>);

impl<F: ThreadModel, D: DownloadMethod> GroupParts<F> for AsyncParts<D> {
    type StaticData<'a> = GroupShareData<F>;

    type Result<'a> = Option<Residual<D>>; //运行结果
    type Data<'a> = RunningData; //唤醒器
    type SlotData<'a> = SlotData; //结束位置

    type SlotShare<'a> = SlotShareData<F>; //进度，取消标志
}

struct GroupShareData<F: ThreadModel> {
    info: RequestInfo,
    progress: F::AtomicCell<u64>,
}

pub struct RunningData {
    pub(crate) waker: Waker,
    pub(crate) info: RequestInfo,

    // 延迟结束线程
    // 在创建新线程时，将此值-1就不用实际创建线程了
    pub(crate) lazy_cancel_count: usize,
}

// ///残留值
// pub struct IdleData {
//     pub error_info: Option<(SuperError<(),()>, Vec<Segment>)>,
// }

///上次运行失败的残留值
struct Residual<D: DownloadMethod> {
    error: D::Error,
    error_segment: Segment,
    segments: Vec<Segment>,
}

struct SlotData {
    end: u64,
}

struct SlotShareData<F: ThreadModel> {
    pub(crate) abort: F::AtomicCell<bool>,
    pub(crate) remain: F::AtomicCell<u64>,
}

// impl<F, D: DownloadMethod> DownloadGroup<'static, F, AsyncParts<D>>
// where
//     F: ThreadModel
// {

//     ///等待直到任务完成，连续调用多次join_all只会在第一次返回一次error，接着返回Ok(())
//     async fn join_all(&mut self) -> Result<(), SuperError<impl DownloadStream, impl BufWriter>> {
//         loop {
//             let guard = self.lock();
//             match guard.state_data() {
//                 State::Idle(s) => {
//                     let result = Ok(());
//                     swap(&mut s.data.error_info, &mut result);
//                     return result
//                 },
//                 State::Busy(s) => {
//                     if s.slots().len() == 0{
//                         return Ok(())
//                     }
//                     drop(guard);

//                     //应对虚假唤醒
//                     yield_until_wake().await;
//                 }
//             }
//         }
//     }

//     ///设置结果并发送通知
//     /// TODO:移动到BusyGroup
//     fn emit_result<S, W>(&self, result: Result<(), SuperError<S, W>>)
//     where
//         S: DownloadStream,
//         W: BufWriter,
//     {
//         let guard = self.lock();
//         let State::Busy(mut busy_group) = guard.state() else {panic!()};
//         let waker: Waker = busy_group.set_result(result);
//         todo!("设置result");
//         drop(guard);
//         waker.wake();
//     }

//     ///结束所有任务，在其他线程因为错误而结束时也会调用此方法
//     fn super_abort(&mut self, error: Option<impl Error>) {
//         todo!()
//     }
//     //fn super_run<I: IntoIterator<Item = (&mut self, iter: I)
// }

// impl<'t, F, D: DownloadMethod> BusyGroup<'t, 'static, F, AsyncParts<D>>
// where
//     F: ThreadModel
// {

//     ///根据下载结果处理任务组状态
//     fn handle_result<S, W>(&self, result: Result<Result<(), SuperError<S, W>>, Aborted>) {
//         match result {
//             Ok(Ok(())) => {
//                 if self.slots().len() == 1 {
//                     self.emit_result(Ok(()));
//                 }
//             },
//             Ok(Err(error)) => {
//                 for i in self.slots().0 {
//                     i.share.ext.abort.store(true, std::sync::atomic::Ordering::Relaxed);
//                 }

//             },
//             Err(_aborted) => {

//                 for i in self.slots_mut().0{
//                     i.share.ext.abort.store(true, std::sync::atomic::Ordering::Relaxed);
//                 }
//                 busy.slots_mut().0.clear();
//             }
//         }
//     }
//     ///从分段创建reporter
//     fn extend_segment(&mut self, segment: Segment) -> Reporter<'a, F, AsyncParts> {
//         let slot_data = SlotData{
//             end: segment.end(),
//         };
//         let slot_share = SlotShareData{
//             abort: <F::AtomicCell<bool> as Radium>::new(false),
//             remain: <F::AtomicCell<u64> as Radium>::new(segment.remain.get()),
//         };

//         todo!()
//     }

//     ///从分段迭代器创建reporter
//     fn extend_from_iter<I>(&mut self, segment_iter: I) -> impl Iterator<Item = Reporter<'static, F, AsyncParts>>
//     where
//         I: IntoIterator<Item = Segment>
//     {
//         let iter = segment_iter.into_iter();

//         let size_hit = self.slots().0.iter().size_hint();
//         self.slots_mut().0.reserve(size_hit.0);
//         iter.map(|segment| {
//             self.extend_segment(segment)
//         })
//     }

//     ///查看每一个插槽下载进度，无序
//     fn inspect_slots(&self, f: impl FnMut(&Slot<'static, F, AsyncParts>)) {
//         self.slots().0.iter().for_each(f);
//     }

//     ///生成往remain writer写入stream的任务
//     fn spawn_task(&self, stream: Option<impl DownloadStream>, remain_writer: RemainWriter<'_, impl BufWriter, F>) -> impl Future {
//         todo!()
//     }
// }

// impl<'t, F, D: DownloadMethod> IdleGroup<'t, 'static, F, AsyncParts<D>>
// where
//     F: ThreadModel,
// {

//     ///注册waker并转换至BusyGroup
//     async fn submit_waker(self) -> BusyGroup<'t, 'static, F, AsyncParts> {
//         let waker = clone_waker().await;
//         todo!()
//     }
// }

// impl<M, D: DownloadMethod> Reporter<'static, M, AsyncParts<D>>
// where
//     M: ThreadModel,
// {

//     ///作为reporter执行
//     async fn execute<F, S, W>(&self, task: F)
//     where
//         F: AsyncFnMut(<AsyncParts as GroupParts<M>>::StaticData, <AsyncParts as GroupParts<M>>::SlotShare) -> Result<Result<(), RawError<S, W>>>,
//         S: DownloadStream,
//         W: BufWriter,
//     {
//         todo!()
//     }

//     ///处理结果
//     #[deprecated]
//     fn handle_result<S, W>(self, result: Result<Result<(), SuperError<S, W>>, Aborted>) -> DownloadGroup<'a, M, P> {
//         match result {
//             //成功完成
//             Ok(Ok(())) => todo!("移除my_slots"),
//             //出现不可重试错误
//             Ok(Err(e)) => todo!("执行所有线程的数据清理"),
//             //因为其他协程出现不可重试错误而被取消
//             Err(_aborted) => todo!("直接退出")
//         }
//     }
// }

// impl<'t, F, D: DownloadMethod> ReporterGuard<'t, 'static, F, AsyncParts<D>>
// where
//     F: ThreadModel
// {

//     //仅在非abort退出
//     fn super_done(&mut self, idle_data: ()) {
//         let state = self.as_state();
//         let busy: ReporterBusy<'a, F, AsyncParts> = match state {
//             State::Idle(_) => panic!(""),
//             State::Busy(b) => b
//         };
//         //取出waker

//         //修改状态为Idle

//         //释放锁

//         //唤醒waker
//     }

//     ///为每个异步任务附加的上下文
//     async fn execute(&mut self, f: impl AsyncFnOnce(&mut Self) -> Result<Result<(), SuperError<>>, Aborted>) {
//         let r = f(self).await;
//         match r {
//             Ok(Ok(())) => self.super_done(()),
//             Ok(Err(e)) => self.done_error(),
//             Err(_) => self.abort_exit(),
//         }
//     }

//     ///如果任务已经被取消，返回None
//     fn check_state<M: ThreadModel>(self) -> Option<ReporterBusy>{
//         let aborted: bool = todo!();
//         if aborted {
//             None
//         } else {
//             match self.state() {
//                 State::Busy(b) => return Some(b),
//                 _ => panic!("task group is idle but reporter has not been canceled")
//             }
//         }
//     }
// }

// impl<'t, M: ThreadModel, D: DownloadMethod> ReporterBusy<'t, 'static, M, AsyncParts<D>>
// where
//     M: ThreadModel,
// {

//     ///处理非取消下载结果
//     fn result(&mut self, error: Result<(), SuperError<impl DownloadStream, impl BufWriter>>, save: impl FnOnce(dyn Iterator<Item = Segment>) -> T) -> Option<T>{
//         match result {
//             Ok(()) => {
//                 // 在任务列表中删除当前任务
//                 // 如果剩余任务为0，唤醒waker

//                 None
//             }
//             Err(e) => {

//                 // 取消其他所有任务
//                 // 删除所有任务
//                 // 保存最后分段
//                 Some(todo!())
//             }
//         }
//     }
// }

// impl<'t, M: ThreadModel, D: DownloadMethod> IdleReporter<'t, 'static, M, AsyncParts<D>>
// where
//     M: ThreadModel
// {

// }

// impl<F> Slot<'static, F, AsyncParts>
// where
//     F: ThreadModel
// {
//     unsafe fn from_semgent(segment: Segment, index: usize) -> Self {
//         let (start, remain) = segment.into_raw();
//         let end = start + remain.get();
//         let share = RefCounter::new(SlotShare{
//             index: SyncUnsafeCell::new(index),
//             ext: SlotShareData{
//                 abort: F::AtomicCell::new(false),
//                 remain: F::AtomicCell::new(remain),
//             },
//         });
//         Self {
//             data: SlotData{
//                 end,
//             },
//             share,
//         }
//     }
// }

fn yield_until_wake() -> impl Future<Output = ()> {
    let mut yielded = false;
    poll_fn(move |cx| {
        if !yielded {
            yielded = true;
            return Poll::Pending;
        }
        Poll::Ready(())
    })
}

fn clone_waker() -> impl Future<Output = Waker> {
    poll_fn(|cx| return Poll::Ready(cx.waker().clone()))
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

// async fn download_normal<F: ThreadModel>(
//     group: <AsyncParts as GroupParts<F>>::GroupShare,
//     slot: <AsyncParts as GroupParts<F>>::SlotShare,
//     end: usize,
// ) -> Result<Result<(), RawError<S, W>>>
// {
//     let info = group.info.clone();
//     let
// }
//

async fn retry_loop<S: DownloadStream>(
    reporter: &Reporter<'static, F, AsyncParts>,
    mut stream: Option<S>,
) -> Result<(), ()> {
    loop {
        let new_stream = stream.take();
        match handle_stream_optional(reporter, new_stream).await{
            //retry => Continue
            // ok / error => return
        }
    }
}

async fn handle_stream_optional<S: DownloadStream, F: ThreadModel>(
    reporter: &Reporter<'static, F, AsyncParts>,
    stream: Option<S>,
) -> Result<(), ()> {
    let stream = match stream {
        Some(s) => s,
        None => {
            fetch_normal_stream(reporter).await?;
        }
    };

    handle_stream(reporter, stream).await?;
}

///创建连接
async fn fetch_normal_stream<F: ThreadModel>(
    reporter: &Reporter<'static, F, AsyncParts>,
) -> impl DownloadStream {
    todo!()
}

///写入下载流
async fn handle_stream<S: DownloadStream, F: ThreadModel>(
    reporter: &Reporter<'static, F, AsyncParts>,
    stream: S,
) -> Result<(), ()> {
    let remain = &reporter.slot().remain;
    let writer: &'static dyn BufWriter = todo!();
    let remain_writer = RemainWriter::new(remain, writer);
    todo!()
}
