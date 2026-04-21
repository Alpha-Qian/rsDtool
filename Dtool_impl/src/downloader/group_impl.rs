use std::{error::Error, fmt::Display, future, marker::PhantomData, ops::{Deref, DerefMut}, pin::{self, Pin}, sync::atomic::Ordering, task::{self, Poll, Waker}};
use std::{future::poll_fn, task::Poll::{Pending, Ready}};
use bytes::Bytes;
use futures::{FutureExt, TryStream, TryStreamExt, task::AtomicWaker};
use futures::{Stream, StreamExt, Sink};
use headers::{HeaderMapExt, Range};
use radium::Radium;
use reqwest::{Client, Response};
use tokio::task::AbortHandle;
use std::cmp::min;
use pin_project::pin_project;
use std::task::ready;
use crate::downloader::{download_group::{DownloadGroup, GroupExt, GroupGuard, Reporter, ReporterGuard}, family::{RefCounted, ThreadModel}, httprequest::RequestInfo, segment::Segment};

async fn clone_waker() -> Waker{
    future::poll_fn(|c| task::Poll::Ready(c.waker().clone())).await
}

#[derive(Clone, Copy)]
struct Ext;
impl<F: ThreadModel> GroupExt<F> for Ext {
    type GroupExt<'a> = GroupShareExt<F>;
    type InLockExt<'a> = InLockShareExt;
    type SlotInlockExt<'a> = SlotExt;//end
    type SlotExt<'a> = SlotShareExt<F>;//remain
}

struct GroupShareExt<F: ThreadModel>{
    info: RequestInfo,
    process: F::AtomicCell<u64>,
    writer: Box<dyn Writer>
}
struct InLockShareExt{
    segments: Vec<Segment>,
    waker: Option<Waker>,
}

struct SlotExt{
    end: u64
}

struct SlotShareExt<F: ThreadModel>{
    remain: F::AtomicCell<u64>,
    abort: AbortHandle,
}

enum BuildeNewOption{
    RangeAble,
    NoSupport
}


async fn build_new(client: &Client ,info: RequestInfo) -> BuildeNewOption{
    let response = client.execute(info.build_request().headers_mut().typed_insert(Range::bytes(..))).await.unwrap()
    //response.bytes_stream()
    if response.status().as_u16() == 206{
        return BuildeNewOption::RangeAble;
    } else if response.status().as_u16() == 200 {
        async fn download_repsonse(response: Response){}
        let f1 = download_repsonse(response);
        let f2 = async{};
        let pined = pin::pin!(f1);
        let r;
        tokio::select! {
            r1 = pined => {
                r = r1;
            }
            r2 = f2 => {
                r = pined.await;
            }
        }
        response.bytes_stream()


        
    }
    todo!()
}
struct RangeAble{

}

impl RangeAble {
    fn
}


async fn try_loop<'data, F>(
    mut start: u64,
    guard: ReporterGuard<'data, F, Ext>
)
where 
    F: ThreadModel, 
{
    let reporter = guard.release_lock();
    while let result = try_once(&mut reporter, &mut start).await && let Err(()) = result  {
        
    }

    
}

fn build_first_request(){}
fn build_second_request(){}
fn build_resume_request(){}

fn get_first_repsonse(){}

fn get_resume_response(){}


async fn try_once<'data, F>(mut writer: impl Writer, client: Client, reporter: &mut Reporter<'data, F, Ext>, start: &mut u64,) -> Result<(),()>
where F: ThreadModel
{   
    //let end = 
    let request = reporter.group().info.clone();
    let mut response = client
        .execute(request.into())
        .await.unwrap()
        .error_for_status()
        .map_err(|e| ())
        .and_then(|response| Ok(response)).unwrap();

    let mut writed = 0_u64;
    
    while let Some(bytes) = response.chunk().await.unwrap() {
        let remain = reporter.slot_ext().remain.fetch_sub(writed, Ordering::Relaxed);
        *start += writed;

        writed = min(bytes.len() as u64, remain);
        writer.pwrite(*start, bytes.slice(0..(remain as usize)));
        
    };
    Ok(())
}
///
struct AsyncGroup<'a, F: ThreadModel>{
    group: DownloadGroup<'static, F, Ext>,
    length: u64,
    client: &'a Client
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
// trait WriterCreater{

// }
trait Writer{
    type Err;
    async fn pwrite(&self, pos: u64, bytes: Bytes ) -> Result<(), Self::Err>;
}

trait ErrorTypes{
    type Stream: Error + Display;
    type Write: Error + Display;
}


fn filter_map(stream: impl Stream<Item = Bytes>){
    stream.filter_map(f)
}
#[pin_project]
struct CutOff<'a, St, F: ThreadModel>{
    #[pin]
    stream: St,
    writed: i64,
    remain: &'a F::AtomicCell<i64>
}

impl<'a, St, F> CutOff<'a, St, F> 
where St: Stream<Item = Bytes>, F: ThreadModel
{
    fn new(stream: St, remain: &'a F::AtomicCell<i64>) -> Self{
        Self {
            stream,
            writed: 0,
            remain
        }
    }
}

impl<'a, St, F> Stream for CutOff<'a, St, F>
where St: Stream<Item = Bytes>, F: ThreadModel
{
    type Item = Bytes;
    fn poll_next(self: pin::Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();

        match this.stream.poll_next(cx) {
            Ready(Some(v)) => Ready({
                let remain = this.remain.fetch_sub(*this.writed, Ordering::Relaxed);
                if remain > 0{
                    let data_len = v.len();
                    let write_len = min(remain as usize, data_len);
                    Some(v.slice(..write_len))
                } else {
                    None
                }
            }),
            t @ _ => t
        }
    }
}

// Ready(Some(v)) => Ready({
//                 let remain = this.remain.fetch_sub(*this.writed, Ordering::Relaxed);
//                 if remain > 0{
//                     let data_len = v.len();
//                     let write_len = min(remain as usize, data_len);
//                     Some(v.slice(..write_len))
//                 } else {
//                     None
//                 }
//             }),
///一个实现了Unpin的下载器
struct Task<'data, I, O>{
    stream: I,
    writer: O,
    start: u64,
}

// impl<I, O, E> Future for Task<I, O> 
// where I: TryStream<Ok = Bytes> + Unpin, 
//     O: Writer,
//     Self: Unpin,
// {
//     type Output = ();
//     fn poll(mut self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
//         if let Some(bytes) = std::task::ready!(self.stream.poll_next_unpin(cx)){
//             let r = ready!(self.writer.pwrite(self.start, bytes));
//         } else {
//             Ready(())
//         }
//     }
// }


impl<'data, I, O> Task<'data, I, O>
where 
    I: TryStream<Ok = Bytes> + Unpin, 
    O: Writer,
{
    fn new(input: I, ouput: O, file_start: &mut u64, reporter: Reporter<'a, F, E>) {

    }

    async fn download_chunk<F: ThreadModel>(&mut self,remain: &F::AtomicCell<i64>) -> Result<(), TaskError<I::Error, O::Err>>{
        while let Some(bytes) = 
            self.stream
                .try_next()
                .await
                .map_err(TaskError::Stream)
                ?
        {
            let write_len = bytes.len();
            self.writer
                .pwrite(self.start, bytes)
                .await
                .map_err(TaskError::Write)
                ?;
            self.start += write_len as u64;
        };
        Ok(())
    }
}



enum TaskError<T, U> {
    Stream(T),
    Write(U),
    GetResponse,
    StateCode,
    Others(Box<dyn Error>)
}
type ErrorFamily<F: ErrorTypes> = TaskError<F::Stream, F::Write>;