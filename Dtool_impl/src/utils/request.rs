//! GetRequest: 用于构建标准的测试范围请求，请求附带Range头部，范围为整个文件
//! GetResponse: 发送GetRequest得到的响应，确保响应是由发送GetRequest得到的，确保响应在GetRequest发送后不被修改，以免发生逻辑错误
//! 

use std::ops::Deref;

use reqwest::{Client, Request, Response, Url, header::HeaderMap};
use reqwest::Method;
use headers::{Range, HeaderMapExt};
use anyhow::{Error, Ok, Result};

///范围请求，范围为整个文件
pub struct GetRequest {
    inner: Request,
}

impl GetRequest {
    pub fn new(request: Request) -> Result<Self> {
        Self::try_from(request)
    }

    pub fn new_forse(mut request: Request) -> Self {
        *request.method_mut() = Method::GET;
        request.headers_mut().typed_insert(Range::bytes(0..)?);
        Self { inner: request }
    }

    pub(crate) fn with_raw(request: Request) -> Self {
        Self { inner: request }
    }

    pub(crate) fn with_parts(url: Url, headers: HeaderMap) -> Self {
        let mut request = Request::new(Method::GET, url);
        *request.headers_mut() = headers;
        Self::new(request)
    }
}

impl TryFrom<Request> for GetRequest {
    type Error = Error;

    fn try_from(request: Request) -> Result<Self> {
        if request.method() == Method::GET {
            if request.headers().typed_get::<Range>().is_none() {
                request.headers_mut().typed_insert(Range::bytes(0..)?);
                Ok(Self { inner: request })
            } else {
                panic!("Range header already exists")
            }
        } else {
            panic!("Not a GET request")
        }
    }
}

impl Into<Request> for GetRequest {
    fn into(self) -> Request {
        self.inner
    }
}


// ///发送['GetRequest']得到的响应
// struct FullResponse {
//     inner: Response,
// }

// impl FullResponse {
//     fn new(response: Response) -> Self {
//         Self { inner: response }
//     }
// }

// impl Into<Response> for FullResponse {
//     fn into(self) -> Response {
//         self.inner
//     }
// }

pub enum GetResponse {
    Range(PartialResponse),
    UnRange(NonPartialResponse),
}


//我们不能确定Response是否是get请求得到的
// impl TryFrom<Response> for GetResponse {
//     type Error = Error;

//     fn try_from(response: Response) -> Result<Self> {
//         if response.status().as_u16() == 206 {
//             Ok(Self::Range(PartialResponse::new(response)?))
//         } else{
//             response.error_for_status()?;
//             Ok(Self::UnRange(NonPartialResponse { inner: response }))
//         }
//     }
// }

impl Deref for GetResponse {
    type Target = Response;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Range(partial) => &partial.inner,
            Self::UnRange(non_partial) => &non_partial.inner,
        }
    }
}

impl AsRef<Response> for GetResponse {
    fn as_ref(&self) -> &Response {
        match self {
            Self::Range(partial) => &partial.inner,
            Self::UnRange(non_partial) => &non_partial.inner,
        }
    }
}

impl Into<Response> for GetResponse {
    fn into(self) -> Response {
        match self {
            Self::Range(partial) => partial.inner,
            Self::UnRange(non_partial) => non_partial.inner,
        }
    }
}


///范围请求的206响应
pub struct PartialResponse {
    inner: Response,
}

impl PartialResponse {
    fn new(response: Response) -> Result<Self> {
        Self::try_from(response)
    }
}

impl TryFrom<Response> for PartialResponse {
    type Error = Error;

    fn try_from(response: Response) -> Result<Self> {
        if response.status().as_u16() == 206 {
            Ok(Self { inner: response })
        } else {
            Err(Error::msg("Not a partial response"))
        }
    }
}

impl Into<Response> for PartialResponse {
    fn into(self) -> Response {
        self.inner
    }
}

///非范围请求的2xx响应
/// 表示服务器拒绝了范围请求
pub struct NonPartialResponse {
    inner: Response,
}
