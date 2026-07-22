use std::f64::consts::PI;

use chrono::{Datelike, NaiveDate};

use crate::journey_bitmap::{
    JourneyBitmap, MAP_WIDTH, MAP_WIDTH_OFFSET, TILE_WIDTH, TILE_WIDTH_OFFSET,
};

// https://wiki.openstreetmap.org/wiki/Slippy_map_tilenames
// TODO: remove these two duplicated functions once we have the new rendering system.
pub fn lng_lat_to_tile_x_y(lng: f64, lat: f64, zoom: i32) -> (i32, i32) {
    let n = f64::powi(2.0, zoom);
    let lat_rad = (lat / 180.0) * PI;
    let x = ((lng + 180.0) / 360.0) * n;
    let y = (1.0 - ((lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI)) / 2.0 * n;
    (x.floor() as i32, y.floor() as i32)
}

pub fn tile_x_y_to_lng_lat(x: i32, y: i32, zoom: i32) -> (f64, f64) {
    let n = f64::powi(2.0, zoom);
    let lng = (x as f64 / n) * 360.0 - 180.0;
    let lat = (f64::atan(f64::sinh(PI * (1.0 - (2.0 * y as f64) / n))) * 180.0) / PI;
    (lng, lat)
}

#[derive(Debug, Copy, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapBounds {
    pub west: f64,
    pub south: f64,
    pub east: f64,
    pub north: f64,
}

/// Returns Web Mercator-aligned bounds containing every occupied block.
/// Longitude is circular: `east` may be greater than 180 degrees when that is
/// the narrow tile-level representation of a journey crossing the antimeridian.
pub fn get_bounds_from_journey_bitmap(journey_bitmap: &mut JourneyBitmap) -> Option<MapBounds> {
    let block_zoom = (TILE_WIDTH_OFFSET + MAP_WIDTH_OFFSET) as i32;
    let mut occupied_tile_columns = [false; MAP_WIDTH as usize];
    let mut tile_y_bounds: Option<(u16, u16)> = None;
    let tile_keys: Vec<_> = journey_bitmap
        .all_tile_keys()
        .filter(|key| key.x < MAP_WIDTH as u16 && key.y < MAP_WIDTH as u16)
        .collect();
    for key in &tile_keys {
        occupied_tile_columns[key.x as usize] = true;
        tile_y_bounds = Some(match tile_y_bounds {
            Some((min_y, max_y)) => (min_y.min(key.y), max_y.max(key.y)),
            None => (key.y, key.y),
        });
    }
    let (north_tile_y, south_tile_y) = tile_y_bounds?;
    let occupied_columns: Vec<usize> = occupied_tile_columns
        .iter()
        .enumerate()
        .filter_map(|(x, occupied)| occupied.then_some(x))
        .collect();

    // Remove the largest empty gap on the circular x axis. The remaining arc is
    // the narrowest interval containing every occupied tile column.
    let mut largest_gap = 0;
    let mut west_tile_x = occupied_columns[0];
    let mut east_tile_x = occupied_columns[0];
    for (index, &current) in occupied_columns.iter().enumerate() {
        let next = if index + 1 < occupied_columns.len() {
            occupied_columns[index + 1]
        } else {
            occupied_columns[0] + MAP_WIDTH as usize
        };
        let gap = next - current - 1;
        if index == 0 || gap > largest_gap {
            largest_gap = gap;
            west_tile_x = next % MAP_WIDTH as usize;
            east_tile_x = current;
        }
    }

    let edge_tile_keys: Vec<_> = tile_keys
        .into_iter()
        .filter(|key| {
            key.x as usize == west_tile_x
                || key.x as usize == east_tile_x
                || key.y == north_tile_y
                || key.y == south_tile_y
        })
        .copied()
        .collect();

    let mut west_block_x: Option<u8> = None;
    let mut east_block_x: Option<u8> = None;
    let mut north_block_y: Option<u8> = None;
    let mut south_block_y: Option<u8> = None;

    // Loading a tile may deserialize it, so inspect only tiles on the four
    // coarse edges instead of every tile in the journey.
    for tile_key in edge_tile_keys {
        let tile = journey_bitmap.get_tile(&tile_key)?;
        for (block_key, _) in tile.iter() {
            if tile_key.x as usize == west_tile_x {
                west_block_x =
                    Some(west_block_x.map_or(block_key.x(), |current| current.min(block_key.x())));
            }
            if tile_key.x as usize == east_tile_x {
                east_block_x =
                    Some(east_block_x.map_or(block_key.x(), |current| current.max(block_key.x())));
            }
            if tile_key.y == north_tile_y {
                north_block_y =
                    Some(north_block_y.map_or(block_key.y(), |current| current.min(block_key.y())));
            }
            if tile_key.y == south_tile_y {
                south_block_y =
                    Some(south_block_y.map_or(block_key.y(), |current| current.max(block_key.y())));
            }
        }
    }

    let west_x = west_tile_x * TILE_WIDTH as usize + west_block_x? as usize;
    let mut east_x = east_tile_x * TILE_WIDTH as usize + east_block_x? as usize + 1;
    if east_x <= west_x {
        east_x += (MAP_WIDTH * TILE_WIDTH) as usize;
    }
    let north_y = north_tile_y as i32 * TILE_WIDTH as i32 + north_block_y? as i32;
    let south_y = south_tile_y as i32 * TILE_WIDTH as i32 + south_block_y? as i32 + 1;

    let (west, north) = tile_x_y_to_lng_lat(west_x as i32, north_y, block_zoom);
    let (east, south) = tile_x_y_to_lng_lat(east_x as i32, south_y, block_zoom);

    Some(MapBounds {
        west,
        south,
        east,
        north,
    })
}

