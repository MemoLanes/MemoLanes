use std::io::{Read, Write};

use anyhow::{Result, Ok};
use integer_encoding::*;
use itertools::Itertools;

use crate::{
    journey_bitmap::{self, JourneyBitmap, Tile, Block},
    journey_header::JourneyType,
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
};

pub enum JourneyData {
    Vector(JourneyVector),
    Bitmap(JourneyBitmap),
}

// TODO: maybe we want a higher one for archive
// 3 is the zstd default
pub const ZSTD_COMPRESS_LEVEL: i32 = 3;

const JOURNEY_VECTOR_MAGIC_HEADER: [u8; 2] = [b'V', b'0'];
const JOURNEY_BITMAP_MAGIC_HEADER: [u8; 2] = [b'B', b'0'];

// TODO: I don't have a strong reason on putting all serializations here
pub fn serialize_journey_vector<T: Write>(
    journey_vector: &JourneyVector,
    mut writer: T,
) -> Result<()> {
    // magic header
    writer.write_all(&JOURNEY_VECTOR_MAGIC_HEADER)?;

    // data is compressed as a whole
    let mut encoder = zstd::Encoder::new(writer, ZSTD_COMPRESS_LEVEL)?.auto_finish();
    encoder.write_all(&(journey_vector.track_segments.len() as u64).encode_var_vec())?;
    for track_segmant in &journey_vector.track_segments {
        encoder.write_all(&(track_segmant.track_points.len() as u64).encode_var_vec())?;
        for track_point in &track_segmant.track_points {
            encoder.write_all(&track_point.latitude.to_be_bytes())?;
            encoder.write_all(&track_point.longitude.to_be_bytes())?;
        }
    }
    Ok(())
}

pub fn deserialize_journey_vector<T: Read>(mut reader: T) -> Result<JourneyVector> {
    // magic header
    let mut magic_header: [u8; 2] = [0; 2];
    reader.read_exact(&mut magic_header)?;
    if magic_header != JOURNEY_VECTOR_MAGIC_HEADER {
        bail!(
            "Invalid magic header, expect: {:?}, got: {:?}",
            &JOURNEY_VECTOR_MAGIC_HEADER,
            &magic_header
        );
    }

    // data is compressed as a whole
    let mut decoder = zstd::Decoder::new(reader)?;
    let segments_count: u64 = decoder.read_varint()?;
    let mut track_segments = Vec::with_capacity(segments_count as usize);
    for _ in 0..segments_count {
        let points_count: u64 = decoder.read_varint()?;
        let mut track_points = Vec::with_capacity(points_count as usize);
        for _ in 0..points_count {
            let mut buf: [u8; 8] = [0; 8];
            decoder.read_exact(&mut buf)?;
            let latitude = f64::from_be_bytes(buf);
            decoder.read_exact(&mut buf)?;
            let longitude = f64::from_be_bytes(buf);
            track_points.push(TrackPoint {
                latitude,
                longitude,
            })
        }
        track_segments.push(TrackSegment { track_points });
    }
    Ok(JourneyVector { track_segments })
}

fn block_key_to_index(x: u8, y: u8) -> u16 {
    assert!(x < journey_bitmap::TILE_WIDTH as u8);
    assert!(y < journey_bitmap::TILE_WIDTH as u8);
    x as u16 * journey_bitmap::TILE_WIDTH as u16 + y as u16
}

fn block_index_to_key(i: u16) -> (u8, u8) {
    let x = (i / journey_bitmap::TILE_WIDTH as u16) as u8;
    let y = (i % journey_bitmap::TILE_WIDTH as u16) as u8;
    (x, y)
}

#[cfg(test)]
mod tests {
    use crate::journey_data::*;

    #[test]
    fn block_key_conversion() {
        assert_eq!(block_key_to_index(0, 0), (0));
        assert_eq!(block_index_to_key(0), (0, 0));

        assert_eq!(block_key_to_index(127, 127), (16383));
        assert_eq!(block_index_to_key(16383), (127, 127));

        assert_eq!(block_key_to_index(64, 17), (8209));
        assert_eq!(block_index_to_key(8209), (64, 17));
    }
}

