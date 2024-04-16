use std::cmp::min;

pub fn gaussian_blur(data: &mut [u8], width: usize, height: usize, blur_radius: f32) {
    let boxes = create_box_gauss(blur_radius, 3);
    let mut backbuf = data.to_vec().clone();

    for box_size in boxes.iter() {
        let radius = ((box_size - 1) / 2) as usize;
        box_blur(&mut backbuf, data, width, height, radius, radius);
    }
}

#[inline]
/// If there is no valid size (e.g. radius is negative), returns `vec![1; len]`
/// which would translate to blur radius of 0
fn create_box_gauss(sigma: f32, n: usize) -> Vec<i32> {
    if sigma > 0.0 {
        let n_float = n as f32;

        // Ideal averaging filter width
        let w_ideal = (12.0 * sigma * sigma / n_float).sqrt() + 1.0;
        let mut wl: i32 = w_ideal.floor() as i32;

        if wl % 2 == 0 {
            wl -= 1;
        };

        let wu = wl + 2;

        let wl_float = wl as f32;
        let m_ideal = (12.0 * sigma * sigma
            - n_float * wl_float * wl_float
            - 4.0 * n_float * wl_float
            - 3.0 * n_float)
            / (-4.0 * wl_float - 4.0);
        let m: usize = m_ideal.round() as usize;

        let mut sizes = Vec::<i32>::new();

        for i in 0..n {
            if i < m {
                sizes.push(wl);
            } else {
                sizes.push(wu);
            }
        }

        sizes
    } else {
        vec![1; n]
    }
}

/// Needs 2x the same image
#[inline]
fn box_blur(
    backbuf: &mut [u8],
    frontbuf: &mut [u8],
    width: usize,
    height: usize,
    blur_radius_horz: usize,
    blur_radius_vert: usize,
) {
    box_blur_horz(backbuf, frontbuf, width, height, blur_radius_horz);
    box_blur_vert(frontbuf, backbuf, width, height, blur_radius_vert);
}

