//! 不可续传大小未知的单线程下载器

use radium::Radium;
use reqwest::{Client, Response}
use tokio::{net::ToSocketAddrs, task::JoinHandle};

use crate::base::{
    builder::UnKonwLength, family::{RefCounted, ThreadModel}, pwriter::BufWriter, request_info::RequestInfo, subcontext::Writer
};

impl UnKonwLength {
    fn download<'a, F: ThreadModel>(
        self,
        writer: &'a impl BufWriter,
        max_length: u64,
    ) -> (F::RefCounter<F::AtomicCell<u64>>, impl Future) {
        let process = F::RefCounter::<_>::new(F::AtomicCell::new(value));
        let process2 = process.clone();

        let future = async { todo!() };

        (process2, future)
    }
}

fn download<M: ThreadModel>(
    info: RequestInfo,
    client: Client,
    response: Response,
    process: M::RefCounter<M::AtomicCell<u64>>,
    writer: &'a impl BufWriter,
    max_length: u64,
) -> impl Future {
    let response = Some(response);
    loop {
        let currentresponse = response.take();
        let current_response = match currentresponse {
            Some(r) => r,
            None => {
                todo!()
            }
        }

        while let Some(bytes) = current_response.chunk().await? {
            todo!()
        }
    }
}

struct Shared<M: ThreadModel> {
    writed: M::AtomicCell<u64>,
}
