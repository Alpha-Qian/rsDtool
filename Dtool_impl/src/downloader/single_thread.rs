//!单线程分块下载

use std::{cell::Cell, ops::{Deref, Rem}, rc::Rc};

//不可复制包装器
struct Remain(Rc<Cell<u64>>);


impl Remain{

    fn new() {
        todo!()
    }
    
    fn only_one_owner(&self) -> bool{
        Rc::strong_count(&self.0) == 1
    }
}
impl Deref for Remain {
    type Target = Cell<u64>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}