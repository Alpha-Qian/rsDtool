use std::error::Error;

use bytes::Bytes;
use futures::TryStream;

pub trait DownloadStream: TryStream<Ok = Bytes> + Unpin {}

impl<T> DownloadStream for T where T: TryStream<Ok = Bytes > + Unpin {}



pub enum NetWorkErrorType{
    ///不知道
    Unkown,
    ///与服务器的连接出现了问题
    Connect,
    ///服务器存在问题
    Sever,
    ///客户端存在问题
    Local,
    ///互联网连接存在问题
    Internet,
}

trait NetWorkError{
    fn type(&self) -> NetWorkErrorType {
        NetWorkErrorType::Unkown
    }
}


impl NetWorkError for reqwest::Error {}