pub fn serialize_journey_bitmap<T: Write>(
    journey_bitmap: &JourneyBitmap,
    mut writer: T,
) -> Result<()> {
    let serialize_tile = |tile: &Tile| -> Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut encoder = zstd::Encoder::new(&mut buf, ZSTD_COMPRESS_LEVEL)?.auto_finish();
        // We put all block ids in front so we could get all blocks without
        // deserializing the whole block.
        // A bitmap seems more efficient for this case.
        let mut block_keys =
            [0_u8; (journey_bitmap::TILE_WIDTH * journey_bitmap::TILE_WIDTH / 8) as usize];
        for (x, y) in tile.blocks.keys() {
            let i = block_key_to_index(*x, *y);
            let byte_index = (i / 8) as usize;
            block_keys[byte_index] |= 1 << (i % 8);
        }
        encoder.write_all(&block_keys)?; 

        let mut n=0;
        // write each block in the right order
        for byte_index in 0..block_keys.len() {
            for offset in 0..8 {                
                if block_keys[byte_index] & (1 << offset) == 1 {
                    let (x, y) = block_index_to_key((byte_index * 8 + offset) as u16);                                                            
                    match tile.blocks.get(&(x, y)) {
                        Some(block)=>{
                            println!("block write success,x={},y={}",x,y);
                            n+=1;
                            encoder.write_all(&block.data)?;                            
                        },
                        None=>{
                            println!("failed x={},y={}",x,y);
                        }
                    }
                    // let block = tile.blocks.get(&(x, y)).unwrap();
                    // encoder.write_all(&block.data)?;
                }            
            }
        }
        println!("n={}",n);

        drop(encoder);
        Ok(buf)
    };

    // magic header
    writer.write_all(&JOURNEY_BITMAP_MAGIC_HEADER)?;

    writer.write_all(&(journey_bitmap.tiles.len() as u64).encode_var_vec())?;
    for (x, y) in journey_bitmap.tiles.keys().sorted() {
        let tile = journey_bitmap.tiles.get(&(*x, *y)).unwrap();
        let serialized_tile = serialize_tile(tile)?;
        writer.write_all(&x.to_be_bytes())?;
        writer.write_all(&y.to_be_bytes())?;
        // Also write down the size of the tile so we could load the bitmap
        // without eagerly deserialize all tiles.
        writer.write_all(&(serialized_tile.len() as u64).encode_var_vec())?;
        writer.write_all(&serialized_tile)?;
    }

    Ok(())
}

pub fn deserialize_journey_bitmap<T:Read>(mut reader: T)->Result<JourneyBitmap>{
    let mut magic_header: [u8; 2] = [0; 2];
    reader.read_exact(&mut magic_header)?;
    if magic_header != JOURNEY_BITMAP_MAGIC_HEADER {
        bail!(
            "Invalid magic header, expect: {:?}, got: {:?}",
            &JOURNEY_BITMAP_MAGIC_HEADER,
            &magic_header
        );
    }

    let mut journey_bitmap=JourneyBitmap::new();      
    let tiles_count:u64 = reader.read_varint()?;
    for _ in 0..tiles_count {
        let mut buf: [u8; 2] = [0; 2];
        reader.read_exact(&mut buf)?;
        let x_tile=u16::from_be_bytes(buf);

        reader.read_exact(&mut buf)?;
        let y_tile=u16::from_be_bytes(buf);

        let tile_data_len:u64 = reader.read_varint()?;        
        let mut buf_tile=vec![0_u8;tile_data_len as usize];
        reader.read_exact(&mut buf_tile)?;               
        let tile = deserialize_tile(buf_tile.as_slice())?;
        journey_bitmap.tiles.insert((x_tile,y_tile), tile);
    }

    Ok(journey_bitmap)
}

fn deserialize_tile<T:Read>(reader:T)->Result<Tile>{
    let mut decoder = zstd::Decoder::new(reader)?;
    let mut tile=Tile::new();
    let mut block_keys =[0_u8; (journey_bitmap::TILE_WIDTH * journey_bitmap::TILE_WIDTH / 8) as usize];
    decoder.read_exact(&mut block_keys)?;

    for byte_index in 0..block_keys.len() {
        for offset in 0..8 {
            if block_keys[byte_index] & (1 << offset) == 1 {
                let (x, y) = block_index_to_key((byte_index * 8 + offset) as u16);                
                let mut block_data = [0_u8;journey_bitmap::BITMAP_SIZE];                
                decoder.read_exact(&mut block_data)?;
                let block = Block::new_with_data(block_data);
                tile.blocks.insert((x,y), block);
            }
        }
    }

    Ok(tile)
}

impl JourneyData {
    pub fn serialize<T: Write>(&self, writer: T) -> Result<()> {
        match self {
            JourneyData::Vector(vector) => serialize_journey_vector(vector, writer)?,
            JourneyData::Bitmap(bitmap) => serialize_journey_bitmap(bitmap, writer)?,
        };
        Ok(())
    }

    pub fn deserialize<T: Read>(reader: T, journey_type: JourneyType) -> Result<JourneyData> {
        match journey_type {
            JourneyType::Vector => Ok(JourneyData::Vector(deserialize_journey_vector(reader)?)),
            JourneyType::Bitmap => Ok(JourneyData::Bitmap(deserialize_journey_bitmap(reader)?))
        }
    }
}