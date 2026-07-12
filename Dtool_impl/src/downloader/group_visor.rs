use std::time::Instant;

use crate::base::family::ThreadModel;

///

///GroupVisor下发到每一个连接的监视器
trait ConnectVisor {
    fn downloaded(&mut self, length: usize);
}

// trait GroupVisor {
//     fn connect_visor<'a>(&'a self) -> impl ConnectVisor + use<'a>;

//     fn connect_visor2<M: ThreadModel>(self: M::RefCounter<Self>) -> impl ConnectVisor;

//     fn flash(&self)
// }
//
//

fn visor_step(process: u64, now: Instant) {}
