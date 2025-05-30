use crate::api::api::CameraOption;
use crate::journey_area_utils;
use crate::journey_bitmap::JourneyBitmap;
use std::collections::HashMap;
pub struct MapRenderer {
    journey_bitmap: JourneyBitmap,
    changed: bool,
    /* for each tile of 512*512 tiles in a JourneyBitmap, use buffered area to record any update */
    tile_area_cache: HashMap<(u16, u16), f64>,
    current_area: Option<u64>,
    // TODO: `provisioned_camera_option` should be moved out and passed to the
    // map separately.
    provisioned_camera_option: Option<CameraOption>,
}

impl MapRenderer {
    pub fn new(journey_bitmap: JourneyBitmap) -> Self {
        Self {
            journey_bitmap,
            changed: false,
            tile_area_cache: HashMap::new(),
            current_area: None,
            provisioned_camera_option: None,
        }
    }

    pub fn set_provisioned_camera_option(&mut self, camera_option: Option<CameraOption>) {
        self.provisioned_camera_option = camera_option;
    }

    pub fn get_provisioned_camera_option(&self) -> Option<CameraOption> {
        self.provisioned_camera_option
    }

    pub fn update<F>(&mut self, f: F)
    where
        F: Fn(&mut JourneyBitmap, &mut dyn FnMut((u16, u16))),
    {
        let mut tile_changed = |tile_pos: (u16, u16)| {
            self.tile_area_cache.remove(&tile_pos);
        };
        f(&mut self.journey_bitmap, &mut tile_changed);
        // TODO: we should improve the cache invalidation rule
        self.reset();
    }

    pub fn replace(&mut self, journey_bitmap: JourneyBitmap) {
        self.journey_bitmap = journey_bitmap;
        self.reset();
    }

    pub fn reset(&mut self) {
        self.changed = true;
    }

    pub fn changed(&self) -> bool {
        self.changed
    }

    pub fn get_latest_bitmap_if_changed(&mut self) -> Option<&JourneyBitmap> {
        if self.changed {
            self.changed = false;
            Some(&self.journey_bitmap)
        } else {
            None
        }
    }

    pub fn peek_latest_bitmap(&self) -> &JourneyBitmap {
        &self.journey_bitmap
    }

    pub fn get_current_area(&mut self) -> u64 {
        // TODO: we can do something more efficient here, instead of traversing
        // the whole bitmap evey time it changes.
        let area = journey_area_utils::compute_journey_bitmap_area(
            &self.journey_bitmap,
            Some(&mut self.tile_area_cache),
        );
        self.current_area = Some(area);
        area
    }
}
