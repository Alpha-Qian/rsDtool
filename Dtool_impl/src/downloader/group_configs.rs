use crate::downloader::group_download_methold::DownloadMethod;

///功能门控
trait SpecialParts {
    type DownloadMethod: DownloadMethod;

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

///关闭该功能
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
