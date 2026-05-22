use std::{error::Error, fmt::{Debug, Display, write}, ops::{Add, ControlFlow, Sub}, time::{Duration, Instant, SystemTime}};
use reqwest::{Response, StatusCode};
use crate::downloader::group_impl::{DownloadStream};
use super::pwriter::PWriter;

fn error_for_succes<S, W, R>(response: Response, only_partical: bool) -> Result<Response, SubError<S, W, R>> 
where 
    S: DownloadStream<Error = reqwest::Error>, 
    W: PWriter, 
    R: RequestLimiter,
{
    let status_code = response.status();
    if response.status() == StatusCode::PARTIAL_CONTENT{
        return Ok(response)
    } else if response.status() == StatusCode::OK && !only_partical {
        return Err(SubError::Other(DownloaderError::ErrorSuccesStatus(status_code)))
    } else if 200 < status_code.as_u16() && status_code.as_u16() < 300 {
        return Err(SubError::Other(DownloaderError::ErrorSuccesStatus(status_code)))
    };
    
    let error = response.error_for_status_ref().unwrap_err();
    
    if status_code == StatusCode::REQUEST_TIMEOUT {//408
        return Err(SubError::CustomRetry(RetrySuggest::Immediately, error))

    } else if status_code == StatusCode::TOO_MANY_REQUESTS {//429
        return Err(SubError::CustomRetry(RetrySuggest::WaitFuzzy, error));
    }

    if status_code.is_client_error() {
        let error = response.error_for_status_ref().unwrap_err();
        return Err(SubError::CustomRetry(RetrySuggest::Break, error));
    } else if status_code.is_server_error() {
        return Err(SubError::CustomRetry(RetrySuggest::WaitFuzzy, error))
    } else {
        let error = response.error_for_status_ref().unwrap_err();
        return Err(SubError::CustomRetry(RetrySuggest::Break, error));
    }
}


pub enum SubError<S, W> 
where 
    S: DownloadStream, 
    W: PWriter,
{
    Writer(W::Error),
    Other(DownloaderError),
    NetWork(S::Error, Option<RetrySuggest>),
    //CustomRetry(RetrySuggest, S::Error),
}

impl<S, W> Debug for SubError<S, W>
where
    S: DownloadStream,
    W: PWriter,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Writer(e) => f.debug_tuple("Writer").field(e).finish(),
            Self::Other(e) => f.debug_tuple("Other").field(e).finish(),
            Self::NetWork(e) => f.debug_tuple("NetWork").field(e).finish(),
            Self::CustomRetry(r, e ) => f.debug_tuple("CustomRetry").field(r).finish(),
        }
    }
}

impl<S, W> Display for SubError<S, W> 
where 
    S: DownloadStream, 
    W: PWriter,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self{
            Self::NetWork(e) => write!(f, "网络错误"),
            Self::Writer(e) => write!(f, "写入错误"),
            Self::Other(e) => write!(f, "逻辑错误"),
            Self::CustomRetry(r) => write!(f, "todo")
        }
    }
}

impl<S, W, R> Error for SubError<S, W, R> 
where 
    S: DownloadStream, 
    W: PWriter,
    R: RequestLimiter,

{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NetWork(e) => Some(e),
            Self::Writer(e) => Some(e),
            Self::Other(e) => Some(e),
            Self::CustomRetry(r) => Some(r)
        }
    }
}

pub enum SuperError<S, R, W> 
where 
    S: DownloadStream, 
    R: RequestLimiter + ?Sized, 
    W: PWriter
{
    NetWork(NetWorkError<S, R>),
    Writer(W::Error),
    Other(DownloaderError),
    NetWork2(R::Info, S::Error)
}

impl<S, R, W> Debug for SuperError<S, R, W>
where 
    S: DownloadStream,
    R: RequestLimiter,
    W: PWriter,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NetWork(e) => f.debug_tuple("NetWork").field(e).finish(),
            Self::Writer(e) => f.debug_tuple("Writer").field(e).finish(),
            Self::Other(e) => f.debug_tuple("Other").field(e).finish(),
        }
    }
}

impl<S, R, W> Display for SuperError<S, R, W> 
where 
    S: DownloadStream, 
    R: RequestLimiter, 
    W: PWriter 
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self{
            Self::NetWork(e) => write!(f, "网络错误（可能经过重试）"),
            Self::Writer(e) => write!(f, "写入器错误"),
            Self::Other(e) => write!(f, "下载错误")
        }
    }
}


