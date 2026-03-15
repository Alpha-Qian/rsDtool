use std::io::SeekFrom;


pub trait Cacher {
    type Write : Writer;
    async fn write_at(&self, pos:SeekFrom) -> Self::Write;
}

pub trait Writer {
    //type Error: std::error::Error + Send + Sync + 'static;
    async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()>;
}


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