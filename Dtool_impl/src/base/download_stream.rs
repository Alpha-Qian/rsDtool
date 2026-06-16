use bytes::Bytes;
use futures::TryStream;

pub trait DownloadStream: TryStream<Ok = Bytes> {}

impl<T> DownloadStream for T where T: TryStream<Ok = Bytes> {}
