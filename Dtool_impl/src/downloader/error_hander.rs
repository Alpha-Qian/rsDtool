use std::ops::ControlFlow;

use crate::downloader::{
    error::{SubError, SuperError},
    group_impl::DownloadStream,
    pwriter::BufWriter,
};
trait ErrorHander {
    type RawError;
    type BreakError;
    fn and_then(continue_error: Self::RawError) -> ControlFlow<Self::BreakError>;
}

// trait ErrorBarrier{
//     fn super_error<S: DownloadStream, W: BufWriter>(error: SuperError<S, W>) -> ControlFlow<>
// }

trait ErrorStrategy {
    fn error_pre_handle<S: DownloadStream, W: BufWriter>(
        sub: &SubError<S, W>,
    ) -> ControlFlow<SuperError<S, W>>;
    fn barrier_super_error<S: DownloadStream, W: BufWriter>(
        super_error: &SuperError<S, W>,
    ) -> ControlFlow<()>;
}
