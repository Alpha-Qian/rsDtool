use crate::base::base_error::FetchError;
use crate::base::{base_error::SubError, family::ThreadModel, pwriter::BufWriter};
use futures::TryStreamExt;
use radium::Radium;
use std::ops::Deref;
use std::{ops::ControlFlow, sync::atomic::Ordering};

use super::download_stream::DownloadStream;

///多线程逻辑下的remain同步逻辑
pub struct Remain<'a, F>
where
    F: ThreadModel,
{
    remain: &'a F::AtomicCell<i64>,
    cache: i64,
}

impl<'a, F> Remain<'a, F>
where
    F: ThreadModel,
{
    pub fn new(remain: &'a F::AtomicCell<i64>) -> Self {
        Self {
            remain,
            cache: i64::MAX,
        }
    }

    pub fn split_chunk(&mut self, chunk_len: usize) -> ControlFlow<usize> {
        if chunk_len < self.cache as usize {
            ControlFlow::Continue(())
        } else {
            ControlFlow::Break(self.cache as usize)
        }
    }

    pub fn record_writed(&mut self, writed: usize) {
        let progress_now = self.remain.fetch_sub(writed as i64, Ordering::AcqRel) - writed as i64;
        self.cache = progress_now;
    }

    pub fn cache(&self) -> &i64 {
        &self.cache
    }

    pub fn cache_mut(&mut self) -> &mut i64 {
        &mut self.cache
    }

    pub fn remain(&self) -> &F::AtomicCell<i64> {
        &self.remain
    }

    pub fn into_raw(self) -> (&'a F::AtomicCell<i64>, i64) {
        (self.remain, self.cache)
    }

    pub fn flash_cache(&mut self) {
        self.cache = self.remain.load(Ordering::Acquire)
    }
}

pub struct Writer<'a, W> {
    pwriter: &'a W,
    process: u64,
}

impl<'a, W> Writer<'a, W>
where
    W: BufWriter,
{
    pub fn new(pwriter: &'a W, process: u64) -> Self {
        Self { pwriter, process }
    }

    pub async fn write_all<B: Deref<Target = [u8]> + 'static>(
        &mut self,
        buffer: B,
    ) -> Result<(), W::Error> {
        let len = buffer.deref().len();
        self.pwriter.pwrite(self.process, buffer).await?;
        self.process += len as u64;
        Ok(())
    }

    pub fn process(&self) -> u64 {
        self.process
    }

    pub async fn fetch_util<S>(
        &mut self,
        stream: &mut S,
        mut condition: impl FnMut(u64) -> bool,
    ) -> Option<Result<(), FetchError<S, W>>>
    where
        S: DownloadStream,
    {
        loop {
            let bytes = match stream.try_next().await.map_err(FetchError::Stream)? {
                Some(bytes) => bytes,
                None => return Some(Ok(())),
            };

            self.write_all(bytes).await.map_err(FetchError::Write);
            if condition(self.process) {
                return None;
            }
        }
        //Some(Ok(()))
    }

    pub async fn fetch_all<S: DownloadStream>(
        &mut self,
        mut stream: S,
    ) -> Result<(), FetchError<S, W>> {
        while let Some(bytes) = stream.try_next().await.map_err(FetchError::Stream)? {
            self.write_all(bytes).await.map_err(FetchError::Write)?
        }
        Ok(())
    }

    ///取消不安全
    pub async fn fetch_chunk<S: DownloadStream>(
        &mut self,
        stream: &mut S,
    ) -> Result<ControlFlow<()>, FetchError<S, W>> {
        let bytes = match stream.try_next().await.map_err(FetchError::Stream)? {
            Some(b) => b,
            None => return Ok(ControlFlow::Break(())),
        };
        let len = bytes.len();
        self.write_all(bytes).await.map_err(FetchError::Write)?;
        Ok(ControlFlow::Continue(()));
    }

    pub async fn fetch_chunk_in_remain<S: DownloadStream>(
        &mut self,
        stream: &mut S,
        remain: i64,
    ) -> Result<ControlFlow<()>, FetchError<S, W>> {
        let bytes = match stream.try_next().await.map_err(FetchError::Stream)? {
            Some(b) => b,
            None => return Ok(ControlFlow::Break(())),
        };
        let raw_len = bytes.len();
        if raw_len < remain as usize {
            self.write_all(bytes).await?;
            return Ok(ControlFlow::Continue(()));
        } else {
            self.write_all(bytes.slice(..remain)).await?;
            return Ok(ControlFlow::Break(()));
        }
    }
}

pub struct RemainWriter<'a, W, F>
where
    F: ThreadModel,
{
    writer: Writer<'a, W>,
    remain: Remain<F>,
}

