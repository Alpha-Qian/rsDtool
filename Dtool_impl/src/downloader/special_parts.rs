///功能门控
trait SpecialParts {
    type AbortToken: Abort;
    type Waker: Wake;
}

trait Wake {
    fn notify(self);
}

trait Abort {
    fn aborted(&mut self) -> bool;
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
