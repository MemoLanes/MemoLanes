use std::{fs::File, io::Write};

use native::map_renderer::*;

use hex::ToHex;
use sha1::{Digest, Sha1};

#[test]
fn basic() {
    let mut map_renderer = MapRenderer::new();
    let render_result = map_renderer.maybe_render_map_overlay(
        11.0,
        151.1435370795134,
        -33.793291910360125,
        151.2783692841415,
        -33.943600147192235,
    );
    let render_result = render_result.unwrap();
    assert_eq!(render_result.left, 150.99609375);
    assert_eq!(render_result.top, -33.72433966174759);
    assert_eq!(render_result.right, 151.34765625);
    assert_eq!(render_result.bottom, -34.016241889667015);

    // capture image changes
    let mut hasher = Sha1::new();
    hasher.update(&render_result.data.0);
    let result = hasher.finalize();
    assert_eq!(
        result.encode_hex::<String>(),
        "3f97a9f76b3dd80bbac9d0b0b7cb30529afbe827"
    );

    // for human inspection
    let mut f = File::create("./tests/for_inspection/map_renderer_basic.png").unwrap();
    f.write_all(&render_result.data.0).unwrap();

    // a small move shouldn't trigger a re-render
    let render_result = map_renderer.maybe_render_map_overlay(
        11.0,
        151.143537079,
        -33.79329191036,
        151.278369284,
        -33.94360014719,
    );
    assert!(render_result.is_none());

    // but a bigger move will
    let render_result = map_renderer.maybe_render_map_overlay(11.0, 151.0, -33.0, 151.0, -33.0);
    assert!(render_result.is_some());
}
