//!持久化

use std::sync::Arc;

//use super::
pub struct Segment{
    pub(crate) remain: u64,
    pub(crate) end: u64
}

struct Segments{
    inner: Vec<Segment>
}


pub struct SplitSegment{
    remain: u64,
    end: u64
}

impl SplitSegment{
    pub fn next_Segment(&mut self, len: u64) ->Option<Segment> {
        if self.remain > len {
            Some(Segment { remain: len, end })
        } else if self.remain > 0 {
            Some(())
        } else {
            None
        }
    }

    pub fn split_by_times(self, times: usize) -> SegmentIter{
        self.split_by_step(step)
    }

    pub fn split_by_step(self, step: u64) -> SegmentIter{
        SegmentIter { start , end, step }
    }
}


pub struct SegmentIter{
    remain: u64,
    end: u64,
    step: u64,
}

impl Iterator for SegmentIter {
    type Item = Segment;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remain > self.step {
            Some(Segment { remain: len, end })
        } else if self.remain > 0 {
            Some(())
        } else {
            None
        }
    }
}

trait SegmentCodec<S>{//S 是ProgreaaSender
    fn load_segment(segment: &Segment) -> (self, S){
        todo!()
    }

    fn save_as_segment(&self) -> Segment {
        todo!()
    }
}