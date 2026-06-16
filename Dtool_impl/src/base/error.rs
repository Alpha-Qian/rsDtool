use super::download_stream::DownloadStream;
use super::pwriter::BufWriter;
use futures::stream::Aborted;
use headers::{ContentRange, HeaderMapExt};
use reqwest::{Response, StatusCode, header::HeaderMap};
use std::{
    error::Error,
    fmt::{Debug, Display},
    time::SystemTime,
};

// fn error_for_succes<S, W>(
//     response: Response,
//     only_partical: bool,
// ) -> Result<Response, SubError<S, W>>
// where
//     S: DownloadStream<Error = reqwest::Error>,
//     W: BufWriter,
// {
//     let status_code = response.status();
//     if response.status() == StatusCode::PARTIAL_CONTENT {
//         return Ok(response);
//     } else if response.status() == StatusCode::OK && !only_partical {
//         return Err(SubError::Download(DownloaderError::ErrorSuccesStatus(
//             status_code,
//         )));
//     } else if 200 < status_code.as_u16() && status_code.as_u16() < 300 {
//         return Err(SubError::Download(DownloaderError::ErrorSuccesStatus(
//             status_code,
//         )));
//     };

//     let error = response.error_for_status_ref().unwrap_err();

//     if status_code == StatusCode::REQUEST_TIMEOUT {
//         //408
//         return Err(SubError::CustomRetry(RetrySuggest::Immediately, error));
//     } else if status_code == StatusCode::TOO_MANY_REQUESTS {
//         //429
//         return Err(SubError::CustomRetry(RetrySuggest::WaitFuzzy, error));
//     }

//     if status_code.is_client_error() {
//         let error = response.error_for_status_ref().unwrap_err();
//         return Err(SubError::CustomRetry(RetrySuggest::Break, error));
//     } else if status_code.is_server_error() {
//         return Err(SubError::CustomRetry(RetrySuggest::WaitFuzzy, error));
//     } else {
//         let error = response.error_for_status_ref().unwrap_err();
//         return Err(SubError::CustomRetry(RetrySuggest::Break, error));
//     }
// }
//

///下载器产生的原始错误
pub enum RawError<S, W>
where
    S: DownloadStream,
    W: BufWriter,
{
    Writer(W::Error),
    NetWork(S::Error, Option<RetrySuggest>),

    SubDownload(SubDownloadError),
    Download(DownloaderError),
}

///下载组错误
pub enum SuperError<R, S, W>
where
    S: DownloadStream,
    W: BufWriter,
{
    Writer(W::Error),
    NetWork(S::Error, Option<R>),
    Download(DownloaderError),
}

impl<S, W> RawError<S, W>
where
    S: DownloadStream,
    W: BufWriter,
{
    pub(crate) fn new_network(error: S::Error) -> Self {
        Self::NetWork(error, None)
    }

    fn new_network_with_suggest(error: S::Error, suggest: RetrySuggest) -> Self {
        Self::NetWork(error, Some(suggest))
    }
}

impl<S, W> From<SubDownloadError> for RawError<S, W>
where
    S: DownloadStream,
    W: BufWriter,
{
    fn from(value: SubDownloadError) -> Self {
        Self::SubDownload(SubDownloadError)
    }
}

impl<S, W> Debug for RawError<S, W>
where
    S: DownloadStream,
    W: BufWriter,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Writer(e) => f.debug_tuple("Writer").field(e).finish(),
            Self::Download(e) => f.debug_tuple("Other").field(e).finish(),
            Self::NetWork(e) => f.debug_tuple("NetWork").field(e).finish(),
            Self::SubDownload(e) => f.debug_tuple("SubDownload").field(e).finish(),
        }
    }
}

impl<S, W> Display for RawError<S, W>
where
    S: DownloadStream,
    W: BufWriter,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NetWork(e) => write!(f, "网络错误"),
            Self::Writer(e) => write!(f, "写入错误"),
            Self::Download(e) => write!(f, "逻辑错误"),
            Self::SubDownload(e) => write!(f, "子下载错误"),
        }
    }
}

impl<S, W, R> Error for RawError<S, W>
where
    S: DownloadStream,
    W: BufWriter,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NetWork(e) => Some(e),
            Self::Writer(e) => Some(e),
            Self::Download(e) => Some(e),
            Self::SubDownload(e) => Some(e),
        }
    }
}

