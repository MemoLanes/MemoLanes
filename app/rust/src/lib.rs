#![allow(clippy::new_without_default)]

#[macro_use]
extern crate log;
#[macro_use]
extern crate anyhow;
#[macro_use]
extern crate lazy_static;

#[rustfmt::skip]
mod frb_generated; /* AUTO INJECTED BY flutter_rust_bridge. This line may not be accurate, and you can change it according to your needs. */
#[rustfmt::skip]
pub mod build_info;

pub mod api;
pub mod archive;
pub mod cache_db;
pub mod export_data;
pub mod flight_track_processor;
pub mod gps_processor;
pub mod import_data;
pub mod journey_area_utils;
pub mod journey_bitmap;
pub mod journey_data;
pub mod journey_date_picker;
pub mod journey_header;
pub mod journey_vector;
mod logs;
pub mod main_db;
pub mod merged_journey_builder;
pub mod preclean;
mod protos;
pub mod renderer;
pub mod storage;
pub mod utils;
