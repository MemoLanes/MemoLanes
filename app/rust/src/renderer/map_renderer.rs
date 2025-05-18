use crate::api::api::CameraOption;
use crate::journey_area_utils;
use crate::journey_bitmap::JourneyBitmap;

pub struct MapRenderer {
    journey_bitmap: JourneyBitmap,
    version: u64,
    current_area: Option<u64>,
    // TODO: `provisioned_camera_option` should be moved out and passed to the
    // map separately.
    provisioned_camera_option: Option<CameraOption>,
}

impl MapRenderer {
    pub fn new(journey_bitmap: JourneyBitmap) -> Self {
        Self {
            journey_bitmap,
            version: 0,
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
        F: Fn(&mut JourneyBitmap),
    {
        f(&mut self.journey_bitmap);
        // TODO: we should improve the cache invalidation rule
        self.reset();
    }

    pub fn replace(&mut self, journey_bitmap: JourneyBitmap) {
        self.journey_bitmap = journey_bitmap;
        self.reset();
    }

    pub fn reset(&mut self) {
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

    pub fn get_latest_bitmap_if_changed(&self, client_version: Option<&str>) -> Option<(&JourneyBitmap, String)> {
        match client_version {
            Some(v_str) if Self::parse_version_string(v_str).map_or(false, |v| v == self.version) => None,
            _ => Some((&self.journey_bitmap, self.get_version_string())),
        }
    }

    pub fn peek_latest_bitmap(&self) -> &JourneyBitmap {
        &self.journey_bitmap
    }

    pub fn get_current_area(&mut self) -> u64 {
        // TODO: we can do something more efficient here, instead of traversing
        // the whole bitmap evey time it changes.
        match self.current_area {
            Some(area) => area,
            None => {
                let journey_bitmap = self.peek_latest_bitmap();
                let area = journey_area_utils::compute_journey_bitmap_area(journey_bitmap);
                self.current_area = Some(area);
                area
            }
        }
    }
}
