use std::{
    task::{Context, Poll},
    time::{Duration, Instant},
};

use futures::task::noop_waker_ref;

trait Strategy {
    fn report_progress(&mut self, chunk_len: usize);

    fn report_task_count(&mut self, offset: usize, now: Instant);

    fn create_task_suggest(&mut self, now: Instant) -> (bool, Duration /* next_get_suggest */);
}
