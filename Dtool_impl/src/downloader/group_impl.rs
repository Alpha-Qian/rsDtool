use crate::downloader::{
    download_group::{DownloadGroup, GroupExt, GroupGuard, Reporter, ReporterGuard, Slot},
    family::{RefCounted, ThreadModel},
    httprequest::RequestInfo,
    segment::Segment,
};
use bytes::{Bytes};
use futures::{StreamExt, channel::oneshot::Canceled, future::{Either, poll_fn, ready}};
use futures::{FutureExt, Stream, TryStream, TryStreamExt};
use futures::future::select;
use headers::{AcceptRanges, ContentRange, ETag, Header, HeaderMapExt, IfUnmodifiedSince, LastModified, Range};
use radium::Radium;
use reqwest::{
    Client, Request, Response, StatusCode, header::{Entry, HeaderMap, HeaderValue}
};
use std::{
    cell::{Cell, UnsafeCell}, cmp::min, convert::Infallible, error::Error, fmt::{Debug, Display}, future, num::NonZero, ops::{Bound, ControlFlow, Deref, RangeBounds}, pin::{self, Pin}, sync::atomic::Ordering, task::{self, Poll, Waker}
};
use tokio::task::AbortHandle;


macro_rules! any {
    () => {
        impl AnyThing
    };
}


async fn run_async_group<F: ThreadModel>() {

    let group: AsyncGroup<F> = todo!();


}
struct AsyncGroup<F: ThreadModel>{
    inner: DownloadGroup<'static, F, Ext>
}

impl<F: ThreadModel> AsyncGroup<F> {

    fn run_future<F: Future>(&mut self, future: impl FnOnce(Reporter<'static, F, Ext>) -> F) {
        
    } 

    async fn wait_all(&mut self) {
        let locked_group = self.inner.lock();
        *locked_group.inlock_ext_mut().chancel = Channel::Waiting(get_waker().await)
        futures::
    }

    fn wait_all2(&mut self) -> impl Future{
        WaitAll(self)
    }
}

struct WaitAll<'a>(&'a mut AsyncGroup);

impl<'a> Future for WaitAll<'a>  {

    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> task::Poll<Self::Output> {
        let group = self.get_mut();
        let mut locked_group = group.0.inner.lock();
        *locked_group.inlock_ext_mut().chancel = Channel::Waiting(cx.waker().clone());
        drop(locked_group);
        Poll::Pending
    }
}

enum WaitAll2<'a>{
    State1(&'a mut AsyncGroup),
    State2(&'a mut AsyncGroup)
}

impl<'a> Future for WaitAll2<'a>  {
    type Output = Result<(), GroupError>;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        loop {
            match this {
                Self::State1(async_group) => {
                    let mut guard = async_group.inner.lock();
                    *guard.inlock_ext_mut().chancel = Channel::Waiting(cx.waker().clone());
                    drop(guard);

                    *this = Self::State2(async_group);
                    return Poll::Pending
                }

                Self::State2(async_group) => {
                    let mut guard = async_group.inner.lock();
                    match guard.inlock_ext().chancel {
                        Channel::Done(r) => {
                            return Poll::Ready(r)
                        },
                        _ => unreachable!()
                    }
                }
            }
        }
    }
}

// fn get_waker() -> impl Future<Output = Waker>{
//     poll_fn(|cx| task::Poll::Ready(cx.waker().clone()))
// }








#[derive(Clone, Copy)]
struct Ext;
impl<F: ThreadModel> GroupExt<F> for Ext {
    type GroupExt<'a> = GroupShareExt<F>;
    type InLockExt<'a> = InLockShareExt;
    type SlotInlockExt<'a> = SlotExt; //end
    type SlotExt<'a> = SlotShareExt<F>; //remain
}

struct GroupShareExt<F: ThreadModel> {
    info: RequestInfo,
    process: F::AtomicCell<u64>,
    writer: Box<dyn PWriter<Error = dyn Error>>,
}

struct InLockShareExt {
    segments: Vec<Segment>,
    //waker: Option<Waker>,
    chancel: Channel
}

struct SlotExt {
    end: u64,
}

struct SlotShareExt<F: ThreadModel> {
    remain: F::AtomicCell<u64>,
    abort: AbortHandle,
}

