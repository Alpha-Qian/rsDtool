struct AsyncBusyGroup


use crate::{
    base::family::ThreadModel,
    downloader::{
        group_async_parts::BusyGroup2,
        group_download_methold::{DownloadContext, Downloader, IntoDownloader, SegmentProvider},
    },
};

impl<'a, E, M: ThreadModel> BusyGroup2<'a, E, M> {
    fn strealing_context<C: DownloadContext>(
        &self,
        min: u64,
        into_downloader: impl IntoDownloader<C>,
    ) -> impl Future {
        let reporter = self.task_stealing(min);

    }

    fn load_from_provider<C: DownloadContext>(&self, loader: impl SegmentProvider<C>) -> impl Future {
        let (segment, downloader) = loader.provide_parts();
        let reporter = self.submit_segment(segment);
        downloader.download(ctx)
    }
}

enum Imposible {}
