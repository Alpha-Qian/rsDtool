use std::{cell::Cell, sync::atomic::{AtomicU64, Ordering}};

pub struct Block {//AtomicBlock
    pub start: u64,
    end: u64,
    remain: AtomicU64,
}

impl Block {
    pub fn progress(&self) -> u64 {
        let remain = self.remain.load(Ordering::Relaxed);
        self.end - remain
    }

    pub fn get_remain(&self) -> u64 {
        self.remain.load(Ordering::Relaxed)
    }

    // pub fn divition_remain(&self, num: usize){
    //     let remain = self.remain.load(Ordering::Relaxed);
    //     let block_size = remain / num as u64;
    //     self.remain.compare_exchange(remain, block_size, success, failure)
        
    // }
}

pub struct CellBlock{
    pub start: u64,
    end: u64,
    remain: Cell<u64>
}

impl CellBlock {
    pub fn progress(&self) -> u64 {
        let remain = self.remain.get();
        self.end - remain
    }

    pub fn get_remain(&self) -> u64 {
        self.remain.get()
    }

    // pub fn divition_remain(&self, num: usize) {
    //     let remain = self.get_remain();
    //     let blocksize = remain / num as u64;
    //     self.remain
    // }
}