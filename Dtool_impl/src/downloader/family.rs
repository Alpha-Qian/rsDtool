use std::{cell::{Cell, RefCell, RefMut}, ops::{Deref, DerefMut}, rc::Rc, sync::Arc};

use radium::{Radium, marker::{Atomic, Nuclear}};
use radium::Atom;



pub trait SyncKind{
    type SharePtr<T>: ShareExt;

    type MaybeAtom<T: Atomic + PartialEq>: Radium<Item = T>
        where Cell<T>: Radium<Item = T>
        ;

    type DataLock<T>: MaybeMutex<Item = T>;
}

pub trait ShareExt: Deref + Clone{//Maybe Send
    fn new(v: Self::Target) -> Self;
}

pub trait MaybeMutex{
    type Item;
    type Guard<'a>: DerefMut<Target = Self::Item>// + Drop 针对RefMut
        where Self: 'a;
    
    fn lock(&self) -> Self::Guard<'_>;
}

//快捷方式：
pub type SharePtr<F, T> = <F as SyncKind>::SharePtr<T>;//StaticPtr
pub type MaybeAtomic<F, T> = <F as SyncKind>::MaybeAtom<T>;//share Num
pub type DataLock<F, T> = <F as SyncKind>::DataLock<T>;//share struct

//  具体实现：

//需要在多线程中运行：
pub struct MutiThread;
impl SyncKind for MutiThread {
    type SharePtr<T> = Arc<T>;
    type MaybeAtom<T: Atomic + PartialEq> = Atom<T>
        where Cell<T>: Radium<Item = T>
        ;
    type DataLock<T> = parking_lot::Mutex<T>;
}

impl<T> ShareExt for Arc<T> {
    fn new(v: Self::Target) -> Self {
        Arc::new(v)
    }
}

impl<T> MaybeMutex for parking_lot::Mutex<T> {
    type Item = T;
    type Guard<'a> = parking_lot::MutexGuard<'a, Self::Item>
        where Self: 'a;

    fn lock(&self) -> Self::Guard<'_> {
        self.lock()
    }
}


//只需要在单线程环境中运行：
pub struct SingleThread;
impl SyncKind for SingleThread {
    type SharePtr<T> = Rc<T>;
    type MaybeAtom<T: Atomic + PartialEq> = Cell<T>
        where Cell<T>: Radium<Item = T>
        ;
    type DataLock<T> = RefCell<T>;
}

impl<T> ShareExt for Rc<T> {
    fn new(v: Self::Target) -> Self {
        Rc::new(v)
    }
}

impl<T> MaybeMutex for RefCell<T> {
    type Item = T;
    type Guard<'a> = RefMut<'a, T>
            where Self: 'a;
    fn lock(&self) -> Self::Guard<'_> {
        self.borrow_mut()
    }
}