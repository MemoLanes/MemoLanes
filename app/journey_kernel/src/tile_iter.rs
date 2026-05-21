use crate::utils::xy_to_index;
use bitvec::prelude::*;

/// IndexIter helps index pixels within a tile with a specific width_exp.
pub struct IndexIter {
    x_min: i64,
    x_max: i64,
    y_min: i64,
    y_max: i64,
    current_x: i64,
    current_y: i64,
}

impl Iterator for IndexIter {
    type Item = (i64, i64);

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_y >= self.y_max {
            return None;
        }

        let index = (self.current_x, self.current_y);

        self.current_x += 1;
        if self.current_x >= self.x_max {
            self.current_x = self.x_min;
            self.current_y += 1;
        }

        Some(index)
    }
}

impl IndexIter {
    /// the interest region of a tile
    pub fn new(x: i64, y: i64, resolution_exp: i16) -> Self {
        let x_min = x << resolution_exp;
        let x_max = (x + 1) << resolution_exp;
        let y_min = y << resolution_exp;
        let y_max = (y + 1) << resolution_exp;

        Self {
            x_min,
            x_max,
            y_min,
            y_max,
            current_x: x_min,
            current_y: y << resolution_exp,
        }
    }

    pub fn get_min_xy(&self) -> (i64, i64) {
        (self.x_min, self.y_min)
    }
}

pub struct MipmapIter<'a> {
    bitmap: &'a BitVec,
    index_iter: IndexIter,
    width_exp: i16,
    start_x: i64,
    start_y: i64,
    x_offset: i64,
    y_offset: i64,
}

impl<'a> Iterator for MipmapIter<'a> {
    type Item = (i64, i64);

    fn next(&mut self) -> Option<Self::Item> {
        for (x, y) in self.index_iter.by_ref() {
            if self.bitmap[xy_to_index(x, y, self.width_exp)] {
                return Some((
                    self.start_x + x - self.x_offset,
                    self.start_y + y - self.y_offset,
                ));
            }
        }
        None
    }
}

impl<'a> MipmapIter<'a> {
    pub fn new(
        bitmap: &'a BitVec,
        start_x: i64,
        start_y: i64,
        x: i64,
        y: i64,
        z: i16,
        width_exp: i16,
    ) -> Self {
        let index_iter = IndexIter::new(x, y, width_exp - z);
        let (x_offset, y_offset) = index_iter.get_min_xy();
        Self {
            bitmap,
            index_iter,
            width_exp,
            start_x,
            start_y,
            x_offset,
            y_offset,
        }
    }
}

pub struct OverscanIter<'a> {
    bitmap: &'a BitVec,
    index_iter: IndexIter,
    sub_tile_index_iter: Option<IndexIter>,
    width_exp: i16,
    start_x: i64,
    start_y: i64,
    subtile_resolution_exp: i16,
}

impl<'a> Iterator for OverscanIter<'a> {
    type Item = (i64, i64);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.sub_tile_index_iter.is_none() {
                while let Some((x, y)) = self.index_iter.next() {
                    if self.bitmap[xy_to_index(x, y, self.width_exp)] {
                        let (x_min, y_min) = self.index_iter.get_min_xy();
                        self.sub_tile_index_iter = Some(IndexIter::new(
                            x - x_min,
                            y - y_min,
                            self.subtile_resolution_exp,
                        ));
                        break;
                    }
                }

                self.sub_tile_index_iter.as_ref()?;
            }

            if let Some((x, y)) = self.sub_tile_index_iter.as_mut().unwrap().next() {
                return Some((self.start_x + x, self.start_y + y));
            } else {
                self.sub_tile_index_iter = None;
            }
        }
    }
}

impl<'a> OverscanIter<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bitmap: &'a BitVec,
        start_x: i64,
        start_y: i64,
        x: i64,
        y: i64,
        z: i16,
        width_exp: i16,
        subtile_resolution_exp: i16,
    ) -> Self {
        Self {
            bitmap,
            index_iter: IndexIter::new(x, y, width_exp - z),
            sub_tile_index_iter: None,
            width_exp,
            start_x,
            start_y,
            subtile_resolution_exp,
        }
    }
}

/// Pixel iterator for dense `BitMap2D` tiles (no sparse tree traversal).
pub enum BitmapPixelIter<'a> {
    MipmapIter(MipmapIter<'a>),
    OverscanIter(OverscanIter<'a>),
    Empty,
}

impl<'a> Iterator for BitmapPixelIter<'a> {
    type Item = (i64, i64);

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            BitmapPixelIter::MipmapIter(iter) => iter.next(),
            BitmapPixelIter::OverscanIter(iter) => iter.next(),
            BitmapPixelIter::Empty => None,
        }
    }
}
