//! 检查错误是否可重试
//! 对于单线程下载和多线程下载的第一个线程，错误重复一定次数后抛出
//! 多线程下载总体速度太慢会出错
//!处理重试次数上限和可重试错误判断
//!
use std::{error::Error, ops::ControlFlow};

pub fn reqwest_retryable(error: &reqwest::Error) -> bool {
    todo!()
}
