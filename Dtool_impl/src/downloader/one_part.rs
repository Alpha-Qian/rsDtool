use std::{cell::{Cell, UnsafeCell}, sync::{Arc, atomic::AtomicU64}};

use radium::Radium;
use crate::downloader::{ httprequest::RequestInfo};

struct Download<T>{
    info: RequestInfo,
    length: u64,
    progress: T
}

impl<T: Radium<Item = u64>> Download<T> {
    
}

struct BaseInfo{
    info: RequestInfo,
    length: u64,
}
