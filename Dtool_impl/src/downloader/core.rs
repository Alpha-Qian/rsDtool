use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::Result;
use reqwest::Response;

use crate::cache::{Cacher, Writer};



/// 进度记录器，最简单的进度记录器只需要一个u64变量记录进度，
/// 如果需要跨线程同步，则可以使用Arc<AtomicU64>
pub trait ProgressRecorder {
    ///发送的是增量而不是总量
    fn update_progress(&mut self, chunk_len: usize);
}

/// 不但记录进度，还要检查剩余量
/// 通常需要两个u64变量来分别记录进度和结束位置
/// 如果需要，则可以将进度和结束位置分别使用原子变量
pub trait RemainGetter: ProgressRecorder {
    ///返回当前块剩余的总量,每次调用此函数的返回值必须只减少不增加
    fn get_remain(&mut self) -> u64; //i64?
}

// /// 标记trait
// pub trait GetRemainRetry: RemainGetter {}

pub async fn download(mut response: Response, writer: &mut impl Writer) -> Result<()> {
    while let Some(chunk) = response.chunk().await? {
        writer.write_all(chunk.as_ref()).await?;
    }
    Ok(())
}

pub async fn download_with_record_progress(
    mut response: Response,
    writer: &mut impl Writer,
    sender: &mut impl ProgressRecorder,
) -> Result<()> {
    while let Some(chunk) = response.chunk().await? {
        writer.write_all(chunk.as_ref()).await?;
        sender.update_progress(chunk.len());
    }
    Ok(())
}

pub async fn download_with_check_remain(
    mut response: Response,
    writer: &mut impl Writer,
    syncer: &mut impl RemainGetter,
) -> Result<()> {
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




async fn resume_unsupported_download(
    mut response: Response,
    cache: &mut impl Cacher,
    offset_from_current: u64,
) -> Result<()> {
    //let mut response: Response = response.into();
    let mut downloaded: u64 = 0;
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

impl ProgressRecorder for &AtomicU64 {
    fn update_progress(&mut self, chunk_len: usize) {
        self.fetch_add(chunk_len as u64, Ordering::Acquire)
    }
}

impl ProgressRecorder for u64 {
    fn update_progress(&mut self, chunk_len: usize) {
        *self += chunk_len as u64
    }
}

struct Progress(AtomicU64);

struct Remain(AtomicU64);

impl Remain {
    fn update(&self, chunk_len: usize){
        self.0.fetch_update(Ordering::Release, Ordering::Acquire, |remain|{
            Some(remain.checked_sub(chunk_len as u64).unwrap_or(0))
        })
    }
}