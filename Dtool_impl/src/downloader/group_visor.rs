use std::{ops::ControlFlow, time::Instant};

use crate::{base::family::ThreadModel, downloader::group_manager::RunningManager};

///

///GroupVisor下发到每一个连接的监视器
trait ConnectVisor {
    fn downloaded(&mut self, length: usize);
}

fn visor_step(process: u64, now: Instant) {}

trait Visor {
    fn visor_step<E, M: ThreadModel>(
        &mut self,
        running: RunningManager<E, M>,
    ) -> ControlFlow<RunResult, RunningManager<E, M>>;
}
