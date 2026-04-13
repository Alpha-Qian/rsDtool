use std::{
    cell::{Cell, RefCell, RefMut, UnsafeCell},
    ops::{Deref, DerefMut},
    ptr::swap,
    rc::Rc,
    sync::{Arc, atomic::Ordering},
};

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

    type Mutex<T>: Lockable<Item = T>;
}

pub trait RefCounted: Deref + Clone {
    fn new(v: Self::Target) -> Self;

    
}

pub trait Lockable {
    type Item;
    type Guard<'a>: DerefMut<Target = Self::Item>//没有 Drop 因为没法检查guard字段是否实现Drop
    where
        Self: 'a;

    fn lock(&self) -> Self::Guard<'_>;
}


//快捷方式：
pub type RefCounter<F: ThreadModel, T> = F::RefCounter<T>;
pub type AtomicCell<F: ThreadModel, T> = F::AtomicCell<T>;
pub type Mutex<F: ThreadModel, T> = F::Mutex<T>;

//  具体实现：

//需要在多线程中运行：
pub struct ThreadSafe;
impl ThreadModel for ThreadSafe {
    type RefCounter<T> = Arc<T>;
    type AtomicCell<T: Atomic + PartialEq>
        = Atom<T>
    where
        Cell<T>: Radium<Item = T>;
    type Mutex<T> = parking_lot::Mutex<T>;
}

impl<T> RefCounted for Arc<T> {
    fn new(v: Self::Target) -> Self {
        Arc::new(v)
    }
}

impl<T> Lockable for parking_lot::Mutex<T> {
    type Item = T;
    type Guard<'a>
        = parking_lot::MutexGuard<'a, Self::Item>
    where
        Self: 'a;

    fn lock(&self) -> Self::Guard<'_> {
        self.lock()
    }
}



//只需要在单线程环境中运行：
pub struct ThreadLocal;
impl ThreadModel for ThreadLocal {
    type RefCounter<T> = Rc<T>;
    type AtomicCell<T: Atomic + PartialEq>
        = Cell<T>
    where
        Cell<T>: Radium<Item = T>;
    type Mutex<T> = RefCell<T>;
}

impl<T> RefCounted for Rc<T> {
    fn new(v: Self::Target) -> Self {
        Rc::new(v)
    }
}

impl<T> Lockable for RefCell<T> {
    type Item = T;
    type Guard<'a>
        = RefMut<'a, T>
    where
        Self: 'a;
    fn lock(&self) -> Self::Guard<'_> {
        self.borrow_mut()
    }
}

// struct UnsafeMutex<T>(UnsafeCell<T>);

// impl Lockable for UnsafeMutex<T> {
//     fn lock(&self) -> Self::Guard<'_> {
        
//     }
// }

// struct UnsafeGuard{

// }