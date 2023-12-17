// TODO: thinking about improving the folder structure.

#![allow(clippy::new_without_default)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;

pub mod api;
mod bridge_generated;

pub mod archive;
pub mod gps_processor;
pub mod journey_bitmap;
pub mod journey_data;
pub mod journey_header;
pub mod journey_vector;
pub mod main_db;
pub mod map_renderer;
mod merged_journey_manager;
mod protos;
pub mod storage;
pub mod tile_renderer;
mod utils;
