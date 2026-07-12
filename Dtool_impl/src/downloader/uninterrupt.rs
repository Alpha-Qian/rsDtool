//!关键在于future是不是'static的




use crate::base::request_info::RequestInfo;


fn download_static<W: 'static>(
    info: RequestInfo,
    response
    writer: &'static W,
    retry: impl Retry,
) -> impl Future + 'static{


}


fn test_static_future() -> impl Future + 'static {

}


trait Retry{
    fn retryable(&mut self, write_process: u64) -> bool;
}


pub enum DownloadResult{
    ///下载完成
    Done,
    ///尝试恢复下载时发现可以升级到多线程
    UpGradeAble,
    ///下载失败
    Error,
}

// struct Downloader<W, F, S, M: ThreadModel> {
//     info: RequestInfo,
//     client: Client,
//     response: Option<Response>,

//     stream_process: M::RefCounter<M::AtomicCell<u64>>,
//     write_process: u64,
//     max_length: u64,
//     writer: W,
// }


// ///跳过已写入的片段并下载
// async fn download<E, M: ThreadModel>(
//     info: RequestInfo,
//     client: Client,
//     response: Option<Response>,
//     max_length: u64,
//     writer: &impl BufWriter,
//     start: u64,
//     progress_visor: impl FnMut(） ->
//     retry_condition: impl FnMut(&E) -> bool
// ) -> Result<(), impl Error> {
//     let mut write_process = start;
//     loop{
//         let stream_process = 0_u64;
//         let Ok(response) = client.execute(info.build_request()).await else {
//             break retry_condition
//         };

//         let last_chunk: Option<Bytes> = loop {
//             let remain = write_process - stream_process;
//             if remain as i64 <= 0 {
//                 break;
//             }

//             let Some(chunk) = response.chunk().await? else {
//                 todo!()
//             };

//             if chunk.len() > remain {
//                 break chunk.slice(remain..)
//             }

//             stream_process += chunk.len()
//         };

//         if let Some(chunk) = last_chunk {
//             let write_len = chunk.len();
//             writer.pwrite(write_process, chunk).await;
//             write_process += write_len;
//             process.store(write_process, Ordering::Relaxed);
//         }

//         while let Some(chunk) = response.chunk().await? {
//             let write_len = chunk.len();
//             writer.pwrite(write_process, write_len).await;
//             write_process += write_len
//             process.store(write_process, Ordering::Relaxed);
//         }

//     }
// }
