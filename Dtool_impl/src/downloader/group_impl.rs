
use crate::downloader::{
    download_group::{
        GroupExt, GroupGuard,
    }, error::SubError, family::{RefCounted, ThreadModel}, httprequest::RequestInfo, segment::Segment,
};
use bytes::Bytes;
use futures::{future::select, io::Write};
use futures::{Stream, TryStream, TryStreamExt};
use futures::{
    future::Either,
};
use headers::{
    AcceptRanges, ContentRange, ETag, Header, HeaderMapExt, IfUnmodifiedSince, LastModified, Range,
};
use radium::Radium;
use reqwest::{
    Client, Response, StatusCode,
    header::{Entry, HeaderMap},
};
use std::{
    cell::Cell, cmp::min, error::Error, fmt::Debug, marker::PhantomData, ops::{ControlFlow, RangeBounds}, pin, process::Output, sync::atomic::Ordering, time::Instant
};
use super::pwriter::BufWriter;

///resume
fn download_unchecked<S: DownloadStream, W: BufWriter>(info: RequestInfo, client: impl AsyncFn(RequestInfo) -> Result<S, SubError<S,W>>, pwriter: W) {

}

trait Strategy{

}











///
async fn send_request(client: &Client, info: &RequestInfo) -> Result<Response, reqwest::Error> {
    let request = info.build_request();
    client.execute(request).await?.error_for_status()
}

async fn simple_test<W>(client: &Client, info: &mut RequestInfo, allow_unversioned_file: bool) -> Result<(ResourceType, Response), reqwest::Error> {
    let response = send_request(client, info).await?;
    let resouce_type = check_response_resumability(info, &response, allow_unversioned_file);
    Ok((response, resouce_type))
}

async fn hanlde_uncomfirmed_response<F: ThreadModel>(length: u64, response: Response, client: &Client, info: &RequestInfo, writer: impl BufWriter) {
    async {
        let remain =
        let stream = response.bytes_stream();
        let context = DownloadContext::new(stream, &writer, None);
        context.download(0, remain, abort_token)
    }
}

struct CanResume<W, F: ThreadModel>{
    response1: Response,// or Download Context?
    remain1: F::RefCounter<F::AtomicCell<u64>>,
    response2: Option<(Response, F::RefCounter<F::AtomicCell<u64>>/* remain */)>,
    writer: W,
    info: RequestInfo,
    length: u64,
}

impl<W, F: ThreadModel> CanResume<W, F> {
    async fn run(self, visor: impl AsyncFnOnce(GroupGuard<'static, F, Ext<W>>)) -> Result<(), GroupError> {
        todo!()
    }
}
struct CannotResume<W, F: ThreadModel> {
    response: Response,
    remain: F::RefCounter<F::AtomicCell<u64>>,
    writer: W,
    info: RequestInfo,
    length: u64,
}

//未经检查创建
fn new_downloader_unchecked<S: DownloadStream>(info: RequestInfo, client: impl AsyncFn(RequestInfo) -> S, resume_info: Option<Vec<Segment>>) {
    info
}







struct Ext<W, S>(PhantomData<(W, S)>);

impl<W: BufWriter, S: DownloadStream,F: ThreadModel> GroupExt<F> for Ext<W, S> {
    type GroupShare<'a> = GroupShareExt<W, F>;
    //GroupInlock
    type Data<'a> = ();
    type IdleData<'a> = Result<(), super::error::SubError<S, W>>;
    type BusyData<'a> = F::RefCounter<F::AtomicCell<bool>>;