// We could just use num days from ce instead of epoch, but ce is quite far
// away and we use varint for serialization, so epoch can make it a bit more
// efficient.
lazy_static! {
    static ref EPOCH_NUM_OF_DAYS_FROM_CE: i32 = NaiveDate::from_ymd_opt(1970, 1, 1)
        .unwrap()
        .num_days_from_ce();
}

pub fn date_to_days_since_epoch(date: NaiveDate) -> i32 {
    date.num_days_from_ce() - *EPOCH_NUM_OF_DAYS_FROM_CE
}

pub fn date_of_days_since_epoch(days: i32) -> NaiveDate {
    NaiveDate::from_num_days_from_ce_opt(days + *EPOCH_NUM_OF_DAYS_FROM_CE)
        .expect("invalid number of days")
}

#[cfg(test)]
mod tests {
    use chrono::{FixedOffset, NaiveDate, TimeZone, Utc};

    use crate::utils::{date_of_days_since_epoch, date_to_days_since_epoch};

    #[test]
    fn days_since_epoch() {
        let check = |y, m, d, expected_days| {
            let date = NaiveDate::from_ymd_opt(y, m, d).unwrap();
            let days = date_to_days_since_epoch(date);
            assert_eq!(days, expected_days);
            assert_eq!(date, date_of_days_since_epoch(days));
        };
        check(1970, 1, 1, 0);
        check(2024, 2, 29, 19782);
        check(1938, 8, 23, -11454);
    }

    #[test]
    fn naive_date_is_local_date() {
        let utc = Utc.with_ymd_and_hms(2024, 3, 31, 23, 0, 0).unwrap();
        assert_eq!(utc.to_rfc3339(), "2024-03-31T23:00:00+00:00");
        let plus8 = utc.with_timezone(&FixedOffset::east_opt(8 * 3600).unwrap());
        assert_eq!(plus8.to_rfc3339(), "2024-04-01T07:00:00+08:00");
        assert_eq!(utc.date_naive().to_string(), "2024-03-31");
        assert_eq!(plus8.date_naive().to_string(), "2024-04-01");
    }
}

pub mod db {
    use anyhow::{Context, Result};
    use auto_context::auto_context;
    use rusqlite::{OptionalExtension, Transaction};

    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    pub struct SchemaVersion {
        pub major: i32,
        pub minor: i32,
    }

    impl SchemaVersion {
        pub const fn new(major: i32, minor: i32) -> Self {
            Self { major, minor }
        }
    }

