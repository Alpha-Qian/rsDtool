//!探测目标链接是否支持续传

use std::{cell::{Cell, UnsafeCell}, sync::{Arc, atomic::AtomicU64}};

use radium::Radium;
use crate::downloader::{ download_group::{GroupExt, Slot}, httprequest::RequestInfo};
use super::family::{ThreadModel, ThreadLocal, ThreadSafe, Lockable, RefCounted, RefCounter, Mutex, AtomicCell};



struct TestUrl<'a, F: ThreadModel, E: GroupExt<F>>{
    request_info: RequestInfo,
    slot: Slot<'a, F, E>,
}


