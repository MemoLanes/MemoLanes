@group(0) @binding(0)
var<storage, read> input: array<u32>;

@group(0) @binding(1)
var<storage, read_write> output: array<u32>;

@group(0) @binding(2)
var<uniform> dimensions: vec2<u32>;

const KERNEL_SIZE: u32 = 5;

// const KERNEL: array<array<f32, 5>, 5> = array<array<f32, 5>, 5>(
//     array<f32, 5>(0.3, 0.5, 0.7, 0.5, 0.3),
//     array<f32, 5>(0.5, 1.0, 1.0, 1.0, 0.5),
//     array<f32, 5>(0.7, 1.0, 1.0, 1.0, 0.7),
//     array<f32, 5>(0.5, 1.0, 1.0, 1.0, 0.5),
//     array<f32, 5>(0.3, 0.5, 0.7, 0.5, 0.3)
// );

fn get_kernel_weight(ky: u32, kx: u32) -> f32 {
    switch (ky * KERNEL_SIZE + kx) {
        case 0u, 4u, 20u, 24u: { return 0.3; }
        case 1u, 3u, 5u, 9u, 15u, 19u, 21u, 23u: { return 0.5; }
        case 2u, 10u, 14u, 22u: { return 0.7; }
        default: { return 1.0; }
    }
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let width = dimensions.x;
    let height = dimensions.y;

    let x = global_id.x;
    let y = global_id.y;

    let self_idx = y * width + x;

    let self_alpha = f32(input[self_idx] >> 24u) / 255.0;
    var min_alpha = 1.0;
    var max_r = 0u;
    var max_g = 0u;
    var max_b = 0u;

    for (var ky = 0u; ky < KERNEL_SIZE; ky++) {
        for (var kx = 0u; kx < KERNEL_SIZE; kx++) {
            // Skip border pixels
            let px = clamp(i32(x) + i32(kx) - 2, 0, i32(width - 1));
            let py = clamp(i32(y) + i32(ky) - 2, 0, i32(height - 1));
            let weight = get_kernel_weight(ky, kx);

            let idx = u32(py) * width + u32(px);
            let pixel = input[idx];
            let alpha = f32(pixel >> 24u) / 255.0 * weight + self_alpha * (1.0 - weight);

            if (alpha < min_alpha) {
                min_alpha = alpha;
                max_r = pixel & 0xFFu;
                max_g = (pixel >> 8u) & 0xFFu;
                max_b = (pixel >> 16u) & 0xFFu;
            }
        }
    }

    let result = (u32(min_alpha * 255.0) << 24u) | (max_b << 16u) | (max_g << 8u) | max_r;
    output[self_idx] = result;
}