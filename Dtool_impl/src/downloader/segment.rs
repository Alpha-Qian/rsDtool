//!持久化

use std::{error::Error, fmt::{Debug, Display}, mem, sync::Arc};
use std::num::NonZeroU64;
use crate::downloader::httprequest::RequestInfo;



pub struct ResumeInfo{
    request: RequestInfo,
    segments: Vec<Segment>
}


// pub trait CodecSegment{//S 是ProgreaaSender //simple
//     type ProcessSender;
//     ///加载进度并启动
//     fn load_segment(segment: Segment) -> (self, Self::ProcessSender);

//     ///中途保存进度
//     fn save_as_segment(&self) -> Segment;
// }


// struct Segments{
//     inner: Vec<Segment>
// }

#[derive(Clone)]
pub struct Segment{
    pub(crate) remain: u64,
    pub(crate) end: u64
}

impl Segment {

    pub(crate) fn new(remain: u64, end: u64) -> Self{
        Self { remain, end }
    }

    pub fn full(size: u64) -> self{
        Self{remain: size, end: size}
    }

    pub fn start(&self) -> u64{
        self.end - self.remain
    }

    pub fn split_at(self, first_remain: u64) -> (Self, Option<Self>) {
        let mut iter = self.split_by_step(first_remain);
        (iter.next().unwrap(), iter.try_into::<Self>().ok())
    }

    pub fn split_by_times(self, times: usize) -> SegmentIter{
        self.split_by_step(step)
    }

    pub fn split_by_step(self, step: u64) -> SegmentIter{
        SegmentIter { remain: self.remain, end: self.end, step }
    }
}

pub struct SegmentIter{
    step: u64,

    //
    remain: u64,
    end: u64,
}

impl Iterator for SegmentIter {
    type Item = Segment;
    fn next(&mut self) -> Option<Self::Item> {
        if self.remain > self.step {

            self.remain -= self.step;
            Segment::new(self.step, self.start() ).into()

        } else if self.remain > 0 {

            self.remain = 0;//迭代器耗尽
            Segment::new(remain, end).into()

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
    fn new(step: u64, remain: u64, end: u64) -> Self{
        Self { step, remain, end }
    }

    fn start(&self) -> u64{
        self.end - self.remain
    }
}


impl From<FromSegmentIterError> for CreateSegmentError {
    fn from(value: FromSegmentIterError) -> Self {
        Self::ZeroRemain
    }
}

#[derive(Debug)]
struct FromSegmentIterError;

impl Display for FromSegmentIterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&"无法转换已耗尽迭代器")
    }
}
impl Error for FromSegmentIterError{}


#[derive(Debug)]
enum CreateSegmentError {
    ZeroRemain,
    NegtiveStartPoint
}


// impl Debug for  {
    
// }
impl Display for CreateSegmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match Self {
            Self::ZeroRemain => todo!(),
            Self::NegtiveStartPoint => todo!()
        }
    }
}