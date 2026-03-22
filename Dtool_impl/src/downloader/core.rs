use std::{any::Any, sync::atomic::{AtomicU64, Ordering}};

use anyhow::Result;
use reqwest::Response;

use crate::cache::{Cacher, Writer};
use core::{sync, task::Waker};


// pub trait Stategy: Syncer + Cacher{

// }

// /// 进度记录器，最简单的进度记录器只需要一个u64变量记录进度，
// /// 如果需要跨线程同步，则可以使用Arc<AtomicU64>
// pub trait Sender {

//     fn pre_write(&mut self, chunk_len: usize);
//     ///发送的是增量而不是总量
//     fn send_progress(&mut self, chunk_len: usize);
// }

// /// 不但记录进度，还要检查剩余量
// /// 通常需要两个u64变量来分别记录进度和结束位置
// /// 如果需要，则可以将进度和结束位置分别使用原子变量
// pub trait Syncer{
    
//     fn get_remain(&self) -> u64;

//     fn pre_write(&mut self, chunk_len: usize) -> u64

//     ///返回当前块剩余的总量,每次调用此函数的返回值必须只减少不增加
//     fn update_remain(&mut self, chunk_len: usize) -> u64; //i64?
// }

// impl<T: Syncer> Sender for T
// {

//     fn pre_write(&mut self, chunk_len: usize) {
        
//     }
//     fn send_progress(&mut self, chunk_len: usize) {
//         self.update_remain(chunk_len);
//     }
// }




// pub async fn download(mut response: Response, writer: &mut impl Writer) -> Result<()> {
//     while let Some(chunk) = response.chunk().await? {
//         writer.write_all(chunk.as_ref()).await?;
//     }
//     Ok(())
// }

// pub async fn download_with_record_progress(
//     mut response: Response,
//     writer: &mut impl Writer,
//     sender: &mut impl Sender,
// ) -> Result<()> {
//     while let Some(chunk) = response.chunk().await? {
//         sender.pre_write(chunk.len());
//         writer.write_all(chunk.as_ref()).await?;
//         sender.send_progress(chunk.len());
//     }
//     Ok(())
// }

// pub async fn download_with_check_remain(
//     mut response: Response,
//     writer: &mut impl Writer,
//     syncer: &mut impl Syncer,
// ) -> Result<()> {

//     let remain = syncer.get_remain();
//     while let Some(chunk) = response.chunk().await? {
//         let chunk = chunk.as_ref();
        
//         if chunk.len() > remian as usize {
//             let splited_chunk = &chunk[..remian as usize];

//             syncer.pre_write(splited_chunk.len());
//             writer.write_all(splited_chunk).await?;
//             remain = syncer.update_remain(splited_chunk.len());

//             return Ok(());
//         }

//         syncer.pre_write(chunk.len());
//         writer.write_all(chunk).await?;
//         remain = syncer.update_remain(chunk.len());
//     }
//     Ok(())
// }


trait UnboundDownload{
    async fn handle(&mut self, chunk: &[u8]);
}

trait BoundDownload{
    async fn get_remain(&mut self) -> u64;
    async fn handle(&mut self, chunk: &[u8]) -> u64;//return Remain
}

//无下载结束限制
pub async fn unbound_download(
    mut response: Response,
    hander: impl UnboundDownload
) -> Result<()> {
    while let Some(chunk) = response.chunk().await? {
        hander.handle(chunk.as_ref()).await
    }
    Ok(())
}

//有下载结束限制
pub async fn download_with_check_remain(
    mut response: Response,
    hander: impl BoundDownload
) -> Result<()> {

    let remain = hander.get_remain().await;
    while let Some(chunk) = response.chunk().await? {
        let chunk = chunk.as_ref();
        if chunk.len() > remian as usize {
            let splited_chunk = &chunk[..remian as usize];
            remain = hander.handle(splited_chunk.as_ref()).await;
            break;
        }

        remain = hander.handle(splited_chunk.as_ref()).await;
    }
    Ok(())
}

// async fn resume_unsupported_download(
//     mut response: Response,
//     cache: &mut impl Cacher,
//     offset_from_current: u64,
// ) -> Result<()> {
//     //let mut response: Response = response.into();
//     let mut downloaded: u64 = 0;
//     while let Some(chunk) = response.chunk().await? {
//         let chunk = chunk.as_ref();
//         let remian = offset_from_current - downloaded;
//         if chunk.len() > remian as usize {
//             let splited_chunk = &chunk[..remian as usize];

//             downloaded += splited_chunk.len() as u64;
//             return Ok(());
//         }
//         downloaded += chunk.len() as u64;
//     }
//     Ok(())
// }
