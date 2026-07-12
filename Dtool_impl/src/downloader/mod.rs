//！reqwest 层

mod file_writer;
mod group;
mod group_control;
mod group_download;
mod group_error;
mod group_manager;
mod group_timeout;
mod group_too_slow_timeout;
mod group_visor;
mod group_worker;
mod main_builder;
pub(crate) mod retry_condition;
mod simple;
mod special_parts;
mod uninterrupt;
mod upgradable_download;
