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

fn poll_once<F: Future>(future: F) -> Option<F::Output> {
    let mut context = Context::from_waker(noop_waker_ref());
    let f = std::pin::pin!(future);
    match f.poll(&mut context) {
        Poll::Ready(result) => Some(result),
        Poll::Pending => None,
    }
}

fn unwarp_sync<F: Future>(future: F) -> F::Output {
    poll_once(future).expect("尝试在同步函数中调用异步函数")
}

trait SyncFuture: Future {
    fn run_sync(self) -> Self::Output
    where
        Self: Sized,
    {
        poll_once(self).expect("尝试在同步函数中调用异步函数")
    }
}

// struct EmptyWaker;

// impl Wake for EmptyWaker {
//     fn wake(self: std::sync::Arc<Self>) {
//         pan
//     }
// }

// enum Input {
//     Time(Instant),
// }

// enum Action {
//     GetTimer(),
// }

// trait MaybeAsync {
//     type Context<'a, 'b: 'a>;
// }

// struct Sync;
// impl MaybeAsync for Sync {
//     type Context<'a, 'b: 'a> = ();
// }

// struct Async;
// impl MaybeAsync for Async {
//     type Context<'a, 'b: 'a> = &'a mut Context<'b>;
// }

// // fn maybesync<F: MaybeAsync>(context: F::Context<'_, '_>, isize) ->

// trait MaybeFuture<T: MaybeAsync> {
//     type Output;

//     fn poll(self: Pin<&mut Self>, context: T::Context) -> Poll<Self::Output>;
// }

// ///
// ///
// ///scaling               prev_                  last_start      last_
// ///  |        ----------------------------          | ------------------------ now
// ///  |      |                              |        |                           |
// ///-----------------------------------------------------------------------time-->>
// ///
// ///
// struct Auto{

//     //config
//     thr: f32,
//     ace: f32,
//     min_task_count: usize,

//     scaling: f32,//record_the_max_speed_per_thread
//     scaling_arce: f32,

//     //prev_
//     prev_task_count: usize,
//     prev_downloaded: u64,
//     //prev_duration: Duration,
//     prev_arce: f32, // = 1 / prec_duration.as_sec_f32

//     last_start: Instant,

//     //last_
//     last_downloaded: u64,
//     last_task_count: usize
// }

// impl Auto{
//     fn new(ace: f64, thro: f64, min_task_count: usize) -> Self{
//         Self{
//             thr: thro,
//             ace,
//             min_task_count
//         }
//     }

//     fn
// }

// impl Strategy for Auto {
//     fn report_progress(&mut self, chunk_len: usize) {
//         self.last_downloaded += chunk_len as u64;
//     }

//     //仅在task_count改变时调用
//     fn report_task_count(&mut self, task_count: usize, now: Instant) {
//         let last_duration = now - self.last_start;
//         // if task_count != 0{
//         //     let speed_per_thread = self.last_downloaded as f32 / last_duration.as_secs_f32() / task_count as f32;
//         //     self.scaling = self.scaling.max(speed_per_thread);
//         // }

//         self.prev_task_count = self.last_task_count;
//         self.last_task_count = task_count;

//         self.prev_downloaded = self.last_downloaded;
//         self.last_downloaded = 0;

//         //self.prev_duration = last_duration;
//         self.prev_arce = 1_f32 / last_duration.as_secs_f32();
//         self.last_start = now;

//         if self.prev_arce > self.scaling_arce{

//         }
//     }

//     fn create_task_suggest(&mut self, now: Instant) -> bool {
//         let duration = now - self.last_start;
//         let second = duration.as_secs_f32();
//         let last_speed_per_new_thread = self.last_downloaded as f32 / second / self.last_task_count as f32;
//         how_to_name_this(self.scaling, last_speed_per_new_thread, self.thr, self.ace, second)
//     }
// }

// fn how_to_name_this(scaling: f32, last_speed_per_new_thread: f32, thold: f32, ace: f32, second: f32) -> bool{
//     //原始表达式： last_speed_per_new_thread / scaling > thold + ace / second
//     last_speed_per_new_thread * second > thold * second * scaling + ace * scaling
// }

// trait Hook{
//     async fn on_task_created(&mut self) {}
//     async fn on_task_return(&mut self) {}

// }

// ///基于曲率的计算
// struct Auto2{
//     speed1: f32,
//     task_count1: usize,

//     speed2: f32,
//     task_count2: usize,

//     last_downloaded: usize,
//     last_instant: Instant,

//     last_ff_output: f32
// }

// impl Auto2{
//     fn push(&mut self, speed: f32, task_count: usize) -> (f32, usize){
//         let speed_old = self.speed1;
//         let task_count_old = self.task_count1;

//         self.speed1 = self.speed2;
//         self.task_count1 = self.task_count2;

//         self.speed2 = speed;
//         self.task_count2 = task_count;
//         (speed_old, task_count_old)
//     }

//     fn f1(&self) -> f32 {
//         if self.task_count1 != self.task_count2{
//             (self.speed2 - self.speed1) / (self.task_count2 - self.task_count1) as f32
//         } else { 0_f32 }

//     }

//     fn f2(&self, speed_now: f32, task_count: usize) -> f32 {
//         if task_count != self.task_count2{
//             (speed_now - self.speed2) / (task_count - self.task_count2) as f32
//         } else { 0_f32 }
//     }

//     fn ff(&self, speed_now: f32, task_count: usize) -> f32 {
//         let task_count_diff = (task_count + self.task_count2) as f32 / 2_f32 - (self.task_count2 - self.task_count1) as f32 / 2_f32;
//         if task_count_diff != 0.0{
//             (self.f2(speed_now, task_count) - self.f1()) / task_count_diff
//         } else { 0.0 }

//     }

//     fn ff_div_avg_of_f1_and_f2(&self, speed_now: f32, task_count: usize) -> f32 {
//         let sum = self.f1() + self.f2(speed_now, task_count);
//         if sum != 0.0 {
//             self.ff(speed_now, task_count) / (self.f1() + self.f2(speed_now, task_count)) * 2.0
//             //   ff                      div       f1          f2                          avg   <- func name from here
//         } else if self.f1() == 0.0{
//             0.0
//         } else {
//             f32::MAX
//         }
//     }
// }
