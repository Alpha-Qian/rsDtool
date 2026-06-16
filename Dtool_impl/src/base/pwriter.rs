use bytes::{Bytes, BytesMut};
use std::error::Error;
use std::mem::ManuallyDrop;
use std::ops::{Deref, DerefMut, Index, RangeBounds};
use std::rc::Rc;
use std::sync::Arc;
// pub trait PWriter {
//     type Error: Error;
//     async fn pwrite(&self, pos: u64, bytes: Bytes) -> Result<(), Self::Error>;
// }

pub trait BufWriter {
    type Error: Error;

    async fn pwrite_raw<S>(&self, pos: u64, buffer: S) -> (Result<(), Self::Error>, S)
    where
        S: Deref<Target = [u8]> + 'static;

    async fn pwrite<S>(&self, pos: u64, buffer: S) -> Result<(), Self::Error>
    where
        S: Deref<Target = [u8]> + 'static,
    {
        self.pwrite_raw(pos, buffer).await.0
    }
}

// struct Buffer(UnsafeCell<[u8]>);

// impl BufWriter for Buffer {
//     type Error = Infallible;
//     async fn pwrite(&self, pos: u64, bytes: Bytes) -> Result<(), Self::Error> {
//         unsafe {
//             let ptr = self.0.get();
//             assert!(pos as usize <= ptr.len());

//             let raw_ptr = (ptr as *mut u8).offset(pos as isize);
//             std::ptr::copy_nonoverlapping(bytes.as_ptr(), raw_ptr, bytes.len());
//         };
//         Ok(())
//     }
// }

// use std::fs::File;
// use std::os::windows::fs::FileExt;

// impl BufWriter for File {
//     type Error = std::io::Error;

//     async fn pwrite(&self, pos: u64, bytes: Bytes) -> Result<(), Self::Error> {
//         // 克隆文件句柄（Windows 下 File 的 clone 是浅拷贝，指向同一个系统对象）
//         let file = self.try_clone()?;

//         tokio::task::spawn_blocking(move || {
//             let mut buf = bytes.as_ref();
//             let mut current_pos = pos;

//             while !buf.is_empty() {
//                 let n = FileExt::seek_write(&file, buf, current_pos)?;
//                 if n == 0 {
//                     return Err(std::io::Error::new(
//                         std::io::ErrorKind::WriteZero,
//                         "write zero",
//                     ));
//                 }
//                 buf = &buf[n..];
//                 current_pos += n as u64;
//             }
//             Ok(())
//         })
//         .await
//         .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
//     }
// }
//
//
//

use std::io::Write;

trait DetachWriter {
    type Error: Error;
    fn write(&mut self, buffer: DetachRef<'_, [u8]>) -> Result<usize, Self::Error>;

    fn write_all(&mut self, buffer: DetachRef<'_, [u8]>) -> Result<(), Self::Error>;
}

trait DetachRead {
    type Error: Error;

    fn read(&self, buffer: DetachRef<'_, [u8]>) -> Result<usize, Self::Error>;

    fn read_all(&mut self, buffer: DetachRef<'_, [u8]>) -> Result<(), Self::Error>;
}

trait DetachWrite {}

impl<T: ?Sized + Write> DetachWrite for T {}

///保证可分离到其他线程的引用,用于io-uring, 主要用于中间层api
#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct DetachRef<'a, T: ?Sized>(&'a T);

impl<'a, T: ?Sized> DetachRef<'a, T> {
    /// 创建一个可分离的不可变引用，用于跨越异步边界（如 io-uring 提交）。
    ///
    /// # Safety
    ///
    /// 调用者必须保证以下所有条件：
    ///
    /// 1. **生命周期真实性**：`inner` 所指向的数据在整个 `'a` 期间保持有效且不被释放，
    ///    即使该引用被传递到当前作用域之外（如内核、其他线程）。
    ///
    /// 2. **不可变性保证**：在 `'a` 的整个持续期间，不存在对同一数据的可变引用（`&mut T`），
    ///    包括通过 `UnsafeCell` 等内部可变性进行的修改——除非 `T` 本身的语义允许并发读取。
    ///
    /// 3. **固定性（Pin-stability）**：底层数据在 `'a` 期间不会被移动或重新分配。
    ///    例如，`Vec` 的 buffer 可能因扩容而失效，调用者必须确保不会发生此类情况。
    ///
    /// 4. **Send 安全性**：如果此引用将被发送到其他线程，`T` 必须满足 `Sync`
    ///    （即 `&T: Send`）的语义要求，即使编译器未在类型层面强制检查。
    ///
    /// # 典型的正确用法
    ///
    /// ```ignore
    /// // buffer 被 pin 住且生命周期覆盖整个 io-uring 操作
    /// let buf: &'a [u8] = pinned_buffer.as_slice();
    /// let detached = unsafe { DetechRef::new(buf) };
    /// // 提交给 io-uring，在 completion 返回前 buf 不会被释放或修改
    /// ```
    ///
    /// # 为什么是 unsafe
    ///
    /// Rust 的借用检查器无法跨越 io-uring 等异步内核接口追踪引用的实际使用范围。
    /// `DetechRef` 通过 `Copy` 语义绕过了编译器的借用追踪，因此构造时必须由
    /// 调用者手动承担生命周期和别名规则的正确性证明。
    pub unsafe fn new(inner: &'a T) -> Self {
        Self(inner)
    }

    pub fn into_raw(self) -> &'a T {
        self.0
    }

    pub fn map<U, F>(self, f: F) -> DetachRef<'a, U>
    where
        U: ?Sized,
        F: FnOnce(&T) -> &U,
    {
        unsafe { DetachRef::new(f(self.0)) }
    }
}

impl<'a, T> DetachRef<'a, [T]> {
    pub fn slice(self, range: impl RangeBounds<usize>) -> Self {
        Self(self.0[range])
    }
}

impl<'a, T: ?Sized> Deref for DetachRef<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

///保证可分离到其他线程的可变引用
#[repr(transparent)]
pub struct DetechMut<'a, T: ?Sized>(&'a mut T);

impl<'a, T: ?Sized> DetechMut<'a, T> {
    pub unsafe fn new(inner: &'a mut T) -> Self {
        Self(inner)
    }

    fn into_raw(self) -> &'a mut T {
        self.0
    }

    fn map<U, F>(self, f: F) -> DetachRef<'a, U>
    where
        U: ?Sized,
        F: FnOnce(&mut T) -> &U,
    {
        unsafe { DetachRef::new(f(self.0)) }
    }

    fn map_mut<U, F>(self, f: F) -> DetechMut<'a, U>
    where
        U: ?Sized,
        F: FnOnce(&mut T) -> &mut U,
    {
        unsafe { DetechMut::new(f(self.0)) }
    }
}

impl<'a, T: ?Sized> Deref for DetechMut<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a, T: ?Sized> DerefMut for DetechMut<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

///可被解引用为可分离引用
trait DerefDetech {
    type Target: ?Sized;

