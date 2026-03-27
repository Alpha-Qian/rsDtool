use std::{any::Any, ops::RangeBounds, str::Bytes, sync::atomic::{AtomicU64, Ordering}};

use anyhow::Result;
use reqwest::{Client, Response, header::{Entry, HeaderValue}};

use crate::{cache::{Cacher, Writer}, downloader::httprequest::RequestInfo};
use core::{sync, task::Waker};
use headers::{ETag, Header, HeaderMapExt, IfMatch, IfRange, LastModified, Range};




trait UnboundDownload{
    type Error;
    async fn handle(&mut self, chunk: &[u8]) -> Result<(), Self::Error>;
}

trait BoundDownload{
    type Error;
    async fn get_remain(&mut self) -> u64;
    async fn handle(&mut self, chunk: &[u8]) -> Result<u64, Self::Error>;//return Remain
}

//无下载结束限制
pub async fn unbound_download(
    mut response: Response,
    hander: impl UnboundDownload
) -> Result<()> {
    while let Some(chunk) = response.chunk().await? {
        hander.handle(chunk.as_ref()).await?
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

        if chunk.len() > remain as usize {
            let splited_chunk = &chunk[..remain as usize];
            remain = hander.handle(splited_chunk.as_ref()).await?;
            break;
        }

        remain = hander.handle(chunk).await?;
    }
    Ok(())
}




async fn send_first_request(info: &mut RequestInfo, client: Client, bound: impl RangeBounds<u64>) -> Result<Response>{
    let request = info.build_request();
    if request.headers_mut().contains_key(Range::name()) { panic!() }
    request.headers_mut().typed_insert(Range::bytes(bounds));

    let response = client.execute(request).await?;

    let etag = todo!();
    if let Some(etag) = info.headers.get_mut(etag) {
        etag = todo!()
    }

    
}

fn get_etag(response: Response){//} -> Option<HeaderValue>{
    let etag = response.headers().try_entry(ETag::name()).unwrap();

    let etag: HeaderValue = match etag {
        Entry::Occupied(v) => Some(v.get().clone()),
        _ => None
    }.unwrap();

    etag.as_bytes().starts_with("W/".as_bytes());
    todo!()
}