#[inline]
fn box_blur_vert(
    backbuf: &[u8],
    frontbuf: &mut [u8],
    width: usize,
    height: usize,
    blur_radius: usize,
) {
    if blur_radius == 0 {
        frontbuf.copy_from_slice(backbuf);
        return;
    }

    let iarr = 1.0 / (blur_radius + blur_radius + 1) as f32;

    for i in 0..width {
        let col_start = i * 4; //inclusive
        let col_end = col_start + width * (height - 1) * 4; //inclusive
        let mut ti: usize = i * 4;
        let mut li: usize = ti;
        let mut ri: usize = ti + blur_radius * width * 4;

        let fv: [u8; 4] = [
            backbuf[col_start],
            backbuf[col_start + 1],
            backbuf[col_start + 2],
            backbuf[col_start + 3],
        ];
        let lv: [u8; 4] = [
            backbuf[col_end],
            backbuf[col_end + 1],
            backbuf[col_end + 2],
            backbuf[col_end + 3],
        ];

        let mut val_r: isize = (blur_radius as isize + 1) * isize::from(fv[0]);
        let mut val_g: isize = (blur_radius as isize + 1) * isize::from(fv[1]);
        let mut val_b: isize = (blur_radius as isize + 1) * isize::from(fv[2]);
        let mut val_a: isize = (blur_radius as isize + 1) * isize::from(fv[3]);

        // Get the pixel at the specified index, or the first pixel of the column
        // if the index is beyond the top edge of the image
        let get_top = |i: usize| {
            if i < col_start {
                fv
            } else {
                [backbuf[i], backbuf[i + 1], backbuf[i + 2], backbuf[i + 3]]
            }
        };

        // Get the pixel at the specified index, or the last pixel of the column
        // if the index is beyond the bottom edge of the image
        let get_bottom = |i: usize| {
            if i > col_end {
                lv
            } else {
                [backbuf[i], backbuf[i + 1], backbuf[i + 2], backbuf[i + 3]]
            }
        };

        for j in 0..min(blur_radius, height) {
            let t = ti + j * width * 4;
            let bb = [backbuf[t], backbuf[t + 1], backbuf[t + 2], backbuf[t + 3]];
            val_r += isize::from(bb[0]);
            val_g += isize::from(bb[1]);
            val_b += isize::from(bb[2]);
            val_a += isize::from(bb[3]);
        }
        if blur_radius > height {
            val_r += (blur_radius - height) as isize * isize::from(lv[0]);
            val_g += (blur_radius - height) as isize * isize::from(lv[1]);
            val_b += (blur_radius - height) as isize * isize::from(lv[2]);
            val_a += (blur_radius - height) as isize * isize::from(lv[3]);
        }

        for _ in 0..min(height, blur_radius + 1) {
            let bb = get_bottom(ri);
            ri += width * 4;
            val_r += isize::from(bb[0]) - isize::from(fv[0]);
            val_g += isize::from(bb[1]) - isize::from(fv[1]);
            val_b += isize::from(bb[2]) - isize::from(fv[2]);
            val_a += isize::from(bb[3]) - isize::from(fv[3]);

            frontbuf[ti] = round(val_r as f32 * iarr) as u8;
            frontbuf[ti + 1] = round(val_g as f32 * iarr) as u8;
            frontbuf[ti + 2] = round(val_b as f32 * iarr) as u8;
            frontbuf[ti + 3] = round(val_a as f32 * iarr) as u8;

            ti += width * 4;
        }

        if height > blur_radius {
            // otherwise `(height - blur_radius)` will underflow
            for _ in (blur_radius + 1)..(height - blur_radius) {
                let bb1 = [
                    backbuf[ri],
                    backbuf[ri + 1],
                    backbuf[ri + 2],
                    backbuf[ri + 3],
                ];
                ri += width * 4;
                let bb2 = [
                    backbuf[li],
                    backbuf[li + 1],
                    backbuf[li + 2],
                    backbuf[li + 3],
                ];
                li += width * 4;

                val_r += isize::from(bb1[0]) - isize::from(bb2[0]);
                val_g += isize::from(bb1[1]) - isize::from(bb2[1]);
                val_b += isize::from(bb1[2]) - isize::from(bb2[2]);
                val_a += isize::from(bb1[3]) - isize::from(bb2[3]);

                frontbuf[ti] = round(val_r as f32 * iarr) as u8;
                frontbuf[ti + 1] = round(val_g as f32 * iarr) as u8;
                frontbuf[ti + 2] = round(val_b as f32 * iarr) as u8;
                frontbuf[ti + 3] = round(val_a as f32 * iarr) as u8;
                ti += width * 4;
            }

            for _ in 0..min(height - blur_radius - 1, blur_radius) {
                let bb = get_top(li);
                li += width;

                val_r += isize::from(lv[0]) - isize::from(bb[0]);
                val_g += isize::from(lv[1]) - isize::from(bb[1]);
                val_b += isize::from(lv[2]) - isize::from(bb[2]);
                val_a += isize::from(lv[3]) - isize::from(bb[3]);

                frontbuf[ti] = round(val_r as f32 * iarr) as u8;
                frontbuf[ti + 1] = round(val_g as f32 * iarr) as u8;
                frontbuf[ti + 2] = round(val_b as f32 * iarr) as u8;
                frontbuf[ti + 3] = round(val_a as f32 * iarr) as u8;
                ti += width * 4;
            }
        }
    }
}

