struct AsyncBusyGroup


use std::sync::atomic::Ordering;

use radium::Radium;

use crate::{
    base::{family::ThreadModel, group_construct::BusyGroup, segment::Segment},
    downloader::{
        group_async_parts::{AsyncParts, BusyGroup2, IdleGroup2, Residual, Slot2},
        group_download_methold::{DownloadContext, Downloader, SegmentDownload, SegmentResume},
    },
};

impl<'t, 'a , E, M: ThreadModel> BusyGroup<'t, 'a, M, AsyncParts<E>>{
    fn strealing_context<C: DownloadContext>(
        &self,
        min: u64,
        into_downloader: impl SegmentDownload<C>,
    ) -> impl Future {
        let reporter = self.task_stealing(min);

    }

    fn load_from_resumer<C: DownloadContext>(&self, loader: impl SegmentResume<C>) -> impl Future {
        let (segment, downloader) = loader.resume();
        let reporter = self.submit_segment(segment);
        downloader.download(ctx)
    }

    pub fn set_error(mut self, error: E) -> IdleGroup2<E, M> {
        unsafe {
            let slots = *(&mut self.slots_mut().0 as *mut _);
            todo!()
        }
        todo!()
    }

    pub fn find_max_remain(&self) -> Option<(&Slot2<E, M>, u64)> {
        let max = self.slots().0
            .iter()
            .map(|s| (s, s.share.ext.remain.load(Ordering::Relaxed)))
            .max_by_key(|t| t.1);
        max
        //max.map(|t| t.0)
    }
}

impl<'t, 'a, F, P> BusyGroup<'t, 'a, F, P> {
    pub fn find_max_remain(&self) -> Option<(&Slot2<E, M>, u64)> { // 注意这里 E 和 M 可能报错，需根据实际调整
        todo!()
    }
}
