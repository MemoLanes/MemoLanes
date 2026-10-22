//! Intentionally depends on the complete, unmodified pre-metadata core.
use std::{fs::File, io::Cursor, path::PathBuf};

use anyhow::{ensure, Context, Result};
use memolanes_core::{
    archive::{self, MldxReader},
    journey_data::JourneyData,
    journey_header::JourneyKind,
    main_db::MainDb,
};
use tempdir::TempDir;

fn main() -> Result<()> {
    let directory = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .context("expected fixture directory")?,
    );
    for name in ["absent", "empty", "populated"] {
        let mut reader = MldxReader::open(File::open(directory.join(format!("{name}.mldx")))?)?;
        ensure!(reader.iter_journey_headers().len() == 1);
        let id = reader.iter_journey_headers()[0].id.clone();
        let (header, data, raw) = reader
            .load_single_journey_with_raw_data(&id)?
            .context("missing journey")?;
        ensure!(header.journey_kind == JourneyKind::Flight);
        ensure!(header.start.context("missing start")?.to_rfc3339() == "2026-10-22T01:00:00+00:00");
        ensure!(header.end.context("missing end")?.to_rfc3339() == "2026-10-22T03:00:00+00:00");
        ensure!(!header.has_raw_data && raw.is_none());
        let JourneyData::Vector(vector) = &data else {
            anyhow::bail!("expected vector")
        };
        ensure!(vector.track_segments.len() == 1);
        let points = &vector.track_segments[0].track_points;
        ensure!(points.len() == 2);
        ensure!(points[0].latitude == 31.2 && points[0].longitude == 121.3);
        ensure!(points[1].latitude == 40.1 && points[1].longitude == 116.6);
        let destination = TempDir::new("mldx-legacy-import")?;
        let mut db = MainDb::open(destination.path().to_str().context("non-UTF8 temp path")?)?;
        let result = db.with_txn(|txn| reader.import(txn, None))?;
        ensure!(result.imported_count == 1 && result.overwritten_count == 0);
        db.with_txn(|txn| {
            ensure!(txn.get_journey_header(&id)? == Some(header.clone()));
            ensure!(txn.get_journey_data(&id)? == data);
            ensure!(txn.get_journey_raw_data(&id)?.is_none());
            Ok(())
        })?;
        let mut exported = Cursor::new(Vec::new());
        db.with_txn(|txn| archive::export_all_journeys_as_mldx(txn, &mut exported, false))?;
        let mut reader = MldxReader::open(Cursor::new(exported.into_inner()))?;
        ensure!(reader.load_single_journey_with_raw_data(&id)? == Some((header, data, None)));
        println!("old core 9641c083: {name} metadata — read, import, database, export PASS");
    }
    Ok(())
}
