use std::{cell::{Cell, UnsafeCell}, sync::{Arc, atomic::AtomicU64}};

use radium::Radium;
use crate::downloader::{ httprequest::RequestInfo};
use super::family::{ThreadModel, ThreadLocal, ThreadSafe, Lockable, RefCounted, RefCounter, Mutex, AtomicCell};


struct Download<F: ThreadModel>{
    info: RequestInfo,
    length: u64,
    progress: F::AtomicCell<u64>
}
