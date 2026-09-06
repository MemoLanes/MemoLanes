pub fn xy_to_index(x: i64, y: i64, width_exp: i16) -> usize {
    (x + y * (1 << width_exp)) as usize
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
