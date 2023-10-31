use super::*;
// Section: wire functions

#[wasm_bindgen]
pub fn wire_init(
    port_: MessagePort,
    temp_dir: String,
    doc_dir: String,
    support_dir: String,
    cache_dir: String,
) {
    wire_init_impl(port_, temp_dir, doc_dir, support_dir, cache_dir)
}

#[wasm_bindgen]
pub fn wire_render_map_overlay(
    port_: MessagePort,
    zoom: f32,
    left: f64,
    top: f64,
    right: f64,
    bottom: f64,
) {
    wire_render_map_overlay_impl(port_, zoom, left, top, right, bottom)
}

#[wasm_bindgen]
pub fn wire_on_location_update(
    port_: MessagePort,
    latitude: f64,
    longitude: f64,
    timestamp_ms: i64,
    accuracy: f32,
    altitude: JsValue,
    speed: JsValue,
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

#[wasm_bindgen]
pub fn wire_list_all_raw_data(port_: MessagePort) {
    wire_list_all_raw_data_impl(port_)
}

#[wasm_bindgen]
pub fn wire_get_raw_data_mode(port_: MessagePort) {
    wire_get_raw_data_mode_impl(port_)
}

#[wasm_bindgen]
pub fn wire_toggle_raw_data_mode(port_: MessagePort, enable: bool) {
    wire_toggle_raw_data_mode_impl(port_, enable)
}

#[wasm_bindgen]
pub fn wire_finalize_ongoing_journey(port_: MessagePort) {
    wire_finalize_ongoing_journey_impl(port_)
}

// Section: allocate functions

// Section: related functions

// Section: impl Wire2Api

impl Wire2Api<String> for String {
    fn wire2api(self) -> String {
        self
    }
}

impl Wire2Api<Vec<u8>> for Box<[u8]> {
    fn wire2api(self) -> Vec<u8> {
        self.into_vec()
    }
}
// Section: impl Wire2Api for JsValue

impl<T> Wire2Api<Option<T>> for JsValue
where
    JsValue: Wire2Api<T>,
{
    fn wire2api(self) -> Option<T> {
        (!self.is_null() && !self.is_undefined()).then(|| self.wire2api())
    }
}
impl Wire2Api<String> for JsValue {
    fn wire2api(self) -> String {
        self.as_string().expect("non-UTF-8 string, or not a string")
    }
}
impl Wire2Api<bool> for JsValue {
    fn wire2api(self) -> bool {
        self.is_truthy()
    }
}
impl Wire2Api<f32> for JsValue {
    fn wire2api(self) -> f32 {
        self.unchecked_into_f64() as _
    }
}
impl Wire2Api<f64> for JsValue {
    fn wire2api(self) -> f64 {
        self.unchecked_into_f64() as _
    }
}
impl Wire2Api<i64> for JsValue {
    fn wire2api(self) -> i64 {
        ::std::convert::TryInto::try_into(self.dyn_into::<js_sys::BigInt>().unwrap()).unwrap()
    }
}
impl Wire2Api<u8> for JsValue {
    fn wire2api(self) -> u8 {
        self.unchecked_into_f64() as _
    }
}
impl Wire2Api<Vec<u8>> for JsValue {
    fn wire2api(self) -> Vec<u8> {
        self.unchecked_into::<js_sys::Uint8Array>().to_vec().into()
    }
}