#[inline]
fn box_blur_horz(
    backbuf: &[u8],
    frontbuf: &mut [u8],
    width: usize,
    height: usize,
    blur_radius: usize,
) {
    if blur_radius == 0 {
        frontbuf.copy_from_slice(backbuf);
        return;
    }

    let iarr = 1.0 / (blur_radius + blur_radius + 1) as f32;

    for i in 0..height {
        let row_start: usize = i * width * 4; // inclusive
        let row_end: usize = row_start + width * 4 - 4; // inclusive
        let mut ti: usize = i * width * 4; // VERTICAL: $i;
        let mut li: usize = ti;
        let mut ri: usize = ti + blur_radius * 4;

        let fv: [u8; 4] = [
            backbuf[row_start],
            backbuf[row_start + 1],
            backbuf[row_start + 2],
            backbuf[row_start + 3],
        ];
        let lv: [u8; 4] = [
            backbuf[row_end],
            backbuf[row_end + 1],
            backbuf[row_end + 2],
            backbuf[row_end + 3],
        ]; // VERTICAL: $backbuf[ti + $width - 1];

        let mut val_r: isize = (blur_radius as isize + 1) * isize::from(fv[0]);
        let mut val_g: isize = (blur_radius as isize + 1) * isize::from(fv[1]);
        let mut val_b: isize = (blur_radius as isize + 1) * isize::from(fv[2]);
        let mut val_a: isize = (blur_radius as isize + 1) * isize::from(fv[3]);

        // Get the pixel at the specified index, or the first pixel of the row
        // if the index is beyond the left edge of the image
        let get_left = |i: usize| {
            if i < row_start {
                fv
            } else {
                [backbuf[i], backbuf[i + 1], backbuf[i + 2], backbuf[i + 3]]
            }
        };

        // Get the pixel at the specified index, or the last pixel of the row
        // if the index is beyond the right edge of the image
        let get_right = |i: usize| {
            if i > row_end {
                lv
            } else {
                [backbuf[i], backbuf[i + 1], backbuf[i + 2], backbuf[i + 3]]
            }
        };

        for j in 0..min(blur_radius, width) {
            let t = ti + j * 4;
            let bb = [backbuf[t], backbuf[t + 1], backbuf[t + 2], backbuf[t + 3]]; // VERTICAL: ti + j * width
            val_r += isize::from(bb[0]);
            val_g += isize::from(bb[1]);
            val_b += isize::from(bb[2]);
            val_a += isize::from(bb[3]);
        }
        if blur_radius > width {
            val_r += (blur_radius - height) as isize * isize::from(lv[0]);
            val_g += (blur_radius - height) as isize * isize::from(lv[1]);
            val_b += (blur_radius - height) as isize * isize::from(lv[2]);
            val_a += (blur_radius - height) as isize * isize::from(lv[3]);
        }

        // Process the left side where we need pixels from beyond the left edge
        for _ in 0..min(width, blur_radius + 1) {
            let bb = get_right(ri);
            ri += 4;
            val_r += isize::from(bb[0]) - isize::from(fv[0]);
            val_g += isize::from(bb[1]) - isize::from(fv[1]);
            val_b += isize::from(bb[2]) - isize::from(fv[2]);
            val_a += isize::from(bb[3]) - isize::from(fv[3]);

            frontbuf[ti] = round(val_r as f32 * iarr) as u8;
            frontbuf[ti + 1] = round(val_g as f32 * iarr) as u8;
            frontbuf[ti + 2] = round(val_b as f32 * iarr) as u8;
            frontbuf[ti + 3] = round(val_a as f32 * iarr) as u8;
            ti += 4; // VERTICAL : ti += width, same with the other areas
        }

        if width > blur_radius {
            // otherwise `(width - blur_radius)` will underflow
            // Process the middle where we know we won't bump into borders
            // without the extra indirection of get_left/get_right. This is faster.
            for _ in (blur_radius + 1)..(width - blur_radius) {
                let bb1 = [
                    backbuf[ri],
                    backbuf[ri + 1],
                    backbuf[ri + 2],
                    backbuf[ri + 3],
                ];
                ri += 4;
                let bb2 = [
                    backbuf[li],
                    backbuf[li + 1],
                    backbuf[li + 2],
                    backbuf[li + 3],
                ];
                li += 4;

                val_r += isize::from(bb1[0]) - isize::from(bb2[0]);
                val_g += isize::from(bb1[1]) - isize::from(bb2[1]);
                val_b += isize::from(bb1[2]) - isize::from(bb2[2]);
                val_a += isize::from(bb1[3]) - isize::from(bb2[3]);

                frontbuf[ti] = round(val_r as f32 * iarr) as u8;
                frontbuf[ti + 1] = round(val_g as f32 * iarr) as u8;
                frontbuf[ti + 2] = round(val_b as f32 * iarr) as u8;
                frontbuf[ti + 3] = round(val_a as f32 * iarr) as u8;
                ti += 4;
            }

            // Process the right side where we need pixels from beyond the right edge
            for _ in 0..min(width - blur_radius - 1, blur_radius) {
                let bb = get_left(li);
                li += 4;

                val_r += isize::from(lv[0]) - isize::from(bb[0]);
                val_g += isize::from(lv[1]) - isize::from(bb[1]);
                val_b += isize::from(lv[2]) - isize::from(bb[2]);
                val_a += isize::from(lv[3]) - isize::from(bb[3]);

                frontbuf[ti] = round(val_r as f32 * iarr) as u8;
                frontbuf[ti + 1] = round(val_g as f32 * iarr) as u8;
                frontbuf[ti + 2] = round(val_b as f32 * iarr) as u8;
                frontbuf[ti + 3] = round(val_a as f32 * iarr) as u8;
                ti += 4;
            }
        }
    }
}

#[inline]
/// Fast rounding for x <= 2^23.
/// This is orders of magnitude faster than built-in rounding intrinsic.
///
/// Source: https://stackoverflow.com/a/42386149/585725
fn round(mut x: f32) -> f32 {
    x += 12582912.0;
    x -= 12582912.0;
    x
}
