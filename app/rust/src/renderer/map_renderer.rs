use crate::journey_area_utils;
use crate::journey_bitmap::JourneyBitmap;
use std::collections::HashMap;
pub struct MapRenderer {
    journey_bitmap: JourneyBitmap,
    /* for each tile of 512*512 tiles in a JourneyBitmap, use buffered area to record any update */
    tile_area_cache: HashMap<(u16, u16), f64>,
    version: u64,
    current_area: Option<u64>,
}

impl MapRenderer {
    pub fn new(journey_bitmap: JourneyBitmap) -> Self {
        Self {
            journey_bitmap,
            tile_area_cache: HashMap::new(),
            version: 0,
            current_area: None,
        }
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
        self.tile_area_cache.clear();
        self.reset();
    }

    fn reset(&mut self) {
        self.version = self.version.wrapping_add(1);
        self.current_area = None;
    }

    pub fn get_current_version(&self) -> u64 {
        self.version
    }

    pub fn get_version_string(&self) -> String {
        format!("\"{:x}\"", self.version)
    }

    pub fn parse_version_string(version_str: &str) -> Option<u64> {
        // Remove quotes if present
        let cleaned = version_str.trim_matches('"');
        u64::from_str_radix(cleaned, 16).ok()
    }

    pub fn get_latest_bitmap_if_changed(
        &self,
        client_version: Option<&str>,
    ) -> Option<(&JourneyBitmap, String)> {
        match client_version {
            Some(v_str) if (Self::parse_version_string(v_str) == Some(self.version)) => None,
            _ => Some((&self.journey_bitmap, self.get_version_string())),
        }
    }

    pub fn peek_latest_bitmap(&self) -> &JourneyBitmap {
        &self.journey_bitmap
    }

    pub fn get_current_area(&mut self) -> u64 {
        *self.current_area.get_or_insert_with(|| {
            journey_area_utils::compute_journey_bitmap_area(
                &self.journey_bitmap,
                Some(&mut self.tile_area_cache),
            )
        })
    }
}
