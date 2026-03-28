use std::{cell::Cell, rc::Rc, sync::{Arc, atomic::AtomicU64}, u64};
use std::sync::atomic::Ordering;
use std::sync::atomic::{fence, compiler_fence};
mod private{
    trait Seald {}
}

trait ProcessChannel: Clone {
    fn get(&mut self) -> u64;
    fn store(&mut self, value: u64);
    fn add(&mut self, value: u64) -> u64;
    fn sub(&mut self, value: u64) -> u64;
}

impl ProcessChannel for Arc<AtomicU64> {
    fn get(&mut self) -> u64 {
        self.load(Ordering::Acquire);
    }

    fn store(&mut self, value: u64) {
        
    }

    fn add(&mut self, value: u64) -> u64 {
        
    }

    fn sub(&mut self, value: u64) -> u64 {
        
    }
}

impl ProcessChannel for Rc<Cell<u64>> {
    
}

struct Now{}
