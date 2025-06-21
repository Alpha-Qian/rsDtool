//!多线程下载器
use std::{env::consts, sync::atomic::AtomicU64};
use Dtool_core;
use reqwest::{self, Client, Response, Url};
use tokio::{self, task::JoinSet};
use parking_lot::RwLock;
use anyhow::Result;
use crate::{cache::Cacher};
use super::core::download_with_check_remain;
struct Block {
    progress: AtomicU64,
    end: u64,
}

impl ProcessSender for *const Block {
    fn read(&self) -> Option<u64> {
        Some(unsafe {(**self).progress.load(std::sync::atomic::Ordering::SeqCst)})
    }

    fn fetch_add(&mut self, len: u32) {
        unsafe {
            (**self).progress.fetch_add(len as u64, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

impl EndReciver for *const Block {
    fn read(&self) -> Option<u64> {
        Some(unsafe {(**self).end})
    }
}



impl Block {
    fn new(progress: u64, end: u64) -> Self {
        Self {
            progress: AtomicU64::new(progress),
            end,
        }
    }

    fn with_total_size(total_size: u64) -> Self {
        Self::new(0, total_size)
    }

    fn as_ptr(&self) -> *const Self {
        self as *const Self
    }
}

struct Builder{
    response: Response,
    total_size: u64,
}

impl Builder {
    
}

pub struct Downloader{
    client: Client,
    waiting_blocks: Vec<Box<Block>>,//后进后出
    progress: AtomicU64,
    total_size: u64,
}

impl Downloader{
    fn new(url: Url, client: &'a Client, total_size: u64) -> Self {
        Self{
            client,
            progress: AtomicU64::new(0),
            total_size,
            blocks: RwLock::new(vec![Block::with_total_size(total_size)]),
        }
    }

    pub fn pop_block(&mut self) -> Option<Box<Block>> {
        self.waiting_blocks.pop()
    }

    fn push_block(&mut self, block: Box<Block>) {
        self.waiting_blocks.push(block)
    }

    pub fn download_remain_block(&mut self) -> Option<impl Future> {
        let block = self.pop_block()?;
        Some(Downloader::download_block(block))
    }

    fn download_block(block: Box<Block>) -> impl Future {
        async move {
            unimplemented!()
        }
    }
}
