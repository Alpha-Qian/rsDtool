//!处理整组的超时

use std::time::{Duration, Instant};

///更新并检查是否超时
trait TimeOutCheck {
    ///是否接收到数据，当前时间/接收数据时的时间
    fn check_timeout(&mut self, received: bool, now: Instant) -> bool;
}

///超时可以多等不可少等
struct TimeOutChecker {
    timeout: Duration,
    last_received: Instant,
}

impl TimeOutChecker {
    fn new(timeout: Duration, start_time: Instant) -> Self {
        Self {
            timeout,
            last_received: start_time,
        }
    }

    fn default(start_time: Instant) -> Self {
        Self::new(Duration::from_mins(1), start_time)
    }

    fn seconds(sec: u64, start_time: Instant) -> Self {
        Self::new(Duration::from_secs(sec), start_time)
    }
}

impl TimeOutCheck for TimeOutChecker {
    fn check_timeout(&mut self, received: bool, now: Instant) -> bool {
        if received {
            self.last_received = now;
            return false;
        }

        now >= self.last_received + self.timeout
    }
}
