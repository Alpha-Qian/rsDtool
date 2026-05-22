
trait DoneWaker{
    fn wake(self);
}

impl<T> DoneWaker for T
where T: FnOnce()
{
    fn wake(self) {
        self()
    }
}


trait AbortToken{
    fn aborted(&mut self) -> bool;
}


impl<T> AbortToken for T
where T: FnMut() -> bool
{
    fn aborted(&mut self) -> bool {
        self()
    }
}