use std::{io::SeekFrom, path::Path};

use reqwest::Response;

pub trait Cacher {
    type Write : Writer;
    async fn write_at(&self, pos:SeekFrom) -> Self::Write;
}

pub trait Writer {
    type Error: std::error::Error + Send + Sync + 'static;
    async fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()>;
}