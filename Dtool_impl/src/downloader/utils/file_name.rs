//!一些工具方法
use reqwest::{blocking::Response, Url};
use headers::{HeaderMapExt, ContentRange};
/// Get the file name from the URL.
pub fn get_file_name(url: &Url) -> Option<&str> {
    match url.path().rsplit('/').next().unwrap(){
        "" => None,
        s => Some(s)
    }
}

pub fn get_file_size(response: &Response) -> Option<u64> {
    response.headers().typed_get::<ContentRange>()?.bytes_len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_file_name() {
        let url = Url::parse("https://www.rust-lang.org/a/").unwrap();
        println!("file name: {:?}", get_file_name(&url));
        assert_eq!(get_file_name(&url), Some("a"));
    }
}