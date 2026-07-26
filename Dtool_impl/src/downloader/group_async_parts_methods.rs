struct AsyncBusyGroup


use std::{num::NonZero, sync::atomic::Ordering};

use radium::Radium;

use crate::{
    base::{family::ThreadModel, group_construct::BusyGroup, segment::Segment},
    downloader::{
        group_async_parts::{AsyncParts, BusyGroup2, IdleGroup2, Residual, Slot2},
        group_download_methold::{DownloadContext, Downloader, SegmentDownload, SegmentResume},
    },
};

impl<'a, E, M: ThreadModel> BusyGroup2<'a, E, M> {
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

    pub fn find_max_remain(&self, min_length: u64) -> Option<(&Slot2<E, M>, u64)> {
        self.slots().0
            .iter()
            .map(|s| (s, s.share.ext.remain.load(Ordering::Relaxed)))
            .max_by_key(|t| t.1)
            .filter(|t| t.1 > min_length)
        //max.map(|t| t.0)
    }
}


pub fn into_no_zero(remain: i64) -> Option<NonZero<u64>> {
    u64::try_from(remain)
        .ok()
        .and_then(NonZero::new)
}
