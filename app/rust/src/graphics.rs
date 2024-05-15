use tiny_skia::PremultipliedColorU8;

// tracks dilation (morphology) according to l-infinity norm (GPT4 generated codes)
pub fn color_dilation(
    data: &mut [PremultipliedColorU8],
    width: usize,
    height: usize,
    color: PremultipliedColorU8,
    radius: usize,
) {
    // Check input constraints
    assert_eq!(data.len(), width * height);

    // Create a buffer to hold the indices of pixels that match the target color
    let mut matches = vec![];

    // Find all pixels that exactly match the target color
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            if data[index] == color {
                matches.push((x, y));
            }
        }
    }

    // Apply dilation to all matched pixels
    for &(x, y) in &matches {
        // Define the bounds for dilation
        let min_x = if x >= radius { x - radius } else { 0 };
        let max_x = if x + radius < width {
            x + radius
        } else {
            width - 1
        };
        let min_y = if y >= radius { y - radius } else { 0 };
        let max_y = if y + radius < height {
            y + radius
        } else {
            height - 1
        };

        // Set the color for these bounds
        for i in min_x..=max_x {
            for j in min_y..=max_y {
                let index = j * width + i;
                data[index] = color;
            }
        }
    }
}

// tracks dilation (morphology) according to l-2 norm and radius=1
pub fn color_dilation2(
    data: &mut [PremultipliedColorU8],
    width: usize,
    height: usize,
    color: PremultipliedColorU8,
) {
    // Check input constraints
    assert_eq!(data.len(), width * height);
    assert_ne!(width, 0);
    assert_ne!(height, 0);

    // Create a buffer to hold the indices of pixels that match the target color
    let mut matches = vec![];

    // Find all pixels that exactly match the target color
    for y in 0..height {
        for x in 0..width {
            let index = y * width + x;
            if data[index] == color {
                matches.push((x, y));
            }
        }
    }

    // Apply dilation to all matched pixels
    for &(x, y) in &matches {
        // self
        let mut i = x;
        let mut j = y;
        let index = j * width + i;
        data[index] = color;

        // up
        if y < height - 1 {
            j = y + 1;
            let index = j * width + i;
            data[index] = color;
        }

        // down
        if y > 0 {
            j = y - 1;
            let index = j * width + i;
            data[index] = color;
        }

        // left
        j = y;
        if x > 0 {
            i = x - 1;
            let index = j * width + i;
            data[index] = color;
        }

        // right
        if x < width - 1 {
            i = x + 1;
            let index = j * width + i;
            data[index] = color;
        }
    }
}