enum Channel<F: ThreadModel>{
    Waiting(Waker, Vec<Slot<'static, F, Ext>>/* Unsafe field */),
    Done(Result<(), GroupError>),
}

///需要导致所有线程停止的错误
enum GroupError {
    FileNoSupportRange,
    FileChanged,
    Authentication,
    Other(Box<dyn Error>),
    ClientError(StatusCode),
    StateCodeError,
}

async fn build_new(client: &Client, mut info: RequestInfo, pwriter: impl PWriter) {
    let mut request = info.build_request();
    request.headers_mut().typed_insert(Range::bytes(..).unwrap());
    let response = client.execute(request).await.unwrap().error_for_status().unwrap();
    let range_type = check_rangeable_for_full_range_request(&mut info, &response, false); 
    if let ResouseType::UnConfirm(ref length) = range_type{
        let remain = radium::Radon::new(length.clone() as i64);
        let abort_signal = Cell::new(false);

        let abort_test = Cell::new(false);
        let mut task = Task::new(response.bytes_stream(), pwriter, Some(length.clone() as i64));

        let test_response: Result<Option<Response>,()>;
        
        match select(
            pin::pin!(task.download(0, &remain, || {abort_signal.get()})), 
            pin::pin!(handle_test_request(client, &info, 1000..))
        ).await
        {
            //download complete first
            Either::Left((download_result, test_future)) => {
                
            },

            //get test response first
            Either::Right((test_result, download_future)) => {
                //必须保证download_future可控退出
                abort_signal.set(true);
                download_future.await;
            }
        }
        
        

        let resouse_type = match &test_response {
            Ok(Some(r)) => ResouseType::ResumeAble(*length),
            Ok(None) => ResouseType::UnResumeAble(*length),
            Err(_) => ResouseType::UnConfirm(*length)
        };

    }
}


async fn download(response: Response, process: u64, remain: &impl Radium<Item = i64>) -> TaskResult{
    let state_code = response.status();
    if (state_code.is_client_error() | state_code.is_server_error()) && state_code != StatusCode::TOO_MANY_REQUESTS{
        return group_error(GroupError::ClientError(state_code.clone()));
    }
    if response.status() != 206 && process != 0{
        return group_error(GroupError::FileNoSupportRange)
    }





}

// enum DownloadResult<T>{
//     Ok(T)
//     Task(TaskError<(),(),()>),
//     Group(GroupError)
// }

// impl<T> DownloadResult<T> {
//     fn () {
        
//     }
// }
type TaskResult = Result<Result<(), TaskError<>>, GroupError>;
fn task_error(task_error: TaskError<(),(),()>) -> TaskResult {
    Ok(Err(task_error))
}

fn group_error(group_error: GroupError) ->TaskResult {
    Err(group_error)
}

async fn first_response_download(client: &Client, info: &mut RequestInfo, test_abort: &Cell<bool>, abort_me: &Cell<bool>) {
    let mut first_try = true;
    let process = 0_u64;
    let range = Range::bytes(process..).unwrap();
    let mut request = info.build_request();
    request.headers_mut().typed_insert(range);
    let response = client.execute(request).await.unwrap().error_for_status().unwrap();
    if first_try{

    }
    if process == 0{
        check_rangeable_for_full_range_request(info, &response, false)
    };
} 


async fn handle_first_request(client: &Client, info: &mut RequestInfo) -> Result<(Response, ResouseType), TaskError<(), ()>>{
    let mut request = info.build_request();
    request.headers_mut().typed_insert(Range::bytes(..).unwrap());
    let response = 
        client
        .execute(request)
        .await.unwrap()
        .error_for_status().unwrap();
    let resouse_type = check_rangeable_for_full_range_request(info, &response,  false);
    Ok((response, resouse_type))
}

async fn handle_test_request(client: &Client, info: &RequestInfo, bounds: impl RangeBounds<u64>) -> Result<Option<Response>, ()> {
    let mut request = info.build_request();
    //debug_assert!(*bounds.start_bound() != 0);
    request.headers_mut().typed_insert(Range::bytes(bounds).unwrap());
    let response = 
        client
        .execute(request)
        .await.unwrap()
        .error_for_status().unwrap();
    if response.status() == 206{
        return Ok(Some(response))
    }
    Ok(None)
}


// async fn handle_test_request_loop(client: &Client, info: &RequestInfo, bounds: impl RangeBounds<u64> + Clone) -> Result<Option<Response>, ()>{
//     let mut retry_times = 5;
//     loop {
//         if retry_times == 0 {break Err(())}
//         retry_times -= 1;
//         handle_test_request(client, info, bounds.clone()).await?;
//     }
// }

///retry and return the last error
async fn ok_or_retry<Fut: Future<Output = Result<T, E>>, T, E> (mut f: impl FnMut() -> Fut, mut times: usize) -> Result<T, E> {
    loop{
        times -= 1;
        match f().await{
            Ok(value) => return Ok(value),
            Err(err) => {
                times -= 1;
                if times == 0{
                    return Err(err)
                }
            }
        }
    }
}

fn include_resume_check_header(headers: &HeaderMap) -> bool{
    headers.typed_get::<ETag>().is_some() | headers.typed_get::<LastModified>().is_some()
}


///为请求头添加响应头中的Etag和Modified信息，顺便检测是否存在这两个字段
/// 如果已经存在，则什么都不干
fn set_conditions_request(info: &mut HeaderMap, response: &HeaderMap) -> bool {
    if let Entry::Vacant(v) = info.entry(ETag::name())
        && let Some(etag) = response.get(ETag::name())
    {
        v.insert(etag.clone());
    } else if let Entry::Vacant(v) = info.entry(IfUnmodifiedSince::name())
        && let Some(motified) = response.get(LastModified::name())
    {
        v.insert(motified.clone());
    } else {
        return false
    }
    true
}

///检查服务器是否支持范围请求的通用方法，可能无法确定
pub fn check_rangeable_for_full_range_request(info: &mut RequestInfo, response: &Response, force: bool) -> ResouseType{
    //顺序：优先级由高到低

    //如果无法获取文件长度，则不可续传
    let length = match response.content_length() {
        None => return ResouseType::UnkownLegth,
        Some(length) => length,
    };

    //如果没有etag或修改时间，因为无法判断文件是否改变则视为不可续传
    if !force && !set_conditions_request(&mut info.headers, response.headers()) {
       return ResouseType::UnResumeAble(length)
    }

    //如果是范围响应，视为可续传
    if is_partial_response(response) {
        return ResouseType::ResumeAble(length)
    }

    //根据服务器声明
    if let Some(v) = response.headers().typed_get::<AcceptRanges>() {
        if v.is_bytes(){
            return ResouseType::ResumeAble(length);
        }
        if v.is_none(){
            return ResouseType::UnResumeAble(length);
        }
    }

    if force{
        ResouseType::ResumeAble(length)
    } else {
        ResouseType::UnConfirm(length)
    }
}



///根据响应码判断
/// 在range请求头 range= 0-  时可能将可续传连接误判为不可续传连接，取决于服务器
/// Output: suport or unsuport
pub fn rangeable_for_partial_request(response: &Response) -> bool{
    is_partial_response(response)
}

//const F : for<'a> fn(&'a reqwest::Response) -> bool = is_partial_response;

fn is_partial_response(response: &Response) -> bool {
    response.status() == 206
}

#[derive(Clone, Debug)]
enum ResouseType{
    ResumeAble(u64),
    UnResumeAble(u64),
    UnConfirm(u64),
    UnkownLegth,
}

impl ResouseType {
    fn resume_able(&self) -> bool {
        match self {
            Self::ResumeAble(_) => true,
            _ => false
        }
    }
    fn length(&self) -> Option<u64> {
        match self {
           Self::ResumeAble(v) | Self::UnResumeAble(v)=> Some(*v),
           _ => None
        }
    }

}

async fn send_second_request(client: &Client, info: &RequestInfo) {
    let mut requestt = info.build_request();
    requestt.headers_mut();//set_range
    let response = client.execute(info.build_request()).await.unwrap();
    
}


struct Task<St, W>{
    stream: St,
    writer: W,
    last_remain: i64,

}

impl<St, W> Task<St, W> {
    fn new(stream: St, writer: W, last_remain: Option<i64>) -> Self{
        let last_remain = last_remain.unwrap_or(i64::MAX);
        Self{
            stream,
            writer,
            last_remain,
        }
    }

