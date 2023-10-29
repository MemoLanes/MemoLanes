use super::*;
// Section: wire functions

#[no_mangle]
pub extern "C" fn wire_init(
    port_: i64,
    temp_dir: *mut wire_uint_8_list,
    doc_dir: *mut wire_uint_8_list,
    support_dir: *mut wire_uint_8_list,
    cache_dir: *mut wire_uint_8_list,
) {
    wire_init_impl(port_, temp_dir, doc_dir, support_dir, cache_dir)
}

#[no_mangle]
pub extern "C" fn wire_render_map_overlay(
    port_: i64,
    zoom: f32,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
) {
    wire_render_map_overlay_impl(port_, zoom, left, top, right, bottom)
}

#[no_mangle]
pub extern "C" fn wire_on_location_update(
    port_: i64,
    latitude: f64,
    longitude: f64,
    timestamp_ms: i64,
    accuracy: f32,
    altitude: *mut f32,
    speed: *mut f32,
) {
    wire_on_location_update_impl(
        port_,
        latitude,
        longitude,
        timestamp_ms,
        accuracy,
        altitude,
        speed,
    )
}

#[no_mangle]
pub extern "C" fn wire_list_all_raw_data(port_: i64) {
    wire_list_all_raw_data_impl(port_)
}

#[no_mangle]
pub extern "C" fn wire_get_raw_data_mode(port_: i64) {
    wire_get_raw_data_mode_impl(port_)
}

#[no_mangle]
pub extern "C" fn wire_toggle_raw_data_mode(port_: i64, enable: bool) {
    wire_toggle_raw_data_mode_impl(port_, enable)
}

#[no_mangle]
pub extern "C" fn wire_finalize_ongoing_journey(port_: i64) {
    wire_finalize_ongoing_journey_impl(port_)
}

// Section: allocate functions

#[no_mangle]
pub extern "C" fn new_box_autoadd_f32_0(value: f32) -> *mut f32 {
    support::new_leak_box_ptr(value)
}

#[no_mangle]
pub extern "C" fn new_uint_8_list_0(len: i32) -> *mut wire_uint_8_list {
    let ans = wire_uint_8_list {
        ptr: support::new_leak_vec_ptr(Default::default(), len),
        len,
    };
    support::new_leak_box_ptr(ans)
}

// Section: related functions

// Section: impl Wire2Api

impl Wire2Api<String> for *mut wire_uint_8_list {
    fn wire2api(self) -> String {
        let vec: Vec<u8> = self.wire2api();
        String::from_utf8_lossy(&vec).into_owned()
    }
}

impl Wire2Api<f32> for *mut f32 {
    fn wire2api(self) -> f32 {
        unsafe { *support::box_from_leak_ptr(self) }
    }
}

impl Wire2Api<Vec<u8>> for *mut wire_uint_8_list {
    fn wire2api(self) -> Vec<u8> {
        unsafe {
            let wrap = support::box_from_leak_ptr(self);
            support::vec_from_leak_ptr(wrap.ptr, wrap.len)
        }
    }
}
// Section: wire structs

#[repr(C)]
#[derive(Clone)]
pub struct wire_uint_8_list {
    ptr: *mut u8,
    len: i32,
}

// Section: impl NewWithNullPtr

pub trait NewWithNullPtr {
    fn new_with_null_ptr() -> Self;
}

impl<T> NewWithNullPtr for *mut T {
    fn new_with_null_ptr() -> Self {
        std::ptr::null_mut()
    }
}

// Section: sync execution mode utility

#[no_mangle]
pub extern "C" fn free_WireSyncReturn(ptr: support::WireSyncReturn) {
    unsafe {
        let _ = support::box_from_leak_ptr(ptr);
    };
}