impl<S, R, W> Error for SuperError<S, R, W> 
where 
    S: DownloadStream, 
    R: RequestLimiter, 
    W: PWriter 
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self{
            Self::NetWork(e) => e,
            Self::Writer(e) => e,
            Self::Other(e) => e,
        }
    }
}

struct NetWorkError<T, R> 
where 
    T: DownloadStream, 
    R: RequestLimiter + ?Sized
{
    retry_info: R::Info,
    error: T::Error,
}

impl<T, R> Display for NetWorkError<T,R> 
where 
    T: DownloadStream, 
    R: RequestLimiter
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let retry_error = &self.retry_info;
        let source = &self.error;
        write!(f, "网络错误: {source:}， 重试错误：{retry_error:}")
    }
}

#[derive(Debug)]
pub enum DownloaderError{
    TooSlow,//speed less than 1kb/s
    ErrorSuccesStatus(StatusCode),//Sever should return Partical response, but didn't
    ErrorContentRange, //sever return error range
    ErrorContentLength, // sever return error length
}

impl Display for DownloaderError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self{
            Self::TooSlow => write!(f, "错误：下载过慢"),
            Self::ErrorContentLength => write!(f, "服务器返回的数据长度不一致"),
            Self::ErrorContentRange => write!(f, "服务器返回的数据范围不一致"),

            Self::ErrorSuccesStatus(StatusCode::OK) => write!(f, "服务器拒绝了范围请求(200 Ok)"),
            Self::ErrorSuccesStatus(c) => {
                let str = c.canonical_reason().unwrap_or("None");
                write!(f, "服务器返回了意料之外的成功响应码：{c:} {str:} ")}
        }
    }
}

impl Error for DownloaderError {}

trait RequestLimiter{

    type Info: Error;

    fn get_request_limit(&self) -> Duration{
        Duration::ZERO
    }

    // fn report_result<S, R, W>(&mut self, result: RetryOperation<S, R, W>) -> Result<(), Self::Info>//Ok => continue retry, Err => Break retry 
    // where S: DownloadStream, R: RequestLimiter, W: PWriter;

    fn report_result<S, W>(&mut self, network_error: S::Error, suggest: Option<RetrySuggest>) -> RetryOperation<S, Self, W>
    where 
        S: DownloadStream,
        W: PWriter;
    //fn unretryed_info<S: DownloadStream>(network: &S::Error) -> Self::Info;
}

enum RetryResult<S, W, R>
where 
    S: DownloadStream,
    W: PWriter,
    R: RequestLimiter
{
    Break(SuperError<S, R, W>),
    Wait(Duration),
}

async fn retry_by_strategy<T, R, B, W>(f: impl AsyncFnMut() -> Result<T, SubError<B, W>>) -> Result<T, SuperError<B, R, W>> 
where R: RequestLimiter, B: DownloadStream, W: PWriter 
{
    loop {
        let result = f().await;
        let result = match result{
            Err(e) => {
                match handle_reqwest_error(e, None) {
                    RetryOperation::Break => 
                }
            },
            Ok(t) => Ok(t)
        };
    }
}



enum RetryOperation<S: DownloadStream, R: RequestLimiter + ?Sized, W: PWriter> {
    Break(SuperError<S, R, W>),

    Immediately,
    WaitFuzzy,
    WaitSecond(usize),
    WaitUntil(SystemTime),

    WaitForNetWorkFuzzy,
}

#[derive(Debug)]
enum RetrySuggest<B = ()>{
    Break(B),

    Immediately,
    WaitFuzzy,
    WaitSecond(usize),
    WaitUntil(SystemTime),

    WaitForNetWorkFuzzy,
}

impl Display for RetrySuggest{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Break => f.write_str("建议退出重试"),
            Self::Immediately => f.write_str("建议立即重试"),
            Self::WaitFuzzy => f.write_str("建议一会后重试"),
            Self::WaitSecond(s) => write!(f, "建议{s}秒后重试"),
            Self::WaitUntil(t) => {
                write!(f, "建议{t:?} 后重试")
            },
            Self::WaitForNetWorkFuzzy => write!(f,"似乎是网络问题，建议一会后重试")
        }
    }
}