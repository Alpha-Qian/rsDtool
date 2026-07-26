//!下载接口Traits

use std::{num::NonZero, ops::ControlFlow, sync::atomic::Ordering};

use radium::Radium;

use crate::base::segment::Segment;

///实现者通常是恢复器
pub trait SegmentPacked {
    type Error;
    fn unpack_segment<D: DownloadContext>(
        self,
    ) -> (
        SegmentCache,
        impl AsyncFnMut(&D) -> ControlFlow<Result<(), Self::Error>>,
    );
}

///依赖注入的下载句柄
pub trait DownloadContext {
    fn remain(&self) -> &impl Radium<Item = i64>;

    fn is_aborted(&self) -> bool;
}

///可重试的下载器
pub trait Downloader {
    type Error;

    /// 返回重试句柄，
    /// 调用一次下载一次
    /// let retry_handle = downloader.into_retry_handle()
    async fn into_retry_handle<F, D: DownloadContext>(self, segment_cache: SegmentCache) -> F
    where
        F: AsyncFnMut(&D) -> ControlFlow<Result<(), Self::Error>>;
}

///和Segment一样，但remain字段没有NonZero规范要求，方便转换
#[derive(Debug, Clone)]
pub struct SegmentCache {
    pub process: u64,
    pub remain_cache: i64,
}

impl SegmentCache {
    pub fn new(process: u64, remain_cache: i64) -> Self {
        Self {
            process,
            remain_cache,
        }
    }
}

///Manager生成下载句柄的抽象，好像没什么用
trait Regist {
    fn regist_segment(&self, segment: Segment) -> impl DownloadContext;
}

impl From<Segment> for SegmentCache {
    fn from(segment: Segment) -> Self {
        let remain = segment.remain.get() as i64;
        Self {
            process: segment.end - remain,
            remain_cache: remain,
        }
    }
}

impl TryFrom<SegmentCache> for Segment {
    type Error = ();

    fn try_from(segment_cache: SegmentCache) -> Result<Self, Self::Error> {
        let remain = u64::try_from(segment_cache.remain_cache)
            .ok()
            .and_then(NonZero::new)?;
        Self::new(segment_cache.process, remain).into()
    }
}
