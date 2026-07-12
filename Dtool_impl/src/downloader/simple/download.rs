use std::str::Bytes;
use std::time::Duration;

use futures::TryStreamExt;
use futures::future::{Either, select};
use futures::{StreamExt, future::Select};
use tokio::time::Instant;

use crate::base::{
    download_stream::DownloadStream, family::ThreadModel, pwriter::BufWriter,
    request_info::RequestInfo,
};

struct DownloadInfo {
    info: RequestInfo,
    process: u64,
}

struct SniffingResponse {
    response: Response,
}

struct Downloader<M: ThreadModel> {
    data: M::RefCounter<ShareData>,
}

impl Downloader {
    fn from_sniffing_response(r: SniffingResponse) -> (Self, impl Future + 'static) {
        todo!()
    }

    fn resume_from(info: DownloadInfo) -> (Self, impl Future + 'static) {
        todo!()
    }
}

struct ShareData<M: ThreadModel> {
    progress: M::AtomicCell<u64>,
}

async fn download(data: &ShareData) {}

async fn fetch_stream<R: RetryStrategy>(
    stream: impl DownloadStream,
    writer: &impl BufWriter,
    timer: impl AsyncFnMut(Duration),
    config: DownloadConfig<R>,
) {
    loop {
        let chunk: Result<Bytes> =
            match select(stream.try_next(), timer(config.stream_timeout)).await {
                Either::Left(_) => {
                    todo!()
                }
                Either::Right(_) => {
                    break;
                }
            };
    }
}

struct DownloadConfig<R> {
    //超时会立即重试
    get_timeout: Duration,
    stream_timeout: Duration,

    retry_strategy: R,
}

///仅适用单线程的重试策略
trait RetryStrategy {
    fn get_wait_time(&mut self) -> Option<Duration>;
}
