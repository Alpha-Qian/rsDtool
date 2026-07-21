use crate::{
    base::{family::ThreadModel, request_info::RequestInfo, segment::Segment},
    downloader::{
        group_async_parts::GroupShare,
        group_download_methold::{RawDownloadUnInjected, SegmentDownload},
        group_manager::{IdleManager, RunningManager},
    },
};

mod seald {}

pub(crate) trait ManagerInitExt {
    fn init<M: ThreadModel, E>(
        self,
        idle: IdleManager<E, M>,
    ) -> (RunningManager<E, M>, impl Iterator<Item = impl Future>);
}

trait ManagerInit {
    type SegmentIter: Iterator<Item = Segment>;

    fn into_initer(self) -> RawIniter<Self::SegmentIter>;
}

///原始的初始化器
struct RawIniter<I, T> {
    info: RequestInfo,
    segments: I,
    tasks: T,
}

impl<I: Iterator<Item = Segment>, T> RawIniter<I, T> {
    pub fn new(info: RequestInfo, segments: impl IntoIterator<IntoIter = I>) -> Self {
        RawIniter {
            info,
            segments: segments.into_iter(),
        }
    }

    ///为了避免ManagerInit的重复实现
    fn init_privied<E, M: ThreadModel>(
        self,
        idle_manager: IdleManager<E, M>,
    ) -> (FutureIter<E, M>, RunningManager<E, M>) {
        //let abort_signal = M::RefCounter::new(M::AtomicCell::new(bool));
        let data_share = M::RefCounter::new(GroupShare::new());
        let Some(frist) = self.segments.next() else {
            return RunningManager {
                group: idle_manager.0,
                share: data_share,
            };
        };

        todo!()
    }
}

impl<T: ManagerInit> ManagerInitExt for T {
    fn init<M: ThreadModel, E>(self, idle: IdleManager<E, M>) -> RunningManager<E, M> {
        let raw = self.into_initer();
        raw.init_privied(idle)
    }
}

trait DownloadIter: Iterator
where
    Self::Item: RawDownloadUnInjected,
{
}

struct ResumeInfo;

struct SniffingResponse;

struct Empty;

struct Done;

// 注释原因：全是init_with<SomeThing>
//
// fn run_from_raw<I: Iterator<Item = Segment>>(
//     self,
//     info: RequestInfo,
//     mut segments: I,
// ) -> RunningManager<M> {
//     //let running = RunningManager(self.0);
//     let guard = self.0.lock();
//     let Some(first) = segments.next() else {
//         return RunningManager(self.0);
//     };
//     todo!()
// }

// fn from_sniffing_response(self) -> RunningManager<M> {
//     todo!()
// }

// fn run_first_response(self, info: RequestInfo, client: Client) -> RunningManager<M> {
//     todo!()
// }

// fn into_done_running_manager(self) -> RunningManager<M> {
//     todo!()
// }

// trait ReInit{
//     fn init() -> Self;
//     fn re_init(&self);
// }
