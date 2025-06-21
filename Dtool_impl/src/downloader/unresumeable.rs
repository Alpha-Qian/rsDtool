use std::{
    io::SeekFrom, ops::Deref, path::Path, sync::{atomic::{AtomicU64, Ordering}, Arc}
};

use reqwest::{Client, Request, Response, Url};

use crate::{cache::{self, Cacher, Writer}, utils::request::GetResponse};
use super::core::{download, download_with_progress};
use super::hander::Hander;
use super::core::{ RemainGetter, ProgressRecorder};

pub struct Builder {
    response: Response,
}

impl Builder {

    fn new(response: Response) -> Self {
        Self { response }
    }

    fn writer_with_response<W: Writer>(self, f: impl FnOnce(&Response) -> W) -> Hander<Share> {
        self.start(f(&self.response))
    }

    fn writer_with_file_name<W: Writer>(self, file_name: &str, f: impl FnOnce(&str) -> W) -> Hander<Share> {
        self.start(f(todo!("file_name")))
    }

    fn start(self, writer: impl Writer + 'static) -> (impl Future + 'static, Hander<Share>){
        let (mut a, b) = Hander::new(Share{ progress: AtomicU64::new(0)});
        (
        async move{
            let mut writer = writer;
            download_with_progress(self.response, &mut writer, &mut a).await
        }, b)
    }
}

///用于共享下载状态信息
pub struct Share {
    progress: AtomicU64,
}

impl ProgressRecorder for Hander<Share> {
    fn update_progress(&mut self, chunk_len: usize) {
        self.progress.fetch_add(chunk_len, Ordering::Relaxed)
    }
}


