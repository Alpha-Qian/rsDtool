use std::io::SeekFrom;


pub trait Cacher {
    type Write : Writer;
    async fn write_at(&self, pos:SeekFrom) -> Self::Write;
}

pub trait Writer {
    //type Error: std::error::Error + Send + Sync + 'static;
    async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()>;
}

pub fn cache_fn(f: F) -> CacheFn<F>
where
    F: Fn(SeekFrom) -> W,
    W: Writer,
{
    CacheFn(f)
}

pub fn writer_fn(f: F) -> WriterFn<F>
where
    F: FnMut(&[u8]) -> std::io::Result<()>,
{
    WriterFn(f)
}

struct CacheFn<F>(F);

struct WriterFn<F>(F);

///为闭包的默认实现
impl<T> Writer for T 
where
    T: FnMut(&[u8]) -> std::io::Result<()>,
{
    async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        (self)(buf)
    }
}

///为闭包的默认实现
impl<T, W> Cacher for T 
where
    T: Fn(SeekFrom) -> W,
    W: Writer,
{
    type Write = W;
    async fn write_at(&self, pos:SeekFrom) -> Self::Write {
        (self)(pos)
    }
}