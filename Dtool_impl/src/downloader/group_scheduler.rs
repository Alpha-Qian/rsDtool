use std::ops::ControlFlow;

use crate::{
    base::{
        family::ThreadModel,
        group_construct::{GroupParts, Slot},
    },
    downloader::group_manager::RunningManager,
};

fn find_minest_slot<'a, I, M, P>(slots: I) -> &Slot<'a, M, P>
where
    I: Iterator<Item = &Slot<'a, M, P>>,
    M: ThreadModel,
    P: GroupParts,
{
    todo!()
}

// trait Schedule {
//     fn schedule_in_step<D, M>(
//         running: RunningManager<D, M>,
//     ) -> ControlFlow<RunResult, RunningManager<D, M>>;
// }

// fn find_largest_slot<'a, I, M, P>(slots: I) -> &Slot<'a, M, P>
// where
//     I: Iterator<Item = &Slot<'a, M, P>>,
//     M: ThreadModel,
//     P: GroupParts,
// {
//     todo!()
// }
