pub trait DoneWaker {
    fn wake(self);
}

impl<T> DoneWaker for T
where
    T: FnOnce(),
{
    fn wake(self) {
        self()
    }
}

pub trait AbortToken {
    fn aborted(&mut self) -> bool;
}

impl<T> AbortToken for T
where
    T: FnMut() -> bool,
{
    fn aborted(&mut self) -> bool {
        self()
    }
}

trait StreamTracker {
    fn track(&mut self, len: usize);
}

impl<T> StreamTracker for T
where
    T: FnMut(usize),
{
    fn track(&mut self, len: usize) {
        self()
    }
}