impl<R, S, W> Debug for SuperError<R, S, W>
where
    R: Debug,
    S: DownloadStream,
    W: BufWriter,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NetWork(e) => f.debug_tuple("NetWork").field(e).finish(),
            Self::Writer(e) => f.debug_tuple("Writer").field(e).finish(),
            Self::Download(e) => f.debug_tuple("Other").field(e).finish(),
        }
    }
}

impl<R, S, W> Display for SuperError<R, S, W>
where
    S: DownloadStream,
    W: BufWriter,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NetWork(e) => write!(f, "网络错误（可能经过重试）"),
            Self::Writer(e) => write!(f, "写入器错误"),
            Self::Download(e) => write!(f, "下载错误"),
        }
    }
}

impl<R, S, W> Error for SuperError<R, S, W>
where
    R: Debug,
    S: DownloadStream,
    W: BufWriter,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NetWork(e) => e,
            Self::Writer(e) => e,
            Self::Download(e) => e,
        }
    }
}

#[derive(Debug)]
pub enum SubDownloadError {
    UnexceptedEOF,
}

impl Display for SubDownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexceptedEOF => f.write_str("Unexcept Response EOF"),
        }
    }
}

impl Error for SubDownloadError {}

#[derive(Debug)]
pub enum DownloaderError {
    //TooSlow,                       //speed less than 1kb/s
    ErrorSuccesStatus(StatusCode), //Sever should return Partical response, but didn't
    ErrorContentRange,             //sever return error range
    ErrorContentLength,            // sever return error length
    BadResponse(&'static str),     //服务器似乎没有遵守http(s)规范
}

impl Display for DownloaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooSlow => write!(f, "错误：下载过慢"),
            Self::ErrorContentLength => write!(f, "服务器返回的数据长度不一致"),
            Self::ErrorContentRange => write!(f, "服务器返回的数据范围不一致"),

            Self::ErrorSuccesStatus(StatusCode::OK) => write!(f, "服务器拒绝了范围请求(200 Ok)"),
            Self::ErrorSuccesStatus(c) => {
                let str = c.canonical_reason().unwrap_or("None");
                write!(f, "服务器返回了意料之外的成功响应码：{c:} {str:} ")
            }
            Self::BadResponse(msg) => write!(f, "服务器似乎没有遵守http规范 {msg:}"),
        }
    }
}

impl DownloaderError {
    fn err_for_unpratical_content(status: StatusCode) -> Result<(), Self> {
        if status == StatusCode::PARTIAL_CONTENT {
            Ok(())
        } else {
            Err(Self::ErrorSuccesStatus(status))
        }
    }

    // fn check_partical_response(
    //     status: StatusCode,
    //     headers: &HeaderMap,
    //     range_start: u64,
    // ) -> Result<(), Self> {
    //     if status != StatusCode::PARTIAL_CONTENT {
    //         return Err(Self::ErrorSuccesStatus(status));
    //     };
    //     let Some(v) = headers.typed_get::<ContentRange>() else {
    //         return Self::BadResponse("返回206响应码但没有ContentRange响应头");
    //     };

    //     if let (Some(range), Some(len)) = (v.bytes_range(), v.bytes_len()) {
    //         if range.1 - range.0 + 1 != len {
    //             return Self::BadResponse("()");
    //         }
    //     }

    //     Ok(())
    // }
}

#[derive(Debug)]
enum RetrySuggest {
    Break,

    Immediately,
    WaitFuzzy,
    WaitSecond(usize),
    WaitUntil(SystemTime),

    WaitForNetWork,
}

impl Display for RetrySuggest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Break => f.write_str("建议退出重试"),
            Self::Immediately => f.write_str("建议立即重试"),
            Self::WaitFuzzy => f.write_str("建议一会后重试"),
            Self::WaitSecond(s) => write!(f, "建议{s}秒后重试"),
            Self::WaitUntil(t) => {
                write!(f, "建议{t:?} 后重试")
            }
            Self::WaitForNetWork => write!(f, "似乎是网络问题，建议一会后重试"),
        }
    }
}

#[derive(Debug)]
pub struct RawNetWorkError<T>(T);

