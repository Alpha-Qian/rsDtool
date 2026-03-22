//!持久化

use std::sync::Arc;

use crate::downloader::request::DownloadRequest;



pub struct ResumeInfo{
    request: DownloadRequest,
    segments: Vec<Segment>
}


pub trait CodecSegment{//S 是ProgreaaSender //simple
    type ProcessSender;
    ///加载进度并启动
    fn load_segment(segment: Segment) -> (self, Self::ProcessSender);

    ///中途保存进度
    fn save_as_segment(&self) -> Segment;
}


struct Segments{
    inner: Vec<Segment>
}

#[derive(Clone)]
pub struct Segment{
    pub(crate) remain: u64,
    pub(crate) end: u64
}

impl Segment {
    pub fn full(size: u64) -> self{
        Self{remain: size, end: size}
    }
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
        SegmentIter { remain: self.remain, end: self.end, step }
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


