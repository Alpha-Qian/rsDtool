use std::{
    cell::{Cell, UnsafeCell},
    ops::Deref,
    rc::Rc,
    sync::Arc,
};

use parking_lot::RawMutex;
use radium::{Atom, Radium, marker::Atomic};

pub trait ThreadModel: 'static {
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

    fn new_acquireed() -> Self {
        let m = Self::new();
        m.acquire();
        m
    }
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

struct BorrowChecker {
    borrowed: UnsafeCell<bool>,
}

unsafe impl Lockable for BorrowChecker {
    fn new() -> Self {
        Self {
            borrowed: false.into(),
        }
    }
    fn acquire(&self) {
        let p = &mut unsafe { *self.borrowed.get() };
        if !*p {
            *p = true
        } else {
            panic!("borrow check error")
        }
    }

    fn release(&self) {
        let p = &mut unsafe { *self.borrowed.get() };
        *p = false
    }
}

struct MyRefCell(Cell<bool>);

impl MyRefCell {
    fn new() -> Self {
        Self(Cell::new(false))
    }

    fn acqure(&self) {
        let b = Cell::new(true);
        self.0.swap(&b);
        if b.get() {
            panic!()
        }
    }

    fn release(&self) {
        self.0.set(false);
    }
}
