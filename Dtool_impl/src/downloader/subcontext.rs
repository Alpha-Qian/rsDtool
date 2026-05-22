use std::{cmp::max, num::NonZero, str::Bytes, sync::atomic::Ordering};

use radium::Radium;

use crate::downloader::family::ThreadModel;



pub struct Context<F: ThreadModel>{
    remain: F::RefCounter<F::AtomicCell<i64>>,
    remain_cache: i64,
}

impl<F: ThreadModel> Context<F> {
    pub fn new(remain: F::RefCounter<F::AtomicCell<i64>>) -> Self{
        Self{
            remain,
            remain_cache: i64::MAX,
        }
    }

    pub fn split_chunk(&mut self, chunk_len: usize) -> Option<NonZero<usize>> {
        let write_len = max(chunk_len as i64, self.remain_cache);//预计写入的字节数
        
        let old_remain = self.remain.fetch_sub(write_len, order);
        
        self.remain_cache = old_remain - write_len;
        if old_remain <= 0 {
            return None
        };
        NonZero::new(write_len as usize)
    }

    pub fn split_slice(&mut self, slice: &[u8]) -> Option<&[u8]> {
        let new_len = self.split_chunk(slice.len())?;
        return slice[..new_len]
    }

    pub fn split_bytes(&mut self, bytes: Bytes) -> Option<Bytes> {
        let new_len = self.split_chunk(bytes.len())?;
        return bytes[..new_len]
    }

    pub fn remain_cache(&self) -> &i64{
        &self.remain_cache
    }

    pub fn remain(&self) -> &F::RefCounter<F::AtomicCell<i64>>{
        &self.remain
    }

    pub fn into_raw(self) -> (F::RefCounter<F::AtomicCell<i64>>, i64) {
        (self.remain, self.remain_cache)
    }
}

async fn adapt_download<F: ThreadModel>(context: Context<F>, )

