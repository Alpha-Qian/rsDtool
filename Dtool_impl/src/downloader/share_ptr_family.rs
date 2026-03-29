use std::{ops::Deref, rc::Rc, sync::{Arc, atomic::AtomicPtr}};

use radium::Radium;
use radium::{Atom, Radon, Isotope};

trait SyncFamily{
    type SmartPtr<T>: Deref<Target = T> + Clone;
    fn new_smart_ptr<T>(t: T) -> Self::SmartPtr<T>;

    type MaybeAtom<T>: Radium<Item = T>;

    // fn new_maybe_atom<T>(t: T) -> Self::MaybeAtom<T>{
    //     Self::MaybeAtom::<T>::new(t)//既然能直接调用，那这个函数有啥用？所以注释了
    // }

    type Mutex<T>: MaybeMutex<Item = T>;
    //一样
    // fn new_maybe_mutex<T>(t: T) -> Self::Mutex{
    //     Self::Mutex::<T>::new(t)
    // }


    type Rwlock<T>: MaybeRwLock<Item = T>;


    // type Rwlock:
}


trait SmartPtrExt{
    type Target;
    fn new(t: Self::Target) -> Self;
    fn strong_count(&self) -> usize;
}


trait MaybeMutex{
    type Item;
    fn new(t: Self::Item) -> Self;
    fn lock(&self, f: impl FnOnce(MutexGuard));
    fn into_inner(self) -> Self::Item;
    fn get_mut(&mut self) -> &mut Self::Item;
}

struct MutexGuard{

}

trait MaybeRwLock{
    type Item;
    fn new(t: Self::Item) -> Self;
    fn read(&self, f: impl FnOnce(ReadGuard));
    fn write(&self, f: impl FnOnce(WriteGuard));
    fn into_inner(self) -> Self::Item;
    fn get_mut(&mut self) -> &mut Self::Item;
}

struct ReadGuard{

}

struct WriteGuard{

}


struct MutithreadFamily;
impl SyncFamily for  {
    
}




///Ptr Family

trait SmartPtrFamily{
    type Ptr<T>: Deref<Target = T> + Clone;

    fn new<T>(t: T) -> Self::Ptr<T>;
}

struct RcFamily;
impl SmartPtrFamily for RcFamily {
    type Ptr<T> = Rc<T>;

    fn new<T>(t: T) -> Self::Ptr<T> {
        Rc::new(t)
    }
}

struct ArcFamily;
impl SmartPtrFamily for ArcFamily {
    type Ptr<T> = Arc<T>;

    fn new<T>(t: T) -> Self::Ptr<T> {
        Arc::new(t)
    }
}

