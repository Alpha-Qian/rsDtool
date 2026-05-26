use std::{error::Error, fmt::Display};

use reqwest::Client;

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

// impl StreamFamily for &Client {
//     ///Into Stream Error
//     type Error = reqwest::Error;

//     async fn new_stream(self, info: RequestInfo) -> impl DownloadStream<Error = Self::Error> {
//         let response = self.execute(info.into_request()).await.unwrap();
//         response.bytes_stream()
//     }
// }

///防止重复impl 的new type包装器
#[derive(Debug)]
struct IntoNetWorkError<E>(E);

impl<T: Display> Display for IntoNetWorkError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: Error + 'static> Error for IntoNetWorkError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

impl<T> From<T> for IntoNetWorkError<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T, S, W> From<IntoNetWorkError<T>> for SubError<S, W>
where
    S: DownloadStream<Error = T>,
    W: BufWriter,
{
    fn from(value: IntoNetWorkError<T>) -> Self {
        Self::NetWork(value.0, None)
    }
}

#[derive(Debug)]
struct IntoWriterError<E>(E);

impl<T: Display> Display for IntoWriterError<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: Error + 'static> Error for IntoWriterError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.0)
    }
}

impl<T, S, W> From<IntoWriterError<T>> for SubError<S, W>
where
    W: BufWriter<Error = T>,
    S: DownloadStream,
{
    fn from(value: IntoWriterError<T>) -> Self {
        Self::Writer(value.0)
    }
}
