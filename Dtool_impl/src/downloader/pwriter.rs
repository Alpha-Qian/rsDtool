use std::cell::UnsafeCell;
use std::convert::Infallible;
use bytes::Bytes;
use std::error::Error;
pub trait PWriter {
    type Error: Error;
    async fn pwrite(&self, pos: u64, bytes: Bytes) -> Result<(), Self::Error>;
}

struct Buffer(UnsafeCell<[u8]>);

impl PWriter for Buffer {
    type Error = Infallible;
    async fn pwrite(&self, pos: u64, bytes: Bytes) -> Result<(), Self::Error> {
        unsafe {
            let ptr = self.0.get();
            assert!(pos as usize <= ptr.len());

            let raw_ptr = (ptr as *mut u8).offset(pos as isize);
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), raw_ptr, bytes.len());
        };
        Ok(())
    }
}

use std::fs::File;
use std::os::windows::fs::FileExt;

impl PWriter for File {
    type Error = std::io::Error;

    async fn pwrite(&self, pos: u64, bytes: Bytes) -> Result<(), Self::Error> {
        // 克隆文件句柄（Windows 下 File 的 clone 是浅拷贝，指向同一个系统对象）
        let file = self.try_clone()?;

        tokio::task::spawn_blocking(move || {
            let mut buf = bytes.as_ref();
            let mut current_pos = pos;

            while !buf.is_empty() {
                let n = FileExt::seek_write(&file, buf, current_pos)?;
                if n == 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::WriteZero,
                        "write zero",
                    ));
                }
                buf = &buf[n..];
                current_pos += n as u64;
            }
            Ok(())
        })
        .await
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
    }
}

trait IoBuf: Unpin + 'static{
    fn as_slice(&self) -> &[u8];

    fn as_mut_slice(&mut self) -> &mut [u8]
    where Self: Mutable;
}

trait Mutable{}


pub trait PWriterNew{
    fn pwrite<S: Deref<Target = [u8]> + 'static>(position: u64, slice: S);
}

pub fn write_ref<W: PWriterNew>(pos: u64, slice: &[u8])