    fn into_raw(self) -> (St, W, i64) {
        (self.stream, self.writer, self.last_remain)
    }
}
impl<St, W> Task<St, W>
where 
    St: TryStream<Ok = Bytes> + Unpin,
    W: PWriter,
{
    async fn download(&mut self, mut process: u64, remain: &impl Radium<Item = i64>, abort_token: impl FnMut() -> bool) -> Result<(), TaskError<St::Error, W::Error, _ >>{
        while let Some(bytes) = self.stream.try_next().await.map_err(TaskError::Stream)? {

            let write_length = min(bytes.len(), self.last_remain as usize);
            let write_bytes = bytes.slice(..write_length);

            self.writer.pwrite(process, write_bytes).await.map_err(TaskError::Write);
            self.last_remain = remain.fetch_sub(write_length as i64, Ordering::Release);
            process += write_length as u64;


            if self.last_remain <= 0{
                break;
            }

            if abort_token() {
                return Err(TaskError::Cancelled);
            }
        }
        Ok(())
    }
}

//use futures::future::select::select;
//#[derive(PartialEq)]
enum TaskError<St, W, I> {
    Stream(St),
    Write(W),
    GetResponseHeaders(I),
    Cancelled,
    Others(Box<dyn Error>),
    NotParticalResponse
}

impl TaskError<(),(),()> {
    fn into_group_error(self) -> Result<(), GroupError> {
        match self {
            Self::Stream(_) | Self::Cancelled => Ok(()),
            Self::NotParticalResponse => Err(GroupError::FileNoSupportRange),
            Self::Write(_) => Err(GroupError::Other(()))
        }
    }
}

impl<St, W, I> TaskError<St, W, I> {
   fn is_error_range(&self) -> bool {
        matches!(self, Self::StateCode(416))
   }
}
trait AnyThing{}

impl<T: ?Sized> AnyThing for T {}


fn check_if_return_early<F: ThreadModel>(
    bytes: Bytes,
    writed: &mut usize,
    remain: &F::AtomicCell<i64>,
) -> Option<Bytes> {
    let remain = remain.fetch_sub(*writed as i64,Ordering::Release);
    if remain > 0 {
        let data_len = bytes.len();
        let write_len = min(remain as usize, data_len);
        Some(bytes.slice(..write_len))
    } else {
        None
    }
}

trait PWriter {
    type Error;
    async fn pwrite(&self, pos: u64, bytes: Bytes) -> Result<(), Self::Error>;
}

struct Buffer(UnsafeCell<[u8]>);

impl PWriter for Buffer {
    type Error = Infallible;
    async fn pwrite(&self, pos: u64, bytes: Bytes) -> Result<(), Self::Error> {
        unsafe{
            let ptr = self.0.get();
            assert!(pos as usize <= ptr.len());

            let raw_ptr = (ptr as *mut u8).offset(pos as isize);
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), raw_ptr, bytes.len());
        };
        Ok(())
    }
}

