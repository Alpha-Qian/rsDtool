use std::{cell::{Cell, RefCell, UnsafeCell}, ops::{Deref, DerefMut}, rc::Rc, sync::{Arc, atomic::AtomicPtr}, task::Context};

use futures::future::IntoStream;
use radium::{Radium, marker::{Atomic, Nuclear}};
use radium::{Atom, Radon, Isotope};
use scopeguard::defer;

use std::future::Future;


trait SyncKind{
    type SharePtr<T>: ShareExt;

    type MaybeAtom<T: Atomic + PartialEq + Nuclear>: Radium<Item = T>
        where Cell<T>: Radium<Item = T>;

    type DataLock<T>: MaybeMutex<Item = T>;
}

trait ShareExt: Deref + Clone{//Maybe Send
    fn new(v: Self::Target) -> Self;
}

// trait Atomable : Atomic + PartialEq + Nuclear{

// }

trait MaybeMutex{
    type Item;
    type Guard<'a>: DerefMut<Target = Self::Item>// + Drop 针对std::cell::RefMut
        where Self: 'a;
    
    fn lock(&self) -> Self::Guard<'_>;
}


//  具体实现：

//需要在多线程中运行：
struct MutiThread;
impl SyncKind for MutiThread {
    type SharePtr<T> = Arc<T>;
    type MaybeAtom<T: Atomic + PartialEq + Nuclear> = Atom<T>
         where Cell<T>: Radium<Item = T>,;
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
struct SingleThread;
impl SyncKind for SingleThread {
    type SharePtr<T> = Rc<T>;
    type MaybeAtom<T: Atomic + PartialEq + Nuclear> = Cell<T>
        where Cell<T>: Radium<Item = T>;
    type DataLock<T> = std::cell::RefCell<T>;
}

impl<T> ShareExt for Rc<T> {
    fn new(v: Self::Target) -> Self {
        Rc::new(v)
    }
}

impl<T> MaybeMutex for std::cell::RefCell<T> {
    type Item = T;
    type Guard<'a> = std::cell::RefMut<'a, T>
            where Self: 'a;
    fn lock(&self) -> Self::Guard<'_> {
        self.borrow_mut()
    }
}