impl<'a, W, M> RemainWriter<'a, W, M>
where
    W: BufWriter,
    M: ThreadModel,
{
    pub fn new(remain: &'a M::AtomicCell<u64>, writer: &'a impl BufWriter) -> Self {
        todo!()
    }

    pub fn with_raw(writer: Writer<'a, W>, context: Remain<M>) -> Self {
        Self {
            writer,
            remain: context,
        }
    }

    pub fn write_process(&self) -> &u64 {
        &self.writer.process
    }

    pub fn remain_cache(&self) -> &i64 {
        &self.remain.cache
    }

    ///在remain范围内写入数据
    pub async fn write_in_remain(
        &mut self,
        buffer: impl Deref<Target = [u8]> + 'static,
    ) -> Result<ControlFlow<usize>, W::Error> {
        let raw_len = buffer.deref().len();
        match self.remain.split_chunk(buffer.deref().len()) {
            ControlFlow::Continue(()) => {
                self.writer.write_all(buffer).await?;
                self.remain.record_writed(raw_len);
                return Ok(ControlFlow::Continue(()));
            }
            ControlFlow::Break(new_len) => {
                self.writer.write_all(buffer).await?;
                self.remain.record_writed(new_len);
                return Ok(ControlFlow::Break(new_len));
            }
        };
    }

    // pub async fn write_record(
    //     &mut self,
    //     buffer: impl Deref<Target = [u8]> + 'static,
    // ) -> Result<(), W::Error> {
    //     self.writer.write_all(buffer).await?;
    //     self.remai
    // }

    //pub async fn fetch_stream_optional<S>(mut self)

    ///写入Stream
    pub async fn fetch_stream<S>(&mut self, mut stream: S) -> Result<(), FetchError<S, W>>
    where
        S: DownloadStream,
    {
        while let Some(bytes) = stream.try_next().await.map_err(FetchError::Stream)? {
            let raw_len = bytes.len();
            match self.remain.split_chunk(bytes.len()) {
                ControlFlow::Continue(()) => {
                    self.writer
                        .write_all(bytes)
                        .await
                        .map_err(FetchError::Write)?;
                    self.remain.record_writed(raw_len);
                }
                ControlFlow::Break(len) => {
                    self.writer
                        .write_all(bytes.slice(..len))
                        .await
                        .map_err(FetchError::Write)?;
                    self.remain.record_writed(len);
                    break;
                }
            }
        }
        Ok(())
    }

    ///可暂停写入stream
    pub async fn stopable_fetch<S, F>(
        &mut self,
        stream: &mut S,
        condition: F,
    ) -> Option<Result<(), SubError<S, W>>>
    where
        S: DownloadStream,
        F: FnMut(u64) -> bool,
        SubError<S, W>: From<S::Error>,
        SubError<S, W>: From<W::Error>,
    {
        //while let Some(bytes) = stream.try_next().await?
        loop {
            let bytes = match stream.try_next().await {
                Ok(Some(b)) => b,
                Ok(None) => break,
                Err(e) => return Some(Err(SubError::NetWork(e, None))),
            };

            let raw_len = bytes.len();
            match self.remain.split_chunk(raw_len) {
                ControlFlow::Continue(()) => {
                    match self.writer.write_all(bytes).await {
                        Ok(()) => (),
                        Err(e) => return Some(Err(e.into())),
                    }

                    if condition(*self.write_process()) {
                        return None;
                    };
                }
                ControlFlow::Break(new_len) => {
                    let result = self.writer.write_all(bytes.slice(..new_len)).await;
                    return Some(Ok(()));
                }
            }
        }
        return Some(Ok(()));
    }

    ///写入stream的单个chunk
    ///
    pub async fn fetch_chunk<S>(
        &mut self,
        stream: &mut S,
    ) -> Result<ControlFlow<()>, FetchError<S, W>>
    where
        S: DownloadStream,
    {
        let Some(bytes) = stream.try_next().await.map_err(FetchError::Stream)? else {
            return Ok(ControlFlow::Break(()));
        };
        let cf = self
            .write_in_remain(bytes)
            .await
            .map_err(FetchError::Write)?
            .map_break(|_| {});
        Ok(cf)
    }

    ///结束值缓冲
    pub fn end_cache(&self) -> u64 {
        self.writer.process + self.remain.cache as u64
    }
}

struct WriteError<W: BufWriter>(pub W::Error);

impl<W: BufWriter, S: DownloadStream> From<WriteError<W>> for FetchError<S, W> {
    fn from(value: WriteError<W>) -> Self {
        Self::Write(value.0)
    }
}

// ///抽象
// mod cache_abstart{
//     trait RemainCache{
//         type Cell: Radium<Item = i64>

//         fn new(remain: &Self::Cell) -> Self;

//         ///可能是缓存或即使值
//         fn remain_cache(&self) -> i64;

//         fn remain(&self) -> &Self::Cell;
//     }

//     struct LocalRemain(radium::Isotope<i64>);

//     impl RemainCache for LocalRemain {
//         type Cell = radium::Isotope<i64>;

//         fn new(remain: &Self::Cell) -> Self {
//             Self(remain)
//         }

//         fn remain_cache(&self) -> i64 {
//             self.0.load(Ordering::Relaxed)
//         }

//         fn remain(&self) -> &Self::Cell {
//             &self.0
//         }
//     }

//     trait AtomCache: Radium {
//         fn cache(&self) -> Self::Item;
//     }
// }