struct Writer<W>{
    pwriter: W,
    offset: u64,
}

impl<W> Writer<W> {

    fn new(pwriter: W, offset: u64) -> Self {
        Self { pwriter, offset }
    }

    fn seek(&mut self, pos: u64) {
        self.offset = pos
    }

    fn offset(&self) -> &u64{
        &self.offset
    }
    
    fn offset_mut(&mut self) -> &mut u64 {
        &mut self.offset
    }

    fn into_raw(self) -> (W, u64) {
        (self.pwriter, self.offset)
    }

    
}

impl<W: PWriter> Writer<W>  {
    async fn write(&self, data: Bytes) -> Result<(), W::Error> {
        self.pwriter.pwrite(self.offset, data).await
    }
}

struct ErrorMsg<T>(T);

impl<T: Deref<Target = str>> ErrorMsg<T> {
    fn new_err(msg: T) -> Result<std::convert::Infallible, Self> {
        Err(Self(msg))
    }
}

impl<T: Deref<Target = str>> Debug for ErrorMsg<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Debug::fmt(self.0.deref(), f)
    }
}
impl<T: Deref<Target = str>> Display for ErrorMsg<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self.0.deref(), f)
    }
}
impl<T: Deref<Target = str>> Error for ErrorMsg<T> {
    fn description(&self) -> &str {
        self.0.deref()
    }
}

use std::os::windows::fs::FileExt;
use std::fs::File;

impl PWriter for File {
    type Error = std::io::Error;

    async fn pwrite(&self, pos: u64, bytes: Bytes) -> Result<(), Self::Error> {
        // 克隆文件句柄（Windows 下 File 的 clone 是浅拷贝，指向同一个系统对象）
        let file = self.try_clone()?; 
        
        tokio::task::spawn_blocking(move || {
            let mut buf = bytes.as_ref();
            let mut current_pos = pos;
            
            while !buf.is_empty() {
                let n = FileExt::seek_write(&file, buf, current_pos)?;
                if n == 0 {
                    return Err(std::io::Error::new(std::io::ErrorKind::WriteZero, "write zero"));
                }
                buf = &buf[n..];
                current_pos += n as u64;
            }
            Ok(())
        }).await.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
    }
}






