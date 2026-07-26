use std::{cmp::max, fs::File, ops::{ControlFlow, Deref}, os::windows::fs::FileExt, sync::{Arc, atomic::Ordering}};

use crate::{
    base::{
        family::ThreadModel, group_construct::State, request_info::RequestInfo, segment::Segment,
    },
    downloader::group_downloader_interface::{DownloadContext, Downloader, SegmentCache},
};
use radium::Radium;
use reqwest::{Client, StatusCode};
///依赖: TOkio
struct DefaultDownloader<M: ThreadModel> {
    info: RequestInfo,
    client: Client,
    file: M::RefCounter<File>,
}

impl<M: ThreadModel> Downloader for DefaultDownloader<M> {
    type Error = ();

    async fn into_retry_handle<F, D: DownloadContext>(self) -> F
        where
            F: AsyncFnMut(SegmentCache, &D) -> ControlFlow<Result<(), Self::Error>> {
        async move |
    }
}



impl<M: ThreadModel> SegmentDownload for DefaultDownloader<M> {
    type Error = ();

    fn clone_downloader(&self) -> Self
    where
        Self: Clone,
    {
        todo!()
    }

    fn into_download_method<C: DownloadContext>(
        self,
        segment: Segment,
    ) -> impl AsyncFnOnce(C) -> Result<bool, Self::Error> {
        async move |ctx| {
            let process = segment.start;
            let remain_cache = ctx.load_remain();
            'l: while remain_cache > 0 {
                let mut response = match self.client.execute(self.info.build_request()).await {
                    Ok(r) => r,
                    Err(e) => {
                        continue;
                    }
                };

                match response.status() {
                    StatusCode::PARTIAL_CONTENT => (),
                    status @ _ => {
                        if status.is_client_error() {
                            return Err(todo!());
                        }

                        if status.is_server_error() {
                            continue;
                        }
                    }
                };

                'stream: loop {
                    let b = match response.chunk().await {
                        Ok(Some(b)) => b,
                        Ok(None) => break,
                        Err(e) => {
                            todo!()
                        }
                    };

                    let to_write_len = max(b.len(), remain_cache as );
                    let to_write = b.slice(..max(b.len(), remain_cache as ));

                    match tokio::task::spawn_blocking(todo!("write all")).await.unwrap() {
                        Ok(()) => (),
                        Err(err) => {
                            todo!()
                        }
                    };

                    remain_cache = ctx.remain().fetch_sub(to_write_len as i64, Ordering::Relaxed) - to_write_len as i64;
                    if remain_cache <= 0 {

                    }
                }
            }
            return todo!();
        }
    }
}

// impl<C: DownloadContext> Downloader<C> for DefaultDownloader {
//     async fn download(self, ctx: &C) -> Result<bool, Self::Error> {
//         //retry_loop

//         loop {
//             todo!()
//         }
//     }
// }
