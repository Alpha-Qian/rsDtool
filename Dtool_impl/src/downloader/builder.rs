use reqwest::{Client, Response};

use crate::base::{family::ThreadModel, request_info::RequestInfo, segment::Segment};


///发送嗅探请求
async fn get_url_header(info: RequestInfo, client: Client) -> Result<Builder, reqwest::Error>{
    let request = info.build_request();
    request.headers_mut().insert(todo!(), todo!())
    pub mod group_impl;

    let response = client
        .execute(info.build_request())
        .await?
        .error_for_status()?;

    todo!()

}

enum Builder {
    CanRange(CanRange),
    GetLength(GetLength),
    UnKonwLength(UnKonwLength),
}

///可续传 -> 多线程下载器
pub struct CanRange(RequestInfo, Client, Response, u64);

///不确定是否可续传 -> 可尝试升级为多线程的单线程下载器
pub struct GetLength(RequestInfo, Client, Response, u64);

///确定不可续传 -> 单线程下载器
pub struct UnKonwLength(RequestInfo, Client, Response);

impl CanRange {
    ///un upgrade able
    fn unstopable_download() {
        todo!()
    }

    // ///upgrade able 如果是否可升级已经确定了，为什么还需要这个
    // fn stopable_download() {
    //     todo!()
    // }

    fn muti_thread_download() {
        todo!()
    }
}

impl GetLength {
    fn unstopable_download() {
        todo!()
    }

    fn ungradeable_downlao() {
        todo!()
    }
}

impl UnKonwLength {
    fn unstopable_download() {}
}
