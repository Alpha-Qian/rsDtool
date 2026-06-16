use std::error::Error;
use std::str::Bytes;
use std::sync::atomic::Ordering;

use futures::StreamExt;
use radium::Radium;
use reqwest::{Client, Response};

use crate::base::pwriter::BufWriter;
use crate::base::{family::ThreadModel, request_info::RequestInfo};
use crate::base
struct Downloader<W, F, S, M: ThreadModel> {
    info: RequestInfo,
    client: Client,
    response: Option<Response>,

    stream_process: M::RefCounter<M::AtomicCell<u64>>,
    write_process: u64,
    max_length: u64,
    writer: W,
}


///跳过已写入的片段并下载
async fn download<E, M: ThreadModel>(
    info: RequestInfo,
    client: Client,
    response: Option<Response>,
    max_length: u64,
    writer: &impl BufWriter,
    start: u64,
    process: M::RefCounter<M::AtomicCell<u64>>,
    retry_condition: impl FnMut(&E) -> bool
) -> Result<(), impl Error> {
    let mut write_process = start;
    loop{
        let stream_process = 0_u64;
        let Ok(response) = client.execute(info.build_request()).await else {
            break retry_condition
        };

        let last_chunk: Option<Bytes> = loop {
            let remain = write_process - stream_process;
            if remain as i64 <= 0 {
                break;
            }

            let Some(chunk) = response.chunk().await? else {
                todo!()
            };

            if chunk.len() > remain {
                break chunk.slice(remain..)
            }

            stream_process += chunk.len()
        };

        if let Some(chunk) = last_chunk {
            let write_len = chunk.len();
            writer.pwrite(write_process, chunk).await;
            write_process += write_len;
            process.store(write_process, Ordering::Relaxed);
        }

        while let Some(chunk) = response.chunk().await? {
            let write_len = chunk.len();
            writer.pwrite(write_process, write_len).await;
            write_process += write_len
            process.store(write_process, Ordering::Relaxed);
        }

    }
}
