use auto_enums::
use crate::{
    base::{
        family::ThreadModel, group_construct::State, request_info::RequestInfo, segment::Segment,
    },
    downloader::{
        group_async_parts::TaskShare,
        group_download_methold::{RawDownloadUnInjected, SegmentDownload, SegmentResume},
        group_manager::{IdleManager, RunningManager},
        group_worker::SegmentWorker,
    },
};


struct RawIniter<I, R> {
    info: RequestInfo,
    segments: I,
    resumers: R,
}

impl<I, R> RawIniter<I, R>
where
    I: Iterator<Item = Segment>,
    R: Iterator<Item: SegmentResume>,
{
    fn init<M: ThreadModel, T: SegmentDownload>(
        self,
        idle_manager: IdleManager<<R::Item as SegmentResume>::Error, M>,
    ) -> RunningManager<<R::Item as SegmentResume>::Error, M> {


        enum FutureTypes<T, T1>{
            A(T),
            B(T1)
        }

        impl<T, U> FutureTypes<T, U>
        where
            T: Future, U: Future<Output = T>,
        {
            fn t(t: T) -> Self{
                Self::A(t)
            }

            fn u(u: U) -> Self {
                Self::B(b)
            }
        }

        impl<T, U> Future for FutureTypes<T, U>
        where
            T: Future, U: Future<Output = T::Output>
        {

            type Output = T::Output;

            fn poll(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
                let this = unsafe { self.get_unchecked_mut() };
                match this{
                    Self::A(a) => a.poll(cx),
                    Self::B(b) => b.poll(cx),
                }
            }
        }


        let manager = idle_manager.into_done_running_manager();
        for i in self.resumers.next() {
            let (segment, downloader) = i.resume();
            let share = manager.clone_share();
            let State::Running((manager, segment_worker)) = manager.map_group(|busy_group| {
                let reporter = busy_group.submit_segment(segment);
                let segment_worker = SegmentWorker::new(reporter, share);
                return segment_worker;
            }) else {
                panic!()
            };
        }

        for i in self.segments.next() {
            T::Exe
        }
    }
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
