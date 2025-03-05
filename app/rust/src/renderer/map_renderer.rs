use crate::api::api::CameraOption;
use crate::journey_bitmap::JourneyBitmap;

#[derive(PartialEq, Eq)]
pub struct RenderArea {
    pub zoom: i32,
    pub left_idx: i32,
    pub top_idx: i32,
    pub right_idx: i32,
    pub bottom_idx: i32,
}

pub struct MapRenderer {
    journey_bitmap: JourneyBitmap,

    // For new web based renderer
    changed: bool,
    // TODO: when we switch to flutter side control, we should remove this
    provisioned_camera_option: Option<CameraOption>,
    current_render_area: Option<RenderArea>,
}

impl MapRenderer {
    pub fn new(journey_bitmap: JourneyBitmap) -> Self {
        Self {
            journey_bitmap,
            changed: false,
            provisioned_camera_option: None,
            current_render_area: None,
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
        self.changed = true;
        self.current_render_area = None;
    }

    pub fn replace(&mut self, journey_bitmap: JourneyBitmap) {
        self.journey_bitmap = journey_bitmap;
        self.changed = true;
        self.current_render_area = None;
    }

    pub fn reset(&mut self) {
        self.changed = true;
        self.current_render_area = None;
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

    pub fn peek_current_render_area(&self) -> Option<&RenderArea> {
        if self.current_render_area.is_some() {
            self.current_render_area.as_ref()
        } else {
            None
        }
    }

    pub fn set_current_render_area(&mut self, render_area: RenderArea) {
        self.current_render_area = Some(render_area);
    }
}
