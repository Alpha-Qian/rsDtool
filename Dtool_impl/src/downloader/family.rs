use std::{mem,
    arch::x86_64::_MM_PERM_BBAC, cell::{Cell, RefCell, RefMut, UnsafeCell}, ops::{Deref, DerefMut}, ptr::swap, rc::Rc, sync::{Arc, atomic::Ordering}
};

use parking_lot::{RawMutex, lock_api::RawMutex as RawMutexApi};
use radium::Atom;
use radium::{
    Radium, Radon,
    marker::{Atomic, Nuclear},
};

//命名参考：https://aistudio.google.com/prompts/1kdVbB3yyDukhLxzDQZ6XTWpbUyWQ1eMg

pub trait ThreadModel: 'static + Copy{
    type RefCounter<T>: RefCounted<Target = T>; //这个生命周期视乎就是默认的约束//要求SharePtr<T>必须活得和T一样久

    type AtomicCell<T: Atomic + PartialEq>: Radium<Item = T>
    where
        Cell<T>: Radium<Item = T>;

    type Mutex: Lockable;
}

pub trait RefCounted: Deref + Clone {
    fn new(v: Self::Target) -> Self;
    //fn strong_count(&self) -> usize;
    //fn as_mut(&mut self) -> Option(&mut Slef::Target);
}

pub unsafe trait Lockable {
    fn new() -> Self;
    fn acquire(&self);
    fn release(&self);
}


//快捷方式：
pub type RefCounter<F: ThreadModel, T> = F::RefCounter<T>;
pub type AtomicCell<F: ThreadModel, T> = F::AtomicCell<T>;
pub type Mutex<F: ThreadModel> = F::Mutex;

//  具体实现：

//需要在多线程中运行：
#[derive(Clone, Copy)]
pub struct ThreadSafe;
impl ThreadModel for ThreadSafe {
    type RefCounter<T> = Arc<T>;
    type AtomicCell<T: Atomic + PartialEq>
        = Atom<T>
    where
        Cell<T>: Radium<Item = T>;
    type Mutex = RawMutex;
}

impl<T> RefCounted for Arc<T> {
    fn new(v: Self::Target) -> Self {
        Arc::new(v)
    }
}

// impl Lockable for parking_lot::Mutex<()> {
//     fn acquire(&self) {
//         std::mem::forget(self.lock());
//     }

//     fn release(&self) {
//         unsafe{ self.make_guard_unchecked(); }
//     }
// }



//不需要在多线程中运行：
#[derive(Clone, Copy)]
pub struct ThreadLocal;
impl ThreadModel for ThreadLocal {
    type RefCounter<T> = Rc<T>;
    type AtomicCell<T: Atomic + PartialEq>
        = Cell<T>
    where
        Cell<T>: Radium<Item = T>;
    type Mutex = BorrowChecker;
}

impl<T> RefCounted for Rc<T> {
    fn new(v: Self::Target) -> Self {
        Rc::new(v)
    }
}


struct BorrowChecker{
    borrowed: UnsafeCell<bool>
}

unsafe impl Lockable for BorrowChecker {

    fn new() -> Self {
        Self { borrowed: false.into() }
    }
    fn acquire(&self) {
        let p = &mut unsafe {
           *self.borrowed.get() 
        };
        if !*p {
            *p = true
        } else {
            panic!("borrow check error")
        }
    }

    fn release(&self) {
        let p = &mut unsafe {
           *self.borrowed.get() 
        };
        *p = false
    }
}

// unsafe impl<T: RawMutexApi> Lockable for T {
//     fn new() -> Self {
        
//     }
//     fn acquire(&self) {
//         self.lock();
//     }

//     fn release(&self) {
//         unsafe{self.unlock();}
//     }
// }