    fn detech(&self) -> DetachRef<'_, Self::Target>;
}

trait DerefDetechMut: DerefDetech {
    fn detech_mut(&mut self) -> DetechMut<'_, Self::Target>;
}

///Box<T>
/// 安全性：ManuallyDrop<Box<T>>在其生命周期内都不会因为panic而drop
impl<T: ?Sized + 'static> DerefDetech for ManuallyDrop<Box<T>> {
    type Target = T;

    fn detech(&self) -> DetachRef<'_, Self::Target> {
        unsafe { DetachRef::new(self.deref()) }
    }
}
impl<T: ?Sized + 'static> DerefDetechMut for ManuallyDrop<Box<T>> {
    fn detech_mut(&mut self) -> DetechMut<'_, Self::Target> {
        unsafe { DetechMut::new(self.deref_mut()) }
    }
}

///Rc<T>
impl<T: ?Sized + 'static> DerefDetech for ManuallyDrop<Rc<T>> {
    type Target = T;

    fn detech(&self) -> DetachRef<'_, Self::Target> {
        unsafe { DetachRef::new(self.deref()) }
    }
}

///Arc<T>
impl<T: ?Sized + 'static> DerefDetech for ManuallyDrop<Arc<T>> {
    type Target = T;

    fn detech(&self) -> DetachRef<'_, Self::Target> {
        unsafe { DetachRef::new(self.deref()) }
    }
}

///Bytes
impl DerefDetech for ManuallyDrop<Bytes> {
    type Target = [u8];

    fn detech(&self) -> DetachRef<'_, Self::Target> {
        unsafe { DetachRef::new(self.deref()) }
    }
}

///BytesMut
impl DerefDetech for ManuallyDrop<BytesMut> {
    type Target = [u8];

    fn detech(&self) -> DetachRef<'_, Self::Target> {
        unsafe { DetachRef::new(self.deref()) }
    }
}

impl DerefDetechMut for ManuallyDrop<BytesMut> {
    fn detech_mut(&mut self) -> DetechMut<'_, Self::Target> {
        unsafe { DetechMut::new(self.deref_mut()) }
    }
}

///& T
impl<T: ?Sized> DerefDetech for &'static T {
    type Target = T;
    fn detech(&self) -> DetachRef<'_, Self::Target> {
        unsafe { DetachRef::new(self) }
    }
}

///&mut T
impl<T: ?Sized> DerefDetech for &'static mut T {
    type Target = T;
    fn detech(&self) -> DetachRef<'_, Self::Target> {
        unsafe { DetachRef::new(self) }
    }
}

impl<T: ?Sized> DerefDetechMut for &'static mut T {
    fn detech_mut(&mut self) -> DetechMut<'_, Self::Target> {
        unsafe { DetechMut::new(self) }
    }
}

trait PWriter {
    fn pwrite(&mut self, offset: u64, data: &[u8]);

    fn clone_pwriter(&self) -> Self;
}
