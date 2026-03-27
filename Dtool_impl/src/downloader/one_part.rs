use std::{cell::{Cell, UnsafeCell}, sync::{Arc, atomic::AtomicU64}};

use crate::downloader::{ httprequest::RequestInfo};

struct Download{
    info: RequestInfo,
    length: u64,
    progress: AtomicU64
}

struct Download2{
    info: RequestInfo,
    length: u64,
    progress: Cell<u64>
}

struct BaseInfo{
    info: RequestInfo,
    length: u64,
}

// impl Sender for &Cell<u64> {
//     fn send_progress(&mut self, chunk_len: usize) {
//         let a: &mut &Cell<u64> = self;
//         a.get_mut();
//     }
// }

// impl Syncer for &Cell<u64> {
//     fn update_remain(&mut self) -> u64 {
//         Arc
//         todo!()
//     }
    
// }

// struct RcProject{
//     info: &'a DownloadRequest,
//     length: &'a u64
//     progress:
// }

// impl Download {
//     ///
    
//     fn download_arc(self: Arc<Self>){
//         self.download_ref();
//     }

//     fn download_ref(&self){
//         todo!()
//     }
    
//     fn download_mut(&mut self);
// }


// ///单线程运行时
// struct DownloadUnSync(Download);

// impl !Sync for DownloadUnSync {}


// impl DownloadUnSync {
//     fn download(&self )
// }