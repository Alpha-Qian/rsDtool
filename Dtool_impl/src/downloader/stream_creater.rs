use std::{error::Error, fmt::Display};

use reqwest::Client;

use super::error::IntoNetWorkError;
use crate::downloader::{
    error::SubError, group_impl::DownloadStream, httprequest::RequestInfo, pwriter::BufWriter,
};
trait StreamFamily {
    ///Into Stream Error
    type Error;

    async fn new_stream<S>(self, info: RequestInfo) -> Result<S, IntoNetWorkError<Self::Error>>
    where
        S: DownloadStream;
}

impl StreamFamily for &Client {
    ///Into Stream Error
    type Error = reqwest::Error;

    async fn new_stream(self, info: RequestInfo) -> impl DownloadStream<Self::Error> {
        let response = self.execute(info.into_request()).await.unwrap();
        response.bytes_stream()
    }
}
