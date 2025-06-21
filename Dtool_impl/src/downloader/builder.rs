use headers::{ContentRange, HeaderMapExt, Range};
use reqwest::{Client, Method, Request, Response, Url};
use anyhow::{Result, Error};

use crate::cache::Cacher;

use super::unresumeable::Share as UnRangeableDownloader;
use super::muti_part::Downloader as MultiPartDownloader;
use super::single_part::Share as SinglePartDownloader;

enum Builder<'a> {
    Rangeable(RangeableBuilder<'a>),
    UnRangeable(UnRangeableBuilder<'a>),
}

impl<'a> Builder<'a>{
    pub async fn new(url: Url, client: &'a Client,) -> Result<Self> {
        let mut request = Request::new(Method::GET, url.clone());
        request.headers_mut().typed_insert(Range::bytes(0..)?);
        let response = client.execute(request).await?.error_for_status()?;

        if response.status().as_u16() == 206 {
            let total_size = response.headers().typed_get::<ContentRange>().unwrap().bytes_len().unwrap();
            Ok(Self::Rangeable(RangeableBuilder::new(response, client, total_size)))
        } else {
            Ok(Self::UnRangeable(UnRangeableBuilder::new(url, client)))
        }
    }

}

struct RangeableBuilder<'a>{
    response: Response,
    client: &'a Client,
    total_size: u64,
}

impl<'a> RangeableBuilder<'a> {
    fn new(response: Response, client: &'a Client, total_size: u64) -> Self {
        Self {
            response,
            client,
            total_size,
        }
    }

    fn muti_part_download(self) {
        MultiPartDownloader::with_response(response, client, total_size)
    }

    fn resumeable_single_thread_download(self) {
        unimplemented!()
    }
}

struct UnRangeableBuilder<'a>{
    response: Response,
    client: &'a Client,
    total_size: Option<u64>,
}

impl<'a> UnRangeableBuilder<'a> {
    fn new(response: Response, client: &'a Client) -> Self {
        Self {
            response,
            client,
            total_size: None,
        }
    }

    fn unresumeable_download<C: Cacher>(self) -> UnRangeableDownloader<>{
        UnRangeableDownloader::with_response(response, cache)
    }
}

struct MultiPartBuilder{}

struct SinglePartBuilder{}

struct UnResumeableBuilder{}

