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

pub trait ThreadModel: 'static {
    type RefCounter<T>: RefCounted<Target = T>; //这个生命周期视乎就是默认的约束//要求SharePtr<T>必须活得和T一样久

    type AtomicCell<T: Atomic + PartialEq>: Radium<Item = T>
    where
        Cell<T>: Radium<Item = T>;

    type Mutex<T>: Lockable<Item = T>;

    type AtomicSwap<T>: AtomicSwapable<Target = T>;
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

//MaybeAtomic Box ------>
/// 为任意结构体实现原子交换的类型
/// 在多线程中需要分配堆上但在单线程中不需要
///
pub trait AtomicSwapable {
    //MaybeOnHeap
    //实际结构体
    type Target;

    //结构体在栈上实际存储方式
    type Store; //Un on heap

    fn new(data: Self::Store) -> Self;

    ///# Importent
    ///获取的值在解引用为&mut T期间不能调用self的其他方法
    fn get_ptr(&self, order: Ordering) -> *mut Self::Target;

    fn swap(&self, value: Self::Store, order: Ordering) -> Self::Store;

    fn get_mut(&mut self) -> &mut Self::Store;

    fn into_inner(self) -> Self::Store;
}




///UnsafeCell的简易safe包装
/// store in stack



// unsafe impl CorrectDrop for Vec< {
    
// }
//<--------------MaybeAtomicBox

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

    type AtomicSwap<T> = Atom<*mut T>;
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

///Store in Heap
impl<T> AtomicSwapable for Atom<*mut T> {
    type Target = T;

    type Store = *mut T;

    fn new(data: Self::Store) -> Self {
        <Self as Radium>::new(data)
    }

    fn get_ptr(&self, order: Ordering) -> *mut Self::Target {
        <Self as Radium>::load(&self, order)
    }

    fn swap(&self, value: *mut T, order: Ordering) -> *mut T {
        <Self as Radium>::swap(&self, value, order)
    }

    fn get_mut(&mut self) -> &mut Self::Store {
        <Self as Radium>::get_mut(self)
    }
    
    fn into_inner(self) -> Self::Store {
        <Self as Radium>::into_inner(self)
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

    type AtomicSwap<T> = SwapCell<T>;
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

struct SwapCell<T>(UnsafeCell<T>);

impl<T> AtomicSwapable for SwapCell<T> {
    type Target = T;

    type Store = T;

    fn new(data: Self::Store) -> Self {
        Self(data.into())
    }

    fn get_ptr(&self, _ord: Ordering) -> *mut Self::Target {
        self.0.get()
    }

    fn swap(&self, mut target: T, _ord: Ordering) -> T {
        unsafe {
            swap(self.0.get(), &raw mut target);
        }
        target
    }

    fn get_mut(&mut self) -> &mut Self::Store {
        self.0.get_mut()
    }

    fn into_inner(self) -> Self::Store {
        self.0.into_inner()
    }

    // ///T::drop的错误实现会导致重入，已删除
    // unsafe fn store(&self, value: T, order: Ordering) {
    //     //unsafe { *self.0.get() = value }
    //     // 修复点：复用 swap，避免直接赋值 (*ptr = value) 带来的 Drop 异常 / 重入导致的 UB
    //     drop(self.swap(value, order));
    // }
}