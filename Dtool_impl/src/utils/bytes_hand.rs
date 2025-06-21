use std::ops::ControlFlow;

pub(crate) fn optional_take_prefix(chunk:&[u8], len: Option<usize>) -> (ControlFlow<()>, &[u8]) {
    match len {
        Some(len) => take_prefix(chunk, len),
        None => (ControlFlow::Continue(()), chunk)
    }
}
pub(crate) fn optional_skip_prefix(chunk:&[u8], len: Option<usize>) -> ControlFlow<&[u8]> {
    match len {
        Some(len) => skip_prefix(chunk, len),
        None => ControlFlow::Continue(()),
    }
}

///用于处理字节流的函数
///取出前len个字节
pub(crate) fn take_prefix(chunk:&[u8], len: usize) -> (ControlFlow<()>, &[u8]) {
    if len > chunk.len() {
        (ControlFlow::Continue(()), chunk)
    } else {
        (ControlFlow::Break(()), &chunk[..len])
    }
    
}

///用于处理字节流的函数
///跳过前len个字节
pub(crate) fn skip_prefix(chunk:&[u8], len: usize) -> ControlFlow<&[u8]> {
    if len > chunk.len() {
        ControlFlow::Continue(())
    } else {
        ControlFlow::Break(&chunk[len..])
    }
}
