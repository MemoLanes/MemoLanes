pub fn xy_to_index(x: i64, y: i64, width_exp: i16) -> usize {
    (x + y * (1 << width_exp)) as usize
}

pub fn index_to_xy(index: usize, width_exp: i16) -> (i64, i64) {
    (
        index as i64 % (1 << width_exp),
        index as i64 / (1 << width_exp),
    )
}

#[allow(dead_code, reason = "wasm specific")]
pub fn set_panic_hook() {
    #[cfg(target_arch = "wasm32")]
    {
        static WASM_INIT: std::sync::Once = std::sync::Once::new();
        WASM_INIT.call_once(|| {
            // Route `log` facade output to browser console once per process.
            let _ = console_log::init_with_level(log::Level::Info);
            #[cfg(feature = "console_error_panic_hook")]
            console_error_panic_hook::set_once();
        });
    }
}

/// Normalize a southwest/northeast bounding box into (x_min, y_min, x_max, y_max),
/// clamping y to the Mercator range [0, 1].
#[allow(dead_code, reason = "wasm specific")]
pub fn normalize_mercator_bounds(
    sw_x: f64,
    sw_y: f64,
    ne_x: f64,
    ne_y: f64,
) -> (f64, f64, f64, f64) {
    let merc_x_min = sw_x.min(ne_x);
    let merc_x_max = sw_x.max(ne_x);
    let merc_y_min = sw_y.min(ne_y).clamp(0.0, 1.0);
    let merc_y_max = sw_y.max(ne_y).clamp(0.0, 1.0);
    (merc_x_min, merc_y_min, merc_x_max, merc_y_max)
}
