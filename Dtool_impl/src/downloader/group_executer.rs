use ouroboros::self_referencing;
use std::marker::PhantomData;

use crate::{
    base::{family::ThreadModel, group_construct::Slot, segment::Segment},
    downloader::{
        group_async_parts::{AsyncParts, BusyGroup2, DownloadGroup2},
        group_download_methold::SegmentDownload,
    },
};

// #[self_referencing]
// struct Futurer<'a, D, M> {
//     group: BusyGroup2<'a, D, M>,
//     #[borrows(group)]
//     iter: I,
// }

struct FutureIter<'a, M: ThreadModel, E, I> {
    iter: I,
    group: BusyGroup2<'a, E, M>,
}

impl<'a, M: ThreadModel, E, I> FutureIter<'a, M, E, I> {
    fn new(group: BusyGroup2<'a, E, M>) -> FutureIter<'a, M, E, impl Iterator<Item = Segment>> {
        FutureIter {
            iter: group.slots().0.iter().map(|s| s.save_as_segment()),
            group,
        }
    }

    fn next_future(
        self: &mut FutureIter<'a, M, E, impl Iterator<Item = Segment>>,
    ) -> Option<impl Future> {
        self.iter.next().map(todo!())
    }

    fn next_segment(
        self: &mut FutureIter<'a, M, E, impl Iterator<Item = Segment>>,
    ) -> Option<Segment> {
        self.iter.next()
    }

    fn test(self: FutureIter<'a, M, E, I>) -> BusyGroup2<'a, E, M> {
        self.group
    }
}
fn test() {
    let group: BusyGroup2<_, _> = todo!();
    let iter = FutureIter::new(group);
    let fut = iter.next_future();
}
