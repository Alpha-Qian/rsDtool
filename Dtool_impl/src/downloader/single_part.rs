//!单线程下载器
use std::{path::Path};
use tokio::io::AsyncWriteExt;
use reqwest::{self, Client, Response, Url};
use tokio::fs::File;
use anyhow::Result;
use crate::{cache::Writer};
use super::hander::Hander;
pub struct Builder {
    response: Response,
}

impl Builder {
    pub fn new(response: Response) -> Self {
        Self { response }
    }

    pub fn start<T: Writer>(self, writer: impl Writer + 'static) -> (impl Future + 'static, Hander<Share<T>>){
        let (a, b) = Hander::new(Share::new(writer));
        impl Writer for &Share<T> {
            type Error = T::Error;
            async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
                self.writer.write_all(buf).await
            }
        }
        async {

        }

    }
}
pub struct Share<T>{
    writer: T
}

impl<T> Share<T> {
    fn new(writer: T) -> Self {
        Self {
             writer,
        }
    }
    
    
}
async fn download(url: Url, client: &Client, path: &Path) -> Result<()>{
    let mut response = client.get(url).send().await?;
    let mut file = File::create(path).await?;
    while let Some(chunk) = response.chunk().await? {
        file.write_all(chunk.as_ref()).await?;
    }
    Ok(())
}



#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_download() {
        Path::new(std::env::temp_dir().as_os_str()).join("test.txt");
        let url = Url::parse("https://www.rust-lang.org/logos/rust-logo-512x512.png").unwrap();
        let client = Client::new();
        let path = Path::new(std::env::temp_dir().as_os_str()).join("test.txt");
        println!("{path:?}");
        let result = download(url, &client, &path).await;
        assert!(result.is_ok());
    }

}