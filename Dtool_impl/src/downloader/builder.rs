use headers::{ContentRange, HeaderMapExt, Range};
use reqwest::header::HeaderMap;
use reqwest::{Client, Method, Request, Response, Url, Version};
use anyhow::{Error, Ok, Result};

//use crate::cache::Cacher;
use crate::downloader::httprequest::RequestInfo;


enum Builder<'a> {
    Rangeable(RangeableBuilder<'a>),
    UnRangeable(UnRangeableBuilder<'a>),
}

impl<'a> Builder<'a>{
    pub async fn new(download_info: RequestInfo, client: &'a Client, pre_set_task_num: Option<usize>) -> Result<Self> {
        let mut request: Request = download_info.build_request();
        request
            .headers_mut()
            .typed_insert(Range::bytes(0..)?);

        let response = client.execute(request)
            .await?
            .error_for_status()?;
        if response.status().as_u16() == 206 
            && let Some(content_range) = response.headers().typed_get::<ContentRange>()
            && let Some(total_size) = content_range.bytes_len()
        {
            Ok(Self::Rangeable(RangeableBuilder::new(response, client, total_size)))
        } else {
            todo!()
        }
        
        // else if let Some(length) = response.content_length()
        //     &&{
        //         let mut request = download_info.build_request();
        //         request
        //             .headers_mut()
        //             .typed_insert(Range::bytes((length / pre_set_task_num.unwrap_or(2))..)?);
        //         let response2 = client.execute(request)
        //             .await?
        //             .error_for_status()?;
        //         response2.status().as_u16() == 206
        //     }
        // {
        //     todo!()
        // } else {
        //     todo!()
        // }

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
        //MultiPartDownloader::with_response(response, client, total_size)
    }

    fn single_part_download(self) {
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

    // fn unresumeable_download<C: Cacher>(self) -> UnRangeableDownloader<>{
    //     //UnRangeableDownloader::with_response(response, cache)
    // }
}

struct MultiPartBuilder{}

struct SinglePartBuilder{}

struct UnResumeableBuilder{}



