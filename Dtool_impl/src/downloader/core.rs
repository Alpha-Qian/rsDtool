use core::sync;
use std::ops::ControlFlow;
use std::sync::Arc;

use reqwest::Response;
use anyhow::{Result};

use crate::utils::request::GetResponse;
use crate::cache::{Cacher, Writer};



/// 进度记录器，最简单的进度记录器只需要一个u64变量记录进度，
/// 如果需要跨线程同步，则可以使用Arc<AtomicU64>
pub trait ProgressRecorder{
    ///发送的是增量而不是总量
    fn update_progress(&mut self, chunk_len: usize);
}

/// 不但记录进度，还要检查剩余量
/// 通常需要两个u64变量来分别记录进度和结束位置
/// 如果需要，则可以将进度和结束位置分别使用原子变量
pub trait RemainGetter: ProgressRecorder{
    ///返回当前块剩余的总量
    fn get_remain(&mut self) -> u64;
}

pub async fn download<T: Writer>(mut response: Response, writer: &mut T) -> Result<()> {
    //let mut response: Response = response.into();
    while let Some(chunk) = response.chunk().await? {
        writer.write_all(chunk.as_ref()).await?;
    }
    Ok(())
}

pub async fn download_with_progress(mut response: Response, writer: &mut impl Writer, sender: &mut impl ProgressRecorder) -> Result<()>{
    //let mut response: Response = response.into();
    while let Some(chunk) = response.chunk().await? {
        writer.write_all(chunk.as_ref()).await?;
        sender.update_progress(chunk.len());
    }
    Ok(())
}

pub async fn download_with_check_remain(mut response: Response, writer: &mut impl Writer, syncer: &mut impl RemainGetter) -> Result<()>{
    //let mut response: Response = response.into();
    while let Some(chunk) = response.chunk().await? {
        let chunk = chunk.as_ref();
        let remian = syncer.get_remain();
        if chunk.len() > remian as usize {
            let splited_chunk = &chunk[..remian as usize];
            writer.write_all(splited_chunk).await?;
            syncer.update_progress(splited_chunk.len());
            return Ok(());
        }
        writer.write_all(chunk).await?;
        syncer.update_progress(chunk.len());
    }
    Ok(())
}

async fn resume_unsupported_download(mut response: Response, cache: &mut impl Cacher, offset_from_current: u64) -> Result<()>{
    //let mut response: Response = response.into();
    let mut downloaded : u64 = 0;
    while let Some(chunk) = response.chunk().await? {
        let chunk = chunk.as_ref();
        let remian = offset_from_current - downloaded;
        if chunk.len() > remian as usize {
            let splited_chunk = &chunk[..remian as usize];
            
            downloaded += splited_chunk.len() as u64;
            return Ok(());
        }
        downloaded += chunk.len() as u64;
    }
    Ok(())
}