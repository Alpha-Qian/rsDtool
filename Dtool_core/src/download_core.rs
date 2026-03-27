use std::ops::ControlFlow;

pub fn optional_take_prefix(chunk:&[u8], len: Option<usize>) -> (ControlFlow<()>, &[u8]) {
    match len {
        Some(len) => take_prefix(chunk, len),
        None => (ControlFlow::Continue(()), chunk)
    }
}
pub fn optional_skip_prefix(chunk:&[u8], len: Option<usize>) -> ControlFlow<&[u8]> {
    match len {
        Some(len) => skip_prefix(chunk, len),
        None => ControlFlow::Continue(()),
    }
}

///用于处理字节流的函数
///取出前len个字节
pub fn take_prefix(chunk:&[u8], len: usize) -> (ControlFlow<()>, &[u8]) {
    if len > chunk.len() {
        (ControlFlow::Continue(()), chunk)
    } else {
        (ControlFlow::Break(()), &chunk[..len])
    }
    
}

///用于处理字节流的函数
///跳过前len个字节
pub fn skip_prefix(chunk:&[u8], len: usize) -> ControlFlow<&[u8]> {
    if len > chunk.len() {
        ControlFlow::Continue(())
    } else {
        ControlFlow::Break(&chunk[len..])
    }
}


// pub trait AsyncProcessSender{
//     async fn fetch_add(&mut self, len: u32);
//     async fn get_process(&self) -> Option<u64>;
//     async fn get_unwarp(&self) -> u64{
//         self.get_process().await.unwrap()
//     }
// }

// pub trait AsyncEndReciver{
//     async fn get_end(&self) -> Option<u64>{
//         None
//     }
    
//     async fn get_unwarp(&self) -> u64{
//         self.get_end().await.unwrap()
//     }
// }
// ///同步的ProcessSender
// pub trait ProcessSender{
//     fn fetch_add(&mut self, len: u32);
//     // fn get_process(&self) -> Option<u64>;
//     // fn get_unwarp(&self) -> u64{
//     //     self.get_process().unwrap()
//     // }

// ///同步的EndReciver
// pub trait EndReciver{
//     fn get_end(&self) -> Option<u64>{
//         None
//     }
    
//     fn get_unwarp(&self) -> u64{
//         self.get_end().unwrap()
//     }
// }
  
// impl<T> AsyncProcessSender for T where T: ProcessSender{
//     async fn fetch_add(&mut self, len: u32){
//         self.fetch_add(len);
//     }
//     async fn get_process(&self) -> Option<u64>{
//         self.get_process()
//     }
// }

// impl<T> AsyncEndReciver for T where T: EndReciver{
//     async fn get_end(&self) -> Option<u64>{
//         self.get_end()
//     }
// }
// pub async fn get_remain_async(process:&mut impl AsyncProcessSender, end:&mut impl AsyncEndReciver) -> Option<u64>{
//     process.get_process().await.zip(end.get_end().await).map(|(process, end)| {
//         end - process
//     })
// }

// pub fn get_remain(process:&mut impl ProcessSender, end:&mut impl EndReciver) -> Option<u64>{
//     process.get_process().zip(end.get_end()).map(|(process, end)| {
//         end - process
//     })
// }

// #[cfg(test)]
// mod tests{
//     #[test]
//     fn test() {
//         println!("hello world");
//     }
// }

// #[cfg(test)]
// mod test{
//     #[test]
//     fn test(){
//         println!("hello world");
//     }
// }
