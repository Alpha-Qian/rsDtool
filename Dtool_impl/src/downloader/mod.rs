//！reqwest 层

mod file_writer;
mod group;
mod group_async_parts;
mod group_async_parts_methods;
mod group_configs;
mod group_control;
mod group_default_downloader;
mod group_download;
mod group_download_methold;
mod group_downloader_interface;
mod group_executer;
mod group_init;
mod group_manager;
mod group_scheduler;
mod group_timeout;
mod group_too_slow_timeout;
mod group_visor;
mod group_worker;
mod main_builder;
pub(crate) mod retry_condition;
mod simple;
mod simple_init;
mod uninterrupt;
mod upgradable_download;
