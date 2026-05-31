use std::task::{Context, Poll, Wake, Waker};

use futures::task::noop_waker_ref;

fn poll_once<F: Future>(future: F) -> Option<F::Output> {
    let mut context = Context::from_waker(noop_waker_ref());
    let f = std::pin::pin!(future);
    match f.poll(&mut context) {
        Poll::Ready(result) => Some(result),
        Poll::Pending => None,
    }
}

fn unwarp_sync<F: Future>(future: F) -> F::Output {
    poll_once(future).expect("尝试在同步函数中调用异步函数")
}

trait SyncFuture: Future {
    fn run_sync(self) -> Self::Output
    where
        Self: Sized,
    {
        poll_once(self).expect("尝试在同步函数中调用需要异步操作函数")
    }
}
