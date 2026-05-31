
use futures::Stream;
use reqwest::{Client, Request, Response, StatusCode, header::HeaderMap};

use super::error::IntoNetWorkError;
use crate::downloader::{
    error::DownloaderError, group_impl::DownloadStream, httprequest::RequestInfo,
};
trait StreamFamily {
    ///Into Stream Error
    type Error;

    async fn new_stream<S>(&self, info: RequestInfo) -> Result<S, IntoNetWorkError<Self::Error>>
    where
        S: DownloadStream;

    fn insepect_response<T: StreamFamily>(self) -> T;

    fn insepect_request_info<T: StreamFamily>(self) -> T;

    fn map_err<T: StreamFamily>(self, f: ) -> T;

    fn and_then<T: StreamFamily>(self, f:)
}


fn handle_response(
    response: Response,
) -> Result<impl DownloadStream, IntoNetWorkError<reqwest::Error>> {
    //response.error_for_status_ref().map_err(op);
    Ok(response.bytes_stream())
}

enum StreamErrors {
    Fetech,
    HeaderError(Response),
    StreamError,
}


async fn check_network_error(response: &Response) -> Result<(), IntoNetWorkError<reqwest::Error>>{
    response.error_for_status_ref()
}

// fn check_download_error<R: AsResponse>(response: &R)

fn check_download_error(status: StatusCode, headers: &HeaderMap) -> Result<(), DownloaderError>{
    if status != StatusCode::PARTIAL_CONTENT{
        return Err(DownloaderError::ErrorSuccesStatus(()))
    }
}
