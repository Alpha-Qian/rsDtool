use std::ops::ControlFlow;

use crate::{base::family::ThreadModel, downloader::group_worker::SegmentWorker};

pub trait DownloadMethod {
    type Error;

    fn download_segment<M: ThreadModel>(
        worker: SegmentWorker<M>,
    ) -> Result<ControlFlow<()>, Self::Error>;
}
