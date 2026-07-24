use std::marker::PhantomData;

///定义下载方式通用trait
use crate::base::segment::Segment;

/// 直接从Segment创建下载任务
/// 实现的结构体通常会携带Client和Writer字段
pub trait SegmentDownload {
    type Error;

    /// 返回值：
    /// Ok(true) -> 执行任务窃取，
    /// Ok(false) -> 不执行任务窃取，
    /// Err(_) -> 保存错误和分段信息并中止所有线程
    fn into_download_method<C: DownloadContext>(
        self,
        segment: Segment,
    ) -> impl AsyncFnOnce(C) -> Result<bool, Self::Error>;

    /// 克隆下载器
    /// 设计用于任务窃取
    fn clone_downloader(&self) -> Self
    where
        Self: Clone,
    {
        self.clone()
    }

    /// 直接执行，出于方便添加的方法
    async fn execute<C: DownloadContext>(
        self,
        segment: Segment,
        ctx: C,
    ) -> Result<bool, Self::Error>
    where
        Self: Sized,
    {
        self.into_download_method(segment).download(ctx).await
    }
}

/// 能力提供模式
/// 通常携带Segment字段和 impl SegmentDownload需要的所有字段，可能携带传输到一半的Response
pub trait SegmentResume {
    type Error;

    //type Downloader: SegmentDownload<Error = Self::Error>;

    ///恢复下载，比如从传输到一半的Response恢复
    fn resume<C: DownloadContext>(
        self,
    ) -> (Segment, impl AsyncFnOnce(C) -> Result<bool, Self::Error>);

    // fn into_downloader(&self) -> impl SegmentDownload<Error = Self::Error>;

    /// 恢复并执行
    async fn execute<F, C>(self, ctx_factory: F) -> Result<bool, Self::Error>
    where
        F: FnOnce(Segment) -> C,
        C: DownloadContext,
        Self: Sized,
    {
        let (segment, method) = self.resume();
        let injecter = ctx_factory(segment);
        method.download(injecter).await
    }
}

///依赖注入
pub trait DownloadContext {
    ///send donwloaded length, return remain **before** report
    fn reporter_downloaded(&self, chunk_length: usize) -> i64;

    fn is_aborted(&self) -> bool;
}

///A alias name of AsyncFnOnce(C) -> Result<bool, Self::Error>
/// Maybe Downloader<C> can be rename to CanInjectBy<C>
pub trait Downloader<C: DownloadContext> {
    type Error;
    async fn download(self, ctx: &C) -> Result<bool, Self::Error>;
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

fn need_download<I: DownloadContext, D: Downloader<I>>(download: D) {
    todo!()
}

///创建新下载
struct New;

///从嗅探响应下载
struct FromSniffingResponse;
