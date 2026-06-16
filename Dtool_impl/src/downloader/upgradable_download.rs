//!由于不确定是否可续传，可升级到多线程下载的临时单线程下载器


use std::ops::Deref;

use reqwest::{Client, Response};

use crate::base::{
    family::ThreadModel,
    pwriter::BufWriter, request_info::RequestInfo, subcontext::{RemainWriter, Writer},
};


struct Builder {
    info: RequestInfo,
    client: Client,
    length: u64,
}

impl Builder {

    ///以单线程方式下载可升级连接
    fn download<M: ThreadModel>(
        self,
        writer: &impl BufWriter,
    ) -> (M::RefCounter<Share<M>>, impl Future) {
        todo!()
    }

    fn upgradeable_download(self)
}

///
struct Downloader<M: ThreadModel>{
    share
}


///返回Future和 first_remain
/// Future在下载完成时返回None，升级成功时返回Some
async fn upgrading_download<M: ThreadModel, W: BufWriter>(
    info: RequestInfo,
    client: Client,
    response: Response,

    length: u64,
    remain: M::RefCounter<M::AtomicCell<u64>>,
    writer: &W,
) -> Option<Upgrading<'_, M, W>>
{
    let stream = response.bytes_stream();
    let remain_writer = RemainWriter::new(remain.deref(), writer);

    remain_writer.fetch_chunk(stream)
}


struct Upgrading<'a, M: ThreadModel, W: BufWriter>{
    first_response: Response,
    first_remain: M::RefCounter<M::AtomicCell<i64>>,
    first_end: u64,

    second_response: Response,
    second_remain: u64,
    length: u64,

    writer: &'a W,
    info: RequestInfo,
    client: Client,
}
