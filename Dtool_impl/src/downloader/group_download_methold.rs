use std::{ops::ControlFlow, sync::atomic::Ordering};

use crate::{
    base::{
        family::ThreadModel,
        group_construct::{Slot, SlotShare},
        pwriter::BufWriter,
        segment::Segment,
    },
    downloader::{
        group_async_parts::{AsyncParts, Slot2},
        group_worker::SegmentWorker,
    },
};

// ///IOC反转控制模式
// trait SubmitToGroup {
//     type Error;

//     async fn submit<M: ThreadModel>(
//         self,
//         f: impl FnOnce(Segment) -> SegmentWorker<Self::Error, M>, //TODO: SegmentWorker改成Slot
//     ) -> Result<ControlFlow<()>, Self::Error>;
// }
//

///A alias name of AsyncFnOnce(C) -> Result<bool, Self::Error>
pub trait Downloader<C> {
    type Error;
    async fn download(self, ctx: C) -> Result<bool, Self::Error>;
}

///直接从Segment创建下载任务
pub trait IntoDownloader<C> {
    type Downloader: Downloader<C>;
    fn into_download_method(self, segment: Segment) -> Self::Downloader;
}

///能力提供模式
/// SegmentProvider
/// DownloadTask
pub trait SegmentProvider<C> {
    type DownloadMethod: Downloader<C>;

    ///into_parts
    fn provide_parts(self) -> (Segment, Self::DownloadMethod);

    async fn execute<F>(self, f: F) -> Result<bool, <Self::DownloadMethod as Downloader<C>>::Error>
    where
        F: FnOnce(Segment) -> C,
        Self: Sized,
    {
        let (segment, method) = self.provide_parts();
        let injecter = f(segment);
        method.download(injecter).await
    }
}

///Downloader只是一个trait别名
impl<F, C, E> Downloader<C> for F
where
    C: DownloadContext,
    F: AsyncFnOnce(C) -> Result<bool, E>,
{
    type Error = E;

    ///with context
    async fn download(self, context: C) -> Result<bool, Self::Error> {
        self(context).await
    }
}

///依赖注入
pub trait DownloadContext {
    ///send donwloaded length, return remain **before** report
    fn reporter_downloaded(&self, chunk_length: usize) -> i64;

    fn is_aborted(&self) -> bool;
}

fn need_download<I: DownloadContext, D: Downloader<I>>(download: D) {
    todo!()
}

///创建新下载
struct New;

///从嗅探响应下载
struct FromSniffingResponse;
