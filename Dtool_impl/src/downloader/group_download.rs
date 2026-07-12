use reqwest::blocking::{Client, Response};
use std::future::poll_fn;
use std::task::Poll;

use crate::base::{
    group_construct::{self, BusyGroup, DownloadGroup, GroupGuard, GroupParts, State}, family::ThreadModel, group_impl2::AsyncParts, pwriter::BufWriter, request_info::RequestInfo, segment::Segment
};

struct Builder {
    info: RequestInfo,
    response: Response,
    client: Client,
    length: u64,
}

impl Builder {
    async fn download_scoped<
        M: ThreadModel,
        F: AsyncFnOnce(&mut DownloadGroup<'static, M, AsyncParts>, Fut),
        Fut: Future
    >(
        self,
        f: F,
        writer: &impl BufWriter,
    ) -> Result<(), ()> {
        let waker = poll_fn(|cx| Poll::Ready(cx.waker().clone())).await;
        let download_group = todo!();
        let first_future = todo!();
        f(&mut download_group, first_future).await;

        download_group.join_all()
    }
}


///使用迭代器恢复
struct Resumer<I> {
    info: RequestInfo,
    segments: I,
}

impl<I> Resumer<I>
where
    I: IntoIterator<Item = Segment>
{
    async fn download_scoped<
        M: ThreadModel,
        Iter: IntoIterator<Item = impl Future>,
        F: AsyncFnOnce(//柯里化
            Iter
        ) -> F2,
        F2: AsyncFnOnce(DownloadGroup<'static, M, AsyncParts>)
    >(
        self,
        f: F,
        writer: &impl BufWriter,
    ) -> Result<(), ()> {
        let waker = poll_fn(|cx| Poll::Ready(cx.waker().clone()));
        let download_group = DownloadGroup::new_busy(group, data, busy_data)
        let futures = todo!();

        f(futures).await(todo!());

        loop {
            todo!("deng dai")
        }
    }
}

#[cfg(test)]
mod test{

    #[tokio::test]
    async fn test_download() {

    }
}