impl<T> RawNetWorkError<T> {
    pub fn add_suggest(self, suggest: RetrySuggest) -> IntoRawNetWorkError<T> {
        IntoRawNetWorkError {
            error: self.0,
            suggest: Some(suggest),
        }
    }

    pub fn none_suggest(self) -> IntoRawNetWorkError<T> {
        IntoRawNetWorkError {
            error: self.0,
            suggest: None,
        }
    }

    pub fn into_network_error(self, suggest: Option<RetrySuggest>) -> IntoRawNetWorkError<T> {
        IntoRawNetWorkError {
            error: self.0,
            suggest,
        }
    }
}

impl<T> From<T> for RawNetWorkError<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> Display for RawNetWorkError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RawNetWorkError")
    }
}

impl<T: Error + 'static> Error for RawNetWorkError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

impl<S, W> From<RawNetWorkError<S::Error>> for RawError<S, W>
where
    S: DownloadStream,
    W: BufWriter,
{
    fn from(value: RawNetWorkError<S::Error>) -> Self {
        Self::NetWork(value.0, None)
    }
}

///防止重复impl 的new type包装器
#[derive(Debug)]
pub struct IntoRawNetWorkError<E> {
    error: E,
    suggest: Option<RetrySuggest>,
}

impl<T> IntoRawNetWorkError<T> {
    fn new(error: T) -> Self {
        Self {
            error,
            suggest: None,
        }
    }

    fn with_suggest(error: T, suggest: RetrySuggest) -> Self {
        Self {
            error,
            suggest: Some(suggest),
        }
    }

    fn with_raw(error: T, suggest: Option<RetrySuggest>) -> Self {
        Self {
            error,
            suggest: suggest,
        }
    }
}

impl<T> From<T> for IntoRawNetWorkError<T> {
    fn from(value: T) -> Self {
        Self {
            error: value,
            suggest: None,
        }
    }
}

impl<S, W> From<IntoRawNetWorkError<S::Error>> for RawError<S, W>
where
    S: DownloadStream,
    W: BufWriter,
{
    fn from(value: IntoRawNetWorkError<S::Error>) -> Self {
        Self::NetWork(value.error, None)
    }
}

impl<T> Display for IntoRawNetWorkError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IntoNetWorkError")
    }
}

impl<T: Error + 'static> Error for IntoRawNetWorkError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}

#[derive(Debug)]
pub struct IntoWriterError<E>(E);

impl<T> From<T> for IntoWriterError<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<S, W> From<IntoWriterError<W::Error>> for RawError<S, W>
where
    S: DownloadStream,
    W: BufWriter,
{
    fn from(value: IntoWriterError<W::Error>) -> Self {
        Self::Writer(value.0)
    }
}

impl<T: Display> Display for IntoWriterError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: Error + 'static> Error for IntoWriterError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

pub struct Aborted;

impl Display for Aborted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Aborted")
    }
}

impl Error for Aborted {}

type MayShouldRetry<T, E> = Result<T, E>;

async fn handle_may_should_retry<T, E>(
    f: impl AsyncFnMut() -> Result<MayShouldRetry<T, E>, E>,
) -> Result<T, E> {
    loop {
        let r = f().await?;
        match r {
            Ok(t) => return Ok(t),
            Err(e) => r,
        }
    }
}

#[derive(Debug)]
struct IntoRetryedError<T>(T);

impl<T> Display for IntoRetryedError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("IntoRetryedError")
    }
}

impl<T: Error> Error for IntoRetryedError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

trait RetryChecker<E> {
    type RetryError;
    fn check_retryable(error: &E) -> Option<impl FnOnce(E) -> Self::RetryError>;
}

pub type DownloadResult<T, S, G> = Result<Result<Result<T, ()>, G>, Aborted>;
pub fn aborted<T, S, G>() -> DownloadResult<T, S, G> {
    Err(Aborted())
}
pub fn download_ok<T, S, G>(value: T) -> DownloadResult<T, S, G> {
    Ok(Ok(Ok(value)))
}
pub fn download_may_retry_error<T, S, G>(sub_error: S) -> DownloadResult<T, S, G> {
    Ok(Ok(Err(sub_error)))
}
pub fn download_error<T, S, G>(super_error: G) -> DownloadResult<T, S, G> {
    Ok(Err(super_error))
}

enum MayRetryError {}
enum GroupError {}
