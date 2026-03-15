use anyhow::{Error, Result};
use headers::{ContentRange, HeaderMapExt, Range};
use reqwest::header::HeaderMap;
use reqwest::{Client, Method, Request, Response, Url, Version};

///作为crate的公共api
#[derive(Clone, Debug)]
pub struct DownloadRequest {
    url: Url,
    headers: HeaderMap,
    version: Version,
}

impl DownloadRequest {
    pub fn new(url: Url, headers: HeaderMap) -> Self {
        Self {
            url,
            headers,
            version: Version::default(),
        }
    }

    pub fn new_request(&self) -> Request {
        self.clone().into()
    }

    pub fn with_parts(url: Url, headers: HeaderMap, version: Version) -> Result<Self,()> {
        if headers.typed_get::<Range>().is_none() {
            Ok(Self {
                url,
                headers,
                version,
            })
        } else {
            Err(())
        }
    }

    pub fn with_url(url: Url) -> Self {
        let mut headers = HeaderMap::new();
        headers.typed_insert(Range::bytes(..).unwrap());
        Self::with_parts(url, HeaderMap::new(), Version::default()).unwrap()
    }

    pub fn into_raw(self) -> (Url, HeaderMap, Version) {
        (self.url, self.headers, self.version)
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn url_mut(&mut self) -> &mut Url {
        &mut self.url
    }

    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    pub fn version(&self) -> Version {
        self.version
    }

    pub fn version_mut(&mut self) -> &mut Version {
        &mut self.version
    }
}

impl Into<Request> for DownloadRequest {
    fn into(self) -> Request {
        let mut request = Request::new(Method::GET, self.url);
        *request.headers_mut() = self.headers;
        request
    }
}
