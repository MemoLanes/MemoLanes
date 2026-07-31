use crate::journey_bitmap::{
    self, Block, BlockKey, JourneyBitmap, TileKey, BITMAP_SIZE, MAP_WIDTH, TILE_WIDTH,
};
use anyhow::{Context, Result};
use auto_context::auto_context;
use flate2::read::ZlibDecoder;
use std::{fs::File, io::Read, path::Path};

struct FoWTileId {
    x: u16,
    y: u16,
}

impl FoWTileId {
    const FILENAME_MASK1: &str = "olhwjsktri";

    pub fn from_filename(filename: &str) -> Option<Self> {
        if filename.len() < 6 {
            return None;
        }
        let id_part = &filename[4..(filename.len() - 2)];
        let mut id: u32 = 0;
        for c in id_part.chars() {
            let v = Self::FILENAME_MASK1.find(c)?;
            id = id * 10 + v as u32;
        }
        if id >= (MAP_WIDTH * MAP_WIDTH) as u32 {
            return None;
        }
        Some(Self {
            x: (id % MAP_WIDTH as u32) as u16,
            y: (id / MAP_WIDTH as u32) as u16,
        })
    }
}

#[auto_context]
fn parse_fow_bitmap_file<R: Read>(
    file: R,
    filename: &str,
    journey_bitmap: &mut JourneyBitmap,
    warnings: &mut Vec<String>,
) -> Result<()> {
    const TILE_HEADER_LEN: i64 = TILE_WIDTH * TILE_WIDTH;
    const TILE_HEADER_SIZE: usize = (TILE_HEADER_LEN * 2) as usize;
    const BLOCK_BITMAP_SIZE: usize = BITMAP_SIZE;
    const BLOCK_EXTRA_DATA: usize = 3;
    const BLOCK_SIZE: usize = BLOCK_BITMAP_SIZE + BLOCK_EXTRA_DATA;

    match FoWTileId::from_filename(filename) {
        None => warnings.push(format!("unexpected file: {filename}")),
        Some(id) => {
            let mut tile = journey_bitmap::Tile::new();
            let mut data = Vec::new();
            ZlibDecoder::new(file).read_to_end(&mut data)?;

            let header = &data[0..TILE_HEADER_SIZE];
            for i in 0..TILE_HEADER_LEN {
                // parse two u8 as a single u16 according to little endian
                let index = (i as usize) * 2;
                let block_idx: u16 = (header[index] as u16) | ((header[index + 1] as u16) << 8);
                if block_idx > 0 {
                    let block_key =
                        BlockKey::from_x_y((i % TILE_WIDTH) as u8, (i / TILE_WIDTH) as u8);
                    let start_offset = TILE_HEADER_SIZE + ((block_idx - 1) as usize) * BLOCK_SIZE;
                    let end_offset = start_offset + BLOCK_BITMAP_SIZE;
                    let mut bitmap: [u8; BLOCK_BITMAP_SIZE] = [0; BLOCK_BITMAP_SIZE];
                    bitmap.copy_from_slice(&data[start_offset..end_offset]);
                    let block = Block::new_with_data(bitmap);
                    if !block.is_empty() {
                        tile.set(&block_key, block);
                    }
                }
            }
            if !tile.is_empty() {
                journey_bitmap.insert_tile(&TileKey::new(id.x, id.y), tile);
            }
        }
    }
    Ok(())
}

#[auto_context]
pub fn load_fow_sync_data(mldx_file_path: &str) -> Result<(JourneyBitmap, Option<String>)> {
    let mut warnings: Vec<String> = Vec::new();

    let mut zip = zip::ZipArchive::new(File::open(mldx_file_path)?)?;
    let has_sync_folder = zip
        .file_names()
        .any(|name| name.to_lowercase().contains("sync/"));

    let mut journey_bitmap = JourneyBitmap::new();
    for i in 0..zip.len() {
        let file = zip.by_index(i)?;
        let filename = file.name().to_lowercase();
        // the check below are just best effort.
        // if there is a sync folder, skip all other files
        if has_sync_folder && !filename.contains("sync/") {
            continue;
        }
        let filename = Path::file_name(Path::new(&filename))
            .and_then(|x| x.to_str())
            .unwrap_or("");
        if filename.is_empty() || filename.starts_with('.') || file.is_dir() {
            continue;
        }
        parse_fow_bitmap_file(file, filename, &mut journey_bitmap, &mut warnings)?;
    }

    let warnings = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("\n"))
    };

    if journey_bitmap.is_empty() {
        Err(anyhow!(
            "empty data. warnings: {}",
            warnings.unwrap_or_default()
        ))
    } else {
        Ok((journey_bitmap, warnings))
    }
}

#[auto_context]
pub fn load_fow_snapshot_data(fwss_file_path: &str) -> Result<(JourneyBitmap, Option<String>)> {
    let mut warnings: Vec<String> = Vec::new();
    let mut zip = zip::ZipArchive::new(File::open(fwss_file_path)?)?;

    let mut journey_bitmap = JourneyBitmap::new();
    for i in 0..zip.len() {
        let file = zip.by_index(i)?;
        let fwss_subfilename = file.name().to_lowercase();

        if !fwss_subfilename.contains("model/*/") {
            continue;
        }

        let filename = Path::file_name(Path::new(&fwss_subfilename))
            .and_then(|x| x.to_str())
            .unwrap_or("");
        if filename.is_empty() || filename.starts_with('.') || file.is_dir() {
            continue;
        }
        parse_fow_bitmap_file(file, filename, &mut journey_bitmap, &mut warnings)?;
    }

    let warnings = if warnings.is_empty() {
        None
    } else {
        Some(warnings.join("\n"))
    };

    if journey_bitmap.is_empty() {
        Err(anyhow!(
            "empty data. warnings: {}",
            warnings.unwrap_or_default()
        ))
    } else {
        Ok((journey_bitmap, warnings))
    }
}
