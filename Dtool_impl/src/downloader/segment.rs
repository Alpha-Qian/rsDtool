//!持久化

use std::{error::Error, fmt::{Debug, Display}, mem, num::NonZeroUsize, ops::RangeBounds, sync::Arc};
use std::num::NonZeroU64;
use crate::downloader::httprequest::RequestInfo;



pub struct ResumeInfo{
    request: RequestInfo,
    segments: Vec<Segment>
}


#[derive(Clone)]
pub struct Segment{
    pub start: u64,
    pub remain: NonZeroU64,
}

impl Segment {

    pub fn new(start: u64,remain: NonZeroU64) -> Self{
        Self { start, remain }
    }

    // pub fn from_end_and_remain(remain: NonZeroU64, end: u64) -> Option<Self> {
    //     Self::new(end.checked_sub(remain.into())? , remain).into()
    // }

    pub fn full(size: NonZeroU64) -> Self{
        Self::new(0, size)
    }

    pub fn end(&self) -> u64 {
        self.start + self.remain.get()
    }

    pub fn split_at(self, first_remain: NonZeroU64) -> (Self, Option<Self>) {
        let mut iter = self.split_by_step(first_remain);
        (iter.next().unwrap(), iter.try_into().ok())
    }

    pub fn split_by_times(self, times: NonZeroUsize) -> SegmentIter {
        let fix_remain = self.remain.get() + times.get() as u64;
        self.split_by_step((fix_remain / times.get() as u64).try_into().unwrap())
    }

    pub fn split_by_step(self, step: NonZeroU64) -> SegmentIter {
        SegmentIter::new(self.start, self.remain.into(), step)
    }
}


// impl<T: RangeBounds<u64>> TryFrom<T> for Segment {
//     fn try_from(value: T) -> Result<Self, Self::Error> {
        
//     }
// }

pub struct SegmentIter{
    step: NonZeroU64,

    //
    remain: Option<NonZeroU64>,
    start: u64
}

impl Iterator for SegmentIter {
    type Item = Segment;
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(remain) = self.remain {
            if remain > self.step {
                let origin_start = self.start;
                self.remain = NonZeroU64::new(remain.get() - self.step.get());//remain -= step
                self.start += self.step.get();
                Segment::new(origin_start, self.step).into()
            } else {
                self.remain = None;
                Segment::new(self.start, remain).into()
            }

        } else {
            None
        }
    }
}

impl TryFrom<SegmentIter> for Segment{
    type Error = FromSegmentIterError;
    fn try_from(value: SegmentIter) -> Result<Self, Self::Error> {
        todo!()
    }
}

impl SegmentIter {
    fn new(start: u64, remain: Option<NonZeroU64>, step: NonZeroU64) -> Self {
        Self { step, remain, start }
    }

    // fn end(&self)
}


#[derive(Debug)]
struct FromSegmentIterError;

impl Display for FromSegmentIterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("无法转换已耗尽迭代器")
    }
}
impl Error for FromSegmentIterError{}
