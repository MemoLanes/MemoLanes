import assert from "node:assert/strict";
import test from "node:test";
import "./test-support/register-typescript.mjs";
import { polarUV, polarLngLat } from "./polar-projection.ts";
import { createPolarCameraConstrain } from "./polar-camera-policy.ts";

test("polar camera preserves geographic centres without singular fallback matrices", () => {
  const constrain = createPolarCameraConstrain(0, 18);
  for (const lat of [-90, 90]) {
    const center = { lng: 180, lat };
    const result = constrain(center, 14);
    assert.equal(result.center.lng, center.lng);
    assert.ok(Math.abs(result.center.lat - lat) < 0.000002);
    assert.ok(
      Math.abs(result.center.lat) < 90,
      "Mercator fallback matrices must stay finite",
    );
  }
});

for (const hemisphere of ["north", "south"]) {
  test(`${hemisphere} chart roundtrips every meridian and the pole`, () => {
    const sign = hemisphere === "north" ? 1 : -1;
    for (const lat of [85.0511287798066, 88, 89.999, 90])
      for (const lng of [-180, -90, 0, 90, 180]) {
        const uv = polarUV(lng, sign * lat);
        const ll = polarLngLat(...uv, hemisphere);
        assert.ok(Math.abs(ll[1] - sign * lat) < 1e-8);
        if (lat < 90)
          assert.ok(Math.abs(((ll[0] - lng + 540) % 360) - 180) < 1e-8);
      }
  });
}
