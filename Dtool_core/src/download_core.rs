use std::error::Error;
use std::{io::SeekFrom};
use std::ops::ControlFlow;

use futures_util::stream::StreamExt;
use bytes::Bytes;
use crate::cache::{Writer, Cacher};
use crate::error::DownloadCoreError::{self, *};

type DownloadResult<T, I, W> = Result<T, DownloadCoreError<I, W>>;



#[inline]
pub(crate) async fn download_once<S, B, E>(
    stream: &mut S,//todo 可以在download_once方法外使用StreamExt的Map方法把stream::item转换成Result<&[u8], DownloadCoreError>
    writer: &mut impl Writer,
    process_sync: &mut impl AsyncProcessSender,
    end_sync: &mut impl AsyncEndReciver,
) -> DownloadResult<()>
where
    S: StreamExt<Item = Result<B, E>> + Unpin,
    B: AsRef<[u8]>,
    E: Error
{
    while let Some(item) = stream.next().await{
        let chunk = item.map_err(InternetEorror)?;
        let (controlflow, chunk) = optional_take_prefix(chunk.as_ref(), get_remain_async(process_sync, end_sync).await);

        writer.write_all(chunk).await.map_err(op)?;
        process_sync.fetch_add(chunk.len() as u32).await;

        if let ControlFlow::Break(_) = controlflow { break }
    }
    Ok(())
}


#[inline]
pub(crate) async fn write_a_chunk(
    chunk: &[u8],
    writer: &mut impl Writer,
    process_sync: &mut impl AsyncProcessSender,
    end_sync: &mut impl AsyncEndReciver,
) -> DownloadResult<ControlFlow<()>, I, W>
{
    let remain = end_sync.get_end().await.zip(process_sync.get_process().await)
        .map(|(end, process)| end - process);
    let (c, chunk ) = optional_take_prefix(chunk, remain);
    writer.write_all(chunk).await.map_err(op)?;
    process_sync.fetch_add(chunk.len() as u32).await;
    Ok(c)
}

#[inline]
pub async fn jump_to_write_position(
    stream: &mut (impl StreamExt<Item = Result<Bytes, reqwest::Error>> + std::marker::Unpin),
    cacher: &mut impl Cacher,
    process_sync: &mut impl AsyncProcessSender,
    jump_to: u64,
) -> DownloadResult<impl Writer>{
    while let Some(item) = stream.next().await{
        let chunk = item?;

        let contr = optional_skip_prefix(chunk.as_ref(), jump_to as usize);

        let chunk_size = chunk.len();
        process_sync.fetch_add(chunk_size as u32).await;
        let process = process_sync.get_process().await;
        if process > jump_to {
            let writer = cacher.write_at(SeekFrom::Start(jump_to)).await;
            writer.write_all(&chunk[(jump_to - process) as usize..]).await?;
            return Ok(writer);
        }
    };
    Ok(())
}


pub fn optional_take_prefix(chunk:&[u8], len: Option<usize>) -> (ControlFlow<()>, &[u8]) {
    match len {
        Some(len) => take_prefix(chunk, len),
        None => (ControlFlow::Continue(()), chunk)
    }
}
pub fn optional_skip_prefix(chunk:&[u8], len: Option<usize>) -> ControlFlow<&[u8]> {
    match len {
        Some(len) => skip_prefix(chunk, len),
        None => ControlFlow::Continue(()),
    }
}

///用于处理字节流的函数
///取出前len个字节
pub fn take_prefix(chunk:&[u8], len: usize) -> (ControlFlow<()>, &[u8]) {
    if len > chunk.len() {
        (ControlFlow::Continue(()), chunk)
    } else {
        (ControlFlow::Break(()), &chunk[..len])
    }
    
}

///用于处理字节流的函数
///跳过前len个字节
pub fn skip_prefix(chunk:&[u8], len: usize) -> ControlFlow<&[u8]> {
    if len > chunk.len() {
        ControlFlow::Continue(())
    } else {
        ControlFlow::Break(&chunk[len..])
    }
}


trait AsyncProcessSender{
    async fn fetch_add(&mut self, len: u32);
    async fn get_process(&self) -> Option<u64>;
    async fn get_unwarp(&self) -> u64{
        self.get_process().await.unwrap()
    }
}

trait AsyncEndReciver{
    async fn get_end(&self) -> Option<u64>{
        None
    }
    
    async fn get_unwarp(&self) -> u64{
        self.get_end().await.unwrap()
    }
}
///同步的ProcessSender
trait ProcessSender{
    fn fetch_add(&mut self, len: u32);
    fn get_process(&self) -> Option<u64>;
    fn get_unwarp(&self) -> u64{
        self.get_process().unwrap()
    }
}
///同步的EndReciver
trait EndReciver{
    fn get_end(&self) -> Option<u64>{
        None
    }
    
    fn get_unwarp(&self) -> u64{
        self.get_end().unwrap()
    }
}
  
impl<T> AsyncProcessSender for T where T: ProcessSender{
    async fn fetch_add(&mut self, len: u32){
        self.fetch_add(len);
    }
    async fn get_process(&self) -> Option<u64>{
        self.get_process()
    }
}

impl<T> AsyncEndReciver for T where T: EndReciver{
    async fn get_end(&self) -> Option<u64>{
        self.get_end()
    }
}
async fn get_remain_async(process:&mut impl AsyncProcessSender, end:&mut impl AsyncEndReciver) -> Option<u64>{
    process.get_process().await.zip(end.get_end().await).map(|(process, end)| {
        end - process
    })
}

fn get_remain(process:&mut impl ProcessSender, end:&mut impl EndReciver) -> Option<u64>{
    process.get_process().zip(end.get_end()).map(|(process, end)| {
        end - process
    })
}

///将reqwest crate的Stream转换成本crate内的Stream
fn reqwest_stream_map(stream: impl StreamExt<Item = Result<impl AsRef<[u8]>, impl Error>>) -> impl StreamExt<Item = Result<Bytes, DownloadCoreError>> {
    stream.map(|item| {
        item.map_err(|e| DownloadCoreError::InternetError(e))
    })
}

#[cfg(test)]
mod tests{
    #[test]
    fn test() {
        println!("hello world");
    }
}

#[cfg(test)]
mod test{
    #[test]
    fn test(){
        println!("hello world");
    }
}
