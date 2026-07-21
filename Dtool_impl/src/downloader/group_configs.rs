use crate::downloader::group_download_methold::SegmentDownload;

///功能门控
trait SpecialParts {
    type AbortGroup: Abort;
    type AbortGroupHandle: AbortHandle;

    type AbortSingle: Abort;
    type AbortSingleHandle: AbortHandle;

    type Waker: Wake;
}

trait Wake {
    fn notify(self);
}

trait Abort {
    fn aborted(&mut self) -> bool;
}

trait AbortHandle {
    fn abort();
}

trait DownloadCount {
    fn count(length: usize);
}

///关闭该功能
/// Identy
struct Disable;

impl Wake for Disable {
    fn notify(self) { /* Disable */
    }
}

impl Abort for Disable {
    fn aborted(&mut self) -> bool {
        false /* Disable */
    }
}

impl AbortHandle for Disable {
    fn abort() {}
}

impl DownloadCount for Disable {
    fn count(length: usize) {}
}
