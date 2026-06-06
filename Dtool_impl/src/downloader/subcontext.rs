use futures::{Stream, TryStreamExt};
use radium::Radium;
use std::ops::{Deref, Sub};
use std::result;
use std::{ops::ControlFlow, sync::atomic::Ordering};

use crate::downloader::{
    error::SubError, family::ThreadModel, group_impl::DownloadStream, pwriter::BufWriter,
};

pub struct Remain<F>
where
    F: ThreadModel,
{
    remain: F::RefCounter<F::AtomicCell<i64>>,
    cache: i64,
}

impl<F> Remain<F>
where
    F: ThreadModel,
{
    pub fn new(remain: F::RefCounter<F::AtomicCell<i64>>) -> Self {
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

    pub fn remain(&self) -> &F::RefCounter<F::AtomicCell<i64>> {
        &self.remain
    }

    pub fn into_raw(self) -> (F::RefCounter<F::AtomicCell<i64>>, i64) {
        (self.remain, self.cache)
    }

    pub fn flash_cache(&mut self) {
        self.cache = self.remain.load(Ordering::Acquire)
    }
}

// async fn adapt_download<S, W, F>(
//     context: Remain<F>,
//     mut write_process: u64,
//     stream: &mut S,
//     writer: &mut W,
//     token: impl AbortToken,
// ) -> Result<(), SubError<S, W>>
// where
//     S: DownloadStream,
//     W: BufWriter,
//     F: ThreadModel,
// {
//     while let Some(bytes) = stream.try_next().await? {
//         let len = bytes.len();
//         if let Some(write_len) = context.splited_len::<F>(len) {
//             let write_bytes = bytes.slice(..write_len);
//             writer.pwrite(write_process, bytes).await?;
//             write_process += write_len;
//         } else {
//             return Ok(());
//         }
//     }
//     if context.remain_cache > 0 {
//         panic!("unexcepted response EOF")
//     }
//     Ok(())
// }
// pub struct RawWriter<'a, W>{
//     pwriter: &'a W,
//     process: u64,
// }

pub struct Writer<'a, W> {
    pwriter: &'a W,
    process: u64,
}

impl<'a, W> Writer<'a, W>
where
    W: BufWriter,
{
    pub fn new(pwriter: &'a mut W, process: u64) -> Self {
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
    ) -> Option<Result<(), SubError<S, W>>>
    where
        S: DownloadStream,
        SubError<S, W>: From<S::Error>,
        SubError<S, W>: From<W::Error>,
    {
        loop {
            let bytes = match stream.try_next().await {
                Ok(Some(bytes)) => bytes,
                Ok(None) => return Some(Ok(())),

                Err(e) => return Some(Err(e.into())),
            };

            self.write_all(bytes).await?;
            if condition(self.process) {
                return None;
            }
        }
        //Some(Ok(()))
    }

    pub async fn fetch_all<S: DownloadStream>(
        &mut self,
        mut stream: S,
    ) -> Result<(), SubError<S, W>>
    where
        SubError<S, W>: From<S::Error>,
        SubError<S, W>: From<W::Error>,
    {
        while let Some(bytes) = stream.try_next().await? {
            self.write_all(bytes).await?
        }
        Ok(())
    }

    pub async fn fetech_chunk<S: DownloadStream>(
        &mut self,
        stream: &mut S,
    ) -> Result<ControlFlow<()>, SubError<S, W>>
    where
        SubError<S, W>: From<S::Error>,
        SubError<S, W>: From<W::Error>,
    {
        let bytes = match stream.try_next().await? {
            Some(b) => b,
            None => return ControlFlow::Break(()),
        };
        let len = bytes.len();
        self.write_all(bytes).await?;
        Ok(ControlFlow::Continue(()))
    }

    pub async fn fetch_chunk_in_remain<S: DownloadStream>(
        &mut self,
        stream: &mut S,
        remain: i64,
    ) -> Result<ControlFlow<()>, SubError<S, W>>
    where
        SubError<S, W>: From<S::Error>,
        SubError<S, W>: From<W::Error>,
    {
        let bytes = match stream.try_next().await {
            Ok(Some(b)) => b,
            Ok(None) => return Ok(ControlFlow::Break(())),
            Err(e) => return Err(e.into()),
        };
        let raw_len = bytes.len();
        if raw_len < remain as usize {
            self.write_all(bytes).await?;
            return Ok((raw_len, false));
        } else {
            self.write_all(bytes.slice(..remain)).await?;
            return Ok((remain, true));
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
    pub fn new(writer: Writer<'a, W>, context: Remain<M>) -> Self {
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

    pub async fn fetch_stream<S>(&mut self, mut stream: S) -> Result<(), SubError<S, W>>
    where
        S: DownloadStream,
        SubError<S, W>: From<S::Error>,
        SubError<S, W>: From<W::Error>,
    {
        while let Some(bytes) = stream.try_next().await? {
            let raw_len = bytes.len();
            match self.remain.split_chunk(bytes.len()) {
                ControlFlow::Continue(()) => {
                    self.writer.write_all(bytes).await?;
                    self.remain.record_writed(raw_len);
                }
                ControlFlow::Break(len) => {
                    self.writer.write_all(bytes.slice(..len)).await?;
                    self.remain.record_writed(len);
                    break;
                }
            }
        }
        Ok(())
    }

    pub async fn fetch_conditional<S, F>(
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

    pub async fn fetch_chunk<S>(
        &mut self,
        stream: &mut S,
    ) -> Result<ControlFlow<usize>, SubError<S, W>>
    where
        S: DownloadStream,
        SubError<S, W>: From<S::Error>,
        SubError<S, W>: From<W::Error>,
    {
        let Some(bytes) = stream.try_next().await? else {
            return Ok(ControlFlow::Break(()));
        };
        let cf = self.write_in_remain(bytes).await?.map_break(|_| {});
        Ok(cf)
    }

    pub fn end_cache(&self) -> u64 {
        self.writer.process + self.remain.cache as u64
    }
}
