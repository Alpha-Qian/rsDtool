//!多线程下载器
//! 在Range为0-时不可用
use crate::downloader::core::{Sender, Syncer};
use crate::downloader::segment::{RunningSegmentsVec, SegmentIter};

//use super::core::download_with_check_remain;
use super::request::DownloadRequest;
use std::ops::Deref;
use std::sync::{Arc, Weak};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{sync::atomic::AtomicU64};
use Dtool_core::ProcessSender;
use parking_lot::{Mutex, RwLock};
use tokio;
use tokio::fs::File;
use tokio::task::{AbortHandle, JoinSet, JoinHandle};

use super::segment::{Segment, SplitSegment};


// ///不可复制的包装器
// struct Remain(Arc<AtomicU64>);

// impl Remain {
//     fn only_one_owner(&self) -> bool{
//         Arc::strong_count(&self.0) == 1
//     }

//     fn make_two_copy(remain: u64) -> (Self, Self) {
//         let remain = Arc::new(AtomicU64::new(remain));
//         (Self(remain.clone()), Self(remain))
//     }
// }

// impl Deref for Remain {
//     type Target = AtomicU64;
//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }

// impl ProgressRecorder for Remain {
//     fn update_progress(&mut self, chunk_len: usize) {
        
//     }
// }

// impl RemainGetter for Remain {
//     fn get_remain(&mut self) -> u64 {
//         self.0.load(Ordering::Acquire)
//     }
// }


// type test<T> = RunningSegments<Weak<T>>;


///多线程下载执行器



use futures::task::AtomicWaker;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::task::{JoinError, JoinHandle};

pub struct Downloader {
    share: Arc<Share>,
    length: u64,
}

impl Downloader {
    pub fn process(&self) -> u64 {
        self.share.process.load(Ordering::Relaxed)
    }

    pub fn stop_all(&mut self){
        self.abort_all();
    }
}

struct Share{
    locked: Mutex<Locked>,
    process: AtomicU64,
    requests: DownloadRequest,
    file: File,
    target_task_num: AtomicUsize,
    task_num: AtomicUsize,
}

struct Locked {
    slots: Vec<Slot>,
    idie_segments: Vec<Segment>,
    // 精细化管理：当任务完成并从 Vec 移除时，唤醒正在等待的 join_next
    waker: AtomicWaker,
}


/// 内部存储项
struct Slot {
    handle: JoinHandle<()>,
    // 每一个 Slot 都持有一个指向自己当前索引的共享引用
    share: Arc<SlotShare>,
    end: u64,
}

impl Slot {

    fn new(handle: JoinHandle<()>, share: Arc<SlotShare>, end: u64) -> Self{
        Self{
            handle,
            share,
            end
        }
    }

    fn index_ref(&self) -> &AtomicUsize{
        &self.share.index_ref
    }

    fn remain(&self) -> &AtomicU64{
        &self.share.remain
    }
}

struct SlotShare{
    index_ref: AtomicUsize,
    remain: AtomicU64,
}

impl SlotShare {
    fn new_pair(index_ref: usize, remain: u64) -> (Arc<SlotShare>, Arc<SlotShare>) {
        let share: Arc<_> =SlotShare{
            index_ref: index_ref.into(),
            remain: remain.into()
        }.into();
        (share.clone(), share)
    }
}

impl Downloader {


    pub fn new_task<F>(&self, future: F, response: Option<Response>)
    where
        F: Future<Output = Result<(),()>> + Send + 'static,
    {
        self.share.task_num.fetch_add(1, Ordering::Release);
        

        let mut guard = self.share.locked.lock();
        let (share_for_slot, share_for_task) = SlotShare::new_pair(guard.slots.len(), todo!());
        let dl_info_for_task = self.share.clone();


        let task = async move {
            let result = future.await;

            
            let mut guard = dl_info_for_task.locked.lock();
            let idx = index_ref_for_task.load(Ordering::Acquire);
            
            // 关键：执行 swap_remove
            let last_idx = guard.slots.len() - 1;
            if idx != last_idx {
                // 如果移除的不是最后一个，最后一个元素会被移动到 idx 位置
                // 我们必须更新被移动元素的索引引用
                guard.slots[last_idx].index_ref.store(idx, Ordering::Release);
            }

            let my_slot = guard.slots.swap_remove(idx);

            // 精细化唤醒：任务删除了，通知 join_next 该返回了
            guard.waker.wake();

            dl_info_for_task.task_num.fetch_sub(1, Ordering::Release);
            
            
            match result {
                Ok(_) => {
                    
                },

                Err(_) => {
                    guard.idie_segments.push(Segment { remain: my_slot.remain().load(Ordering::Relaxed), end: my_slot.end });
                }
            }
        };

        guard.slots.push(Slot::new(tokio::spawn(task), share_for_slot, todo!()));
    }

    /// 核心：实现类似 Stream 的 poll_next 逻辑
    pub fn poll_wait_all(&self, cx: &mut Context<'_>) -> Poll<()> {
        let inner = self.share.locked.lock();
        
        if inner.slots.is_empty() {
            Poll::Ready(())
        } else{
            // 注册当前 Context 的 Waker
            // 当任何一个任务执行完“自清理”逻辑后，会调用 inner.waker.wake()
            inner.waker.register(cx.waker());

            // 注意：由于是自清理，我们不需要在这里 poll 每一个 JoinHandle
            // 只要任务完成，它就会把自己从 Vec 删掉，并触发上面的 wake()
            // 这样下一次 poll 进来时，我们会发现 len 变小了。
            // 但为了简单，我们通常让 join_next 只是作为一个“信号”返回
            Poll::Pending
        }
    }

    pub async fn wait_all(&self) -> Option<()> {
        futures::future::poll_fn(|cx| self.poll_wait_all(cx)).await
    }

    pub fn poll_wait_next()

    fn abort_all(&mut self) {
        let inner= self.share.locked.lock();
        for i in inner.slots{ i.handle.abort(); }
    }

    fn shutdown(&mut self) {
        self.abort_all();
        self.wait_all();
    }
}

async fn download_segment(client: Client, share: Arc<Share>, slot_share: Arc<SlotShare>, start: u64) -> Result<(),()>{
    let request = share.requests.build_request();
    let response = client.execute(request).await?;
    struct Sync{
        share: Arc<Share>,
        slot_share: Arc<SlotShare>,
    }

    impl Syncer for Sync {
        fn get_remain(&self) -> u64 {
            self.slot_share.remain.load(Ordering::Acquire)
        }

        fn update_remain(&mut self, chunk_len: usize) -> u64 {
            self.slot_share.remain.fetch_update(set_order, fetch_order, f)
        }
    }

    download_with_check_remain(response, writer, S).await?;
}

enum RunningState {
    Getting,
    Downloading,
}

use reqwest::{Client, Error, Response};
