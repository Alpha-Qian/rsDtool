use crate::base::{
    family::ThreadModel,
    group_construct::{GroupParts, Slot},
};

fn find_minest_slot<'a, I, M, P>(slots: I) -> &Slot<'a, M, P>
where
    I: Iterator<Item = &Slot<'a, M, P>>,
    M: ThreadModel,
    P: GroupParts,
{
    todo!()
}

// fn find_largest_slot<'a, I, M, P>(slots: I) -> &Slot<'a, M, P>
// where
//     I: Iterator<Item = &Slot<'a, M, P>>,
//     M: ThreadModel,
//     P: GroupParts,
// {
//     todo!()
// }
