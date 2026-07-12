//!下载速度太慢导致的错误

use std::time::{Duration, Instant};

///超时可以多等不能少等
struct TooSlowTimeOut {
    too_slow_time: Duration,
    d_progress: u64,

    last_progress: u64,
    last_check: Instant,
}

impl TooSlowTimeOut {
    fn check_time_out(&mut self, progress: u64, now: Instant) -> bool {
        if now - self.last_check >= self.too_slow_time
            && progress - self.last_progress < self.d_progress
        {
            return true;
        }

        if (progress - self.last_progress) / (now - self.last_check).as_secs_f32()
            < self.d_progress / self.too_slow_time.as_secs_f32()
        {
            //speed Slow than timeout speed
            return false;
        } else {
            self.last_progress = progress;
            self.last_check = now;
            return false;
        }
    }

    fn check_speed() -> bool {
        todo!()
    }

    fn check_update() {
        todo!()
    }
}