    pub struct Migration<'a> {
        pub version: SchemaVersion,
        run: &'a dyn Fn(&Transaction) -> Result<()>,
    }

    impl<'a> Migration<'a> {
        pub const fn new(
            major: i32,
            minor: i32,
            run: &'a dyn Fn(&Transaction) -> Result<()>,
        ) -> Self {
            Self {
                version: SchemaVersion::new(major, minor),
                run,
            }
        }
    }

    fn get_version_component(tx: &Transaction, key: &str) -> Result<i32> {
        let value: Option<String> = tx
            .query_row(
                "SELECT value FROM db_metadata WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.map(|value| value.parse()).transpose()?.unwrap_or(0))
    }

    #[auto_context]
    pub fn init_metadata_and_get_version(tx: &Transaction) -> Result<SchemaVersion> {
        let create_db_metadata_sql = "
        CREATE TABLE IF NOT EXISTS `db_metadata` (
	    `key`	TEXT NOT NULL,
	    `value`	TEXT,
	    PRIMARY KEY(`key`)
        )";
        tx.execute(create_db_metadata_sql, ())?;

        // `minor_version` was introduced after the original version-1 schema.
        // Its absence therefore means 0, preserving compatibility with every
        // database created by an older release.
        let version = SchemaVersion::new(
            get_version_component(tx, "version")?,
            get_version_component(tx, "minor_version")?,
        );
        if version.major < 0 || version.minor < 0 || (version.major == 0 && version.minor != 0) {
            bail!("invalid database version: {version:?}");
        }
        Ok(version)
    }

    #[auto_context]
    pub fn set_version_in_metadata(tx: &Transaction, version: SchemaVersion) -> Result<()> {
        tx.execute(
            "INSERT OR REPLACE INTO `db_metadata` (key, value) VALUES
                ('version', ?1),
                ('minor_version', ?2)",
            (version.major.to_string(), version.minor.to_string()),
        )?;
        Ok(())
    }

    #[doc(hidden)]
    pub fn migrations_are_strictly_increasing(migrations: &[Migration<'_>]) -> bool {
        let mut previous = SchemaVersion::new(0, 0);
        for migration in migrations {
            let next = migration.version;
            if next.major <= 0 || next <= previous {
                return false;
            }
            previous = next;
        }
        true
    }

    /// Apply one ordered chain of major and backward-compatible minor migrations.
    ///
    /// The legacy `version` metadata remains the major compatibility version,
    /// so already-released applications continue to accept databases that only
    /// gained nullable columns, optional tables, or indexes. Every migration
    /// declares its resulting version, allowing dependencies such as
    /// `1.0 -> 1.3 -> 3.0 -> 3.5` to execute in their exact historical order.
    /// Versions only need to be strictly increasing; gaps are allowed.
    ///
    /// A database with a higher minor version is deliberately accepted: minor
    /// migrations must be additive and safe for older readers and writers. Its
    /// metadata is left untouched so a downgrade never lowers the schema level.
    #[auto_context]
    pub fn run_migrations(
        tx: &Transaction,
        db_name: &str,
        migrations: &[Migration<'_>],
    ) -> Result<SchemaVersion> {
        let current = init_metadata_and_get_version(tx)?;
        let target = migrations
            .last()
            .map(|migration| migration.version)
            .unwrap_or(SchemaVersion::new(0, 0));
        debug!("{db_name}: current version = {current:?}, target version = {target:?}");

        if current.major > target.major {
            bail!(
                "major version too high: current version = {current:?}, target version = {target:?}"
            );
        }

        if current.major == target.major && current.minor > target.minor {
            warn!(
                "{db_name}: database minor version {} is newer than supported {}; opening because minor versions are backward compatible",
                current.minor, target.minor
            );
            return Ok(current);
        }

        for migration in migrations
            .iter()
            .filter(|migration| migration.version > current)
        {
            info!(
                "{db_name}: running migration {}.{}",
                migration.version.major, migration.version.minor
            );
            (migration.run)(tx)?;
        }

        // Also writes an explicit minor version for legacy databases where the
        // missing key was interpreted as 0.
        set_version_in_metadata(tx, target)?;
        Ok(target)
    }
}