    type SlotInlock<'a> = SlotExt; //end
    type SlotShare<'a> = SlotShareExt<F>; //remain
}
struct GroupShareExt<W: BufWriter, F: ThreadModel> {
    info: RequestInfo,
    process: F::AtomicCell<u64>,
    writer: W,
    abort: F::AtomicCell<bool>,
}

struct SlotExt {
    end: u64,
}
struct SlotShareExt<F: ThreadModel> {
    remain: F::AtomicCell<u64>,
}




async fn build_new(client: &Client, mut info: RequestInfo, pwriter: impl BufWriter) {
    let mut request = info.build_request();
    request
        .headers_mut()
        .typed_insert(Range::bytes(..).unwrap());
    let response = client
        .execute(request)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let range_type = test_unpatical_response_resumeable(&mut info, &response, false);
    if let ResourceType::Unconfirmed(ref length) = range_type {
        let remain = radium::Radon::new(length.clone() as i64);
        let abort_signal = Cell::new(false);

        let abort_test = Cell::new(false);
        let mut task = DownloadContext::new(
            response.bytes_stream(),
            pwriter,
            Some(length.clone() as i64),
        );

        let test_response: Result<Option<Response>, ()>;

        match select(
            pin::pin!(task.download(0, &remain, || { abort_signal.get() })),
            pin::pin!(handle_test_request(client, &info, 1000..)),
        )
        .await
        {
            //download complete first
            Either::Left((download_result, test_future)) => {}

            //get test response first
            Either::Right((test_result, download_future)) => {
                //必须保证download_future可控退出
                abort_signal.set(true);
                download_future.await;
            }
        }

        let resouse_type = match &test_response {
            Ok(Some(r)) => ResourceType::Resumable(*length),
            Ok(None) => ResourceType::UnResumable(*length),
            Err(_) => ResourceType::Unconfirmed(*length),
        };
    }
}

async fn first_response_download<F: ThreadModel>(
    client: &Client,
    info: &mut RequestInfo,
    writer: &impl BufWriter,
    remain: &F::AtomicCell<i64>,
    test_abort: &Cell<bool>,
    abort_me: &Cell<bool>,
) {
    let mut first_try = true;
    let process = 0_u64;
    let range = Range::bytes(process..).unwrap();
    let mut request = info.build_request();
    request.headers_mut().typed_insert(range);
    let response = client
        .execute(request)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    if first_try {}
    if process == 0 {
        test_unpatical_response_resumeable(info, &response, false);
    }

    let task = DownloadContext::new(response.bytes_stream(), writer, None);
    //task.
    task.download(process, remain, || abort_me.get()).await?
}

async fn handle_first_request(
    client: &Client,
    info: &mut RequestInfo,
) -> Result<(Response, ResourceType), DownloadContextError<(), ()>> {
    let mut request = info.build_request();
    request
        .headers_mut()
        .typed_insert(Range::bytes(..).unwrap());
    let response = client
        .execute(request)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    let resouse_type = test_unpatical_response_resumeable(info, &response, false);
    Ok((response, resouse_type))
}

async fn handle_test_request(
    client: &Client,
    info: &RequestInfo,
    bounds: impl RangeBounds<u64>,
) -> Result<Option<Response>, ()> {
    let mut request = info.build_request();
    //debug_assert!(*bounds.start_bound() != 0);
    request
        .headers_mut()
        .typed_insert(Range::bytes(bounds).unwrap());
    let response = client
        .execute(request)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
    if response.status() == 206 {
        return Ok(Some(response));
    }
    Ok(None)
}

fn include_resume_check_header(headers: &HeaderMap) -> bool {
    headers.typed_get::<ETag>().is_some() | headers.typed_get::<LastModified>().is_some()
}

///为请求头添加响应头中的Etag和Modified信息，顺便检测是否存在这两个字段
/// 如果已经存在，则什么都不干
fn set_file_version(info: &mut HeaderMap, response: &HeaderMap) -> bool {
    if let Entry::Vacant(v) = info.entry(ETag::name())
        && let Some(etag) = response.get(ETag::name())
    {
        v.insert(etag.clone());
    } else if let Entry::Vacant(v) = info.entry(IfUnmodifiedSince::name())
        && let Some(motified) = response.get(LastModified::name())
    {
        v.insert(motified.clone());
    } else {
        return false;
    }
    true
}


pub async fn response_builder<S: DownloadStream, W: BufWriter>(client: &Client, info: &RequestInfo) -> Result<Response, SubError<S, W>> {
    let response = match client.execute(info.build_request()).await {
        Ok(r) => r,
        Err(e) if e.
    }
}

pub fn handle_first_full_response(info: &mut RequestInfo, response: Response, allow_unversioned_file: bool) -> (impl Stream, ResourceType) {
    let resource_type = check_response_resumability(info, &response, allow_unversioned_file);
    (response.bytes_stream(), resource_type)
}

pub fn handle_retry_full_response(response: Response) -> impl Stream {
    response.bytes_stream()
}

pub fn hendle_range_response(response: Response) -> (impl Stream, bool) {
    let rangeable = response.status() == StatusCode::PARTIAL_CONTENT;
    (response.bytes_stream(), rangeable)
}


async fn download_response<'a>(context: &mut DownloadContext<'a, impl Stream, impl BufWriter>, ) {

}


