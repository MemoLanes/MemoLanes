#![allow(clippy::new_without_default)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;

mod api;
mod bridge_generated;

pub mod gps_processor;
pub mod journey_bitmap;
pub mod main_db;
pub mod map_renderer;
mod protos;
pub mod storage;
