//!由于不确定是否可续传，可升级到多线程下载的临时单线程下载器

use crate::{
    base::{
        group_construct::DownloadGroup, download_stream::DownloadStream, family::ThreadModel, pwriter::BufWriter, request_info::RequestInfo, subcontext::{FetchError, Writer}
    },
    downloader::retry_condition::reqwest_retryable,
};
use futures::{future::{self, Either, join, select}, stream};
use headers::{HeaderMapExt, Range};
use reqwest::{Client, Response, StatusCode};
use std::{
    any::Any,
    cell::Cell,
    future::poll_fn,
    marker::{PhantomData, PhantomPinned},
    ops::ControlFlow,
    pin::pin,
    str::Bytes,
    task::Poll,
};

struct Builder {
    info: RequestInfo,
    client: Client,
    length: u64,
}

impl Builder {}

///返回Future和 first_remain
/// Future在下载完成时返回None，升级成功时返回Some
///
/// download and try partical fetch
async fn upgrading_download<M: ThreadModel, W: BufWriter>(
    info: RequestInfo,
    client: Client,
    response: Option<Response>,

    length: u64,
    mut progress_visor: impl FnMut(usize),
    pwriter: &W,
) -> UpgradedOk<'_, W, impl DownloadStream> {
    let max_retry = 5;

    //第一个线程的进度和流，
    let process = 0_u64;
    //继续轮询第一个future时通知其可控退出
    let select_done = Cell::new(false); //or name safe_exiting

    // 在下载完成时返回None
    // 不是取消安全的
    // 如果确定了是否支持范围请求，则返回("是否支持"， Response)
    // 返回 Result<Option<Response>, Err>
    let download_future = async {
        let retryed_times = 0;
        //retry loop
        'retry: loop {
            let response: Response = match response {
                Some(r) => r,
                None => {
                    let request = info.build_request();
                    request.headers_mut().typed_insert(Range::bytes(process..));

                    let execute = pin!(client.execute(request));

                    let response: Response = match poll_fn(|cx| {
                        if select_done.get() {
                            Poll::Ready(ControlFlow::Break(()))
                        } else {
                            execute.poll(cx).map(ControlFlow::Continue)
                        }
                    })
                    .await
                    {
                        ControlFlow::Continue(Ok(r)) => r,
                        ControlFlow::Continue(Err(e)) => {
                            // handle error
                            if reqwest_retryable(&e) && retryed_times <= max_retry {
                                continue 'retry;
                            } else {
                                return Err(e);
                            }
                        }
                        ControlFlow::Break(()) => {
                            // select exiting
                            return Ok(None);
                        }
                    };

                    //let response = client.execute(request).await?;

                    if response.status() == StatusCode::PARTIAL_CONTENT {
                        return Ok(Some(true, response.bytes_stream()));
                    }

                    if process != 0 && response.status() != StatusCode::PARTIAL_CONTENT {
                        return Ok(Some(false, response.bytes_stream()));
                    }

                    //first_stream = Some(response.bytes_stream());
                }
            };

            let stream = response.bytes_stream();

            let writer = Writer::new(pwriter, process);
            loop {
                //TODO: 抛出错误
                //这个await点是取消不安全的
                match writer.fetch_chunk(stream).await {
                    Ok(ControlFlow::Continue(())) => {
                        progress_visor(*writer.process());

                        if select_done.get() {
                            return Ok(Some(stream));
                        }
                    }
                    Ok(ControlFlow::Break(())) => {
                        progress_visor(*writer.process());
                    }
                    Err(e) => match e {
                        FetchError::Stream(e) => {
                            if reqwest_retryable(&e) && retryed_times <= max_retry {
                                retryed_times += 1;
                                continue 'retry;
                            } else {
                                return Err(FetchError(e));
                            }
                        },
                        t @ _ => return Err(e),
                    },
                }
            }
        }
    };

    // 返回(bool: "是否支持范围请求", Response)
    // 取消安全
    let partical_fetch_future = async {
        let process = todo!();
        let request = info.build_request();
        request.headers_mut().insert("Range", todo!());
        let retry_times = 0;
        let response = loop {

            //错误超出一定次数后抛出
            match client.execute(request).await {
                Ok(r) => break r,
                Err(e) => {
                    //重试或退出
                    if reqwest_retryable(&e) && retry_times <= max_retry {
                        retry_times += 1;
                        continue;
                    } else {
                        return None;
                    }
                }
            }
        };

        if response.status() == StatusCode::PARTIAL_CONTENT {
            return Some(response);
        } else {
            return None;
        }
    };

    let download_future = pin!(download_future);
    let partical_fetch_future = pin!(partical_fetch_future);

    let either = future::select(download_future, partical_fetch_future).await;
    match either {
        // 第一个线程先下载完
        // 错误被视为下载错误
        Either::Left((download_result, paritical_fetch_future)) => {
            match download_result {
                todo!();
            }
        }

        // 第二个线程先收到请求
        // 错误被视为不支持续传
        Either::Right((partical_fetch_result, download_future)) => {
            match partical_fetch_result {
                todo!();
            }

            select_done.set(true);
            download_future.await
        }
    };

    todo!()
}

enum UpgradedOk<'a, W, S> {
    ///成功升级到可续传下载
    Succes(UpgradeSuccess<'a, W, S>),
    ///升级到可续传下载失败
    Fail(UpgradeFail<'a, W, S>),
    ///下载完成，无需升级
    Done,
}

struct UpgradeSuccess<'a, W, S> {
    first_stream: S,
    first_process: u64,
    first_end: u64,

    second_task: Option<SecondTask>,

    writer: &'a W,
    info: RequestInfo,
    client: Client,
}

struct UpgradeFail<'a, W, S> {
    stream: S,
    length: u64,

    writer: &'a W,
    info: RequestInfo,
    client: Client,
}

struct SecondTask {
    response: Response,
    second_end: u64, //or lehgth
}

fn muti_thread_download_on_success<'a, W, S>(success: UpgradeSuccess<'a, W, S>) -> (DownloadGroup<'static, impl ThreadModel, AsyncParts>, impl Future, Option<impl Future>) {
    todo!()
}

async fn download_on_fail<'a, W, S>(fail: UpgradeFail<'a, W, S>) {
    todo!()
}




// ///在轮询future前先检查条件
// async fn check_and_then<F: Future>(
//     fut: F,
//     break_codition: impl FnOnce() -> bool,
// ) -> ControlFlow<(), F::Output> {
//     let pinned = pin!(fut);
//     poll_fn(|cx| {
//         if break_codition() {
//             return Poll::Ready(ControlFlow::Break(()));
//         }
//         pinned.poll(cx).map(ControlFlow::Continue)
//     })
//     .await
// }

// async fn try_get_partical_response(
//     info: &RequestInfo,
//     client: &Client,
//     process: u64,
//     //end: Option<u64>,
// ) {
//     let request = info.build_request();
//     request.headers_mut().typed_insert(Range::bytes(process..));

//     let response = client.execute(request).await;

// }

// fn assert_unpin<T: Unpin>(_: &T){
//     ()
// }

// fn test() {
//     let a = PhantomPinned;
//     assert_unpin(&a);
// }

trait HandleRetry {
    ///第一个连接的重试
    fn first() -> impl AsyncFnMut() -> ControlFlow<()>;

    ///尝试升级连接的重试
    fn upgrade() -> impl AsyncFn() -> ControlFlow<()>;
}