/// 检查服务器是否支持范围请求（Range Requests）。
///
/// 按优先级从高到低依次判断：
/// 1. 文件版本标识（ETag / Last-Modified）
/// 2. Content-Range 响应头（206 Partial Content）
/// 3. Content-Length
/// 4. Accept-Ranges 声明
pub fn check_response_resumability(//for first Full Range Request
    info: &mut RequestInfo,
    response: &Response,
    allow_unversioned_file: bool,
) -> ResourceType {
    let headers = response.headers();

    // 1. 无文件版本标识且不允许无版本文件 → 无法安全续传
    if !set_file_version(&mut info.headers, headers) && !allow_unversioned_file {
        return match response.content_length() {
            Some(length) => ResourceType::UnResumable(length),
            None => ResourceType::UnknownLength,
        };
    }

    // 2. 存在 Content-Range → 服务器已返回范围响应，视为可续传
    if let Some(range) = headers.typed_get::<ContentRange>() {
        let length = range.bytes_len().or_else(|| response.content_length());
        return match length {
            Some(len) => ResourceType::Resumable(len),
            None => ResourceType::UnknownLength,
        };
    }

    // 3. 必须有 Content-Length 才能继续判断
    let Some(length) = response.content_length() else {
        return ResourceType::UnknownLength;
    };

    // 4. 根据 Accept-Ranges 声明判断
    if let Some(accept_ranges) = headers.typed_get::<AcceptRanges>() {
        if accept_ranges.is_bytes() {
            return ResourceType::Resumable(length);
        } else {
            return ResourceType::UnResumable(length)
        }
    }
    ResourceType::Unconfirmed(length)
}


///根据响应码判断
/// 在range请求头 range= 0-  时可能将可续传连接误判为不可续传连接，取决于服务器
/// Output: suport or unsuport
pub fn test_partial_request_rangeable(response: &Response) -> bool {
    is_partial_response(response)
}

fn is_partial_response(response: &Response) -> bool {
    response.status() == 206
}

#[derive(Clone, Debug)]
enum ResourceType {
    Resumable(u64),
    UnResumable(u64),
    Unconfirmed(u64),
    UnknownLength,
}

impl ResourceType {
    fn resume_able(&self) -> bool {
        match self {
            Self::Resumable(_) => true,
            _ => false,
        }
    }
    fn length(&self) -> Option<u64> {
        match self {
            Self::Resumable(v) | Self::UnResumable(v) => Some(*v),
            _ => None,
        }
    }
}

async fn send_second_request(client: &Client, info: &RequestInfo) {
    let mut requestt = info.build_request();
    requestt.headers_mut(); //set_range
    let response = client.execute(info.build_request()).await.unwrap();
}



///无需pin所以方便移动的下载上下文
struct DownloadContext<'a, S, W> {
    stream: S,
    writer: &'a W,
    remain_cache: i64,
}

impl<'a, S, W> DownloadContext<'a, S, W> {
    fn new(stream: S, writer: &'a W, last_remain: Option<i64>) -> Self {
        let remain_cache = last_remain.unwrap_or(i64::MAX);
        Self {
            stream,
            writer,
            remain_cache,
        }
    }

    fn into_raw(self) -> (S, &'a W, i64) {
        (self.stream, self.writer, self.remain_cache)
    }

    fn into_stream(self) -> S {
        self.stream
    }

    fn new_stream(&mut self, stream: S) {
        self.stream = stream
    }

}

impl<'a, St, W> DownloadContext<'a, St, W>
where
    St: TryStream<Ok = Bytes> + Unpin,
    W: BufWriter,
{
    async fn download(
        &mut self,
        mut process: u64,
        remain: &impl Radium<Item = i64>,
        abort_token: impl FnMut() -> bool,
    ) -> Result<(), DownloadContextError<St::Error, W::Error, _>> {
        while let Some(bytes) = self.stream.try_next().await.map_err(DownloadContextError::Stream)? {
            let write_length = min(bytes.len(), self.remain_cache as usize);
            let write_bytes = bytes.slice(..write_length);

            self.writer
                .pwrite(process, write_bytes)
                .await
                .map_err(DownloadContextError::Write);
            self.remain_cache = remain.fetch_sub(write_length as i64, Ordering::Release);
            process += write_length as u64;

            if self.remain_cache <= 0 {
                break;
            }

            if abort_token() {
                return Err(DownloadContextError::Cancelled);
            }
        }
        Ok(())
    }


}

fn check_if_return_early<F: ThreadModel>(
    bytes: Bytes,
    writed: &mut usize,
    remain: &F::AtomicCell<i64>,
) -> Option<Bytes> {
    let remain = remain.fetch_sub(*writed as i64, Ordering::Release);
    if remain > 0 {
        let data_len = bytes.len();
        let write_len = min(remain as usize, data_len);
        Some(bytes.slice(..write_len))
    } else {
        None
    }
}



trait DownloadClient{
    type Error: Error;
    async fn new(self, info: RequestInfo) -> impl DownloadStream<Error = Self::Error>;
}

/// trait别名
pub(crate) trait DownloadStream: TryStream<Ok = Bytes, Error: Error> + Unpin {}

impl<T: TryStream<Ok = Bytes, Error: Error> + Unpin> DownloadStream for T {}