// impl RangeAble {
//     fn
// }

// async fn try_loop<'data, F>(
//     mut start: u64,
//     guard: ReporterGuard<'data, F, Ext>
// )
// where
//     F: ThreadModel,
// {
//     let reporter = guard.release_lock();
//     while let result = try_once(&mut reporter, &mut start).await && let Err(()) = result  {

//     }

// }

// fn build_first_request(){}
// fn build_second_request(){}
// fn build_resume_request(){}

// fn get_first_repsonse(){}

// fn get_resume_response(){}

// async fn try_once<'data, F>(mut writer: impl PWriter, client: Client, reporter: &mut Reporter<'data, F, Ext>, start: &mut u64,) -> Result<(),()>
// where F: ThreadModel
// {
//     //let end =
//     let request = reporter.group().info.clone();
//     let mut response = client
//         .execute(request.into())
//         .await.unwrap()
//         .error_for_status()
//         .map_err(|e| ())
//         .and_then(|response| Ok(response)).unwrap();

//     let mut writed = 0_u64;

//     while let Some(bytes) = response.chunk().await.unwrap() {
//         let remain = reporter.slot_ext().remain.fetch_sub(writed, Ordering::Relaxed);
//         *start += writed;

//         writed = min(bytes.len() as u64, remain);
//         writer.pwrite(*start, bytes.slice(0..(remain as usize)));

//     };
//     Ok(())
// }
// ///
// struct AsyncGroup<'a, F: ThreadModel>{
//     group: DownloadGroup<'static, F, Ext>,
//     length: u64,
//     client: &'a Client
// }

// impl<F: ThreadModel> AsyncGroup<F> {
//     // async fn new(raw: DownloadGroup<'static, F, Ext>) -> Self{
//     //     let waker = clone_waker().await;
//     //     Self{raw, waker}
//     // }
//     async fn new_reporter() {
//         let waker = clone_waker().await;
//         todo!()
//     }

//     fn lock(&self) -> AsyncGroupGuard<'_, F>{
//         AsyncGroupGuard::new(self.group.lock())
//     }

//     fn join_all(&self) {
//         self.lock().join_all();
//     }

// }

// struct AsyncGroupGuard<'a, F: ThreadModel>{
//     guard: GroupGuard<'a, 'static, F, Ext>,
// }

// impl<'a, F: ThreadModel> AsyncGroupGuard<'a, F> {

//     fn new(guard: GroupGuard<'a, 'static, F, Ext>) -> Self{
//         Self{guard}
//     }

//     async fn new_reporter(&mut self) -> Option<AsyncReporter<F>>{
//         if self.guard.in_lock_ext().aborting{ return None;}
//         let waker = clone_waker().await;
//         let a = self.guard.new_reporter(0, <F::RefCounter<u64> as RefCounted>::new(0));
//         AsyncReporter{reporter: a, waker}.into()
//     }

//     fn join_all(&mut self) -> impl Future {
//         poll_fn(|c| {
//             if self.guard.slots_mut().is_empty() {
//                 Ready(())
//             } else {
//                 let a: &mut DownloadGroup<'_, F, Ext> = self.guard.group_mut();
//                 a.inner.waker.register(c.waker());
//                 Pending
//             }
//         })
//     }

//     fn set_waker(&mut self, waker: Waker) -> Waker{
//         self.guard.
//     }

//     fn abort_all(&mut self) {// todo: 移动到
//         //let slots = self.guard.slots_optional();

//         for i in self.guard.slots().unwrap(){
//             i.share().abort.abort();
//         }
//         self.guard.slots_optional().set_empty();
//     }
// }

// struct DownloadWorker<'data, F: ThreadModel>{
//     reporter: Reporter<'data, F, Ext>
// }

// impl<'data, F: ThreadModel> DownloadWorker<'data, F> {
//     fn exit(&self) {
//         let mut guard = self.reporter.lock();
//         guard.remove_me();
//         let waker = guard.inlock_ext().waker.take();
//         //先释放再唤醒，避免竞争
//         drop(guard);

//         if let Some(waker) = waker{
//             waker.wake();
//         }
//     }
// }
// // trait WriterCreater{

// }
