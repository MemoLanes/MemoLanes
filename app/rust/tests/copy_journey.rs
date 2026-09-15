use chrono::{DateTime, NaiveDate};
use memolanes_core::{
    journey_bitmap::JourneyBitmap,
    journey_data::JourneyData,
    journey_header::{JourneyHeader, JourneyKind},
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
    main_db::{Action, CacheEntry, MainDb},
    raw_data::JourneyRawData,
};
use rusqlite::Connection;
use tempdir::TempDir;

#[test]
fn copies_both_formats_verbatim_and_keeps_records_independent() {
    let mut bitmap = JourneyBitmap::new();
    bitmap.add_line(121.4, 31.2, 121.401, 31.201);
    let vector = JourneyVector {
        track_segments: vec![TrackSegment {
            track_points: vec![
                TrackPoint {
                    latitude: 31.2,
                    longitude: 121.4,
                },
                TrackPoint {
                    latitude: 31.201,
                    longitude: 121.401,
                },
            ],
        }],
    };
    for data in [JourneyData::Vector(vector), JourneyData::Bitmap(bitmap)] {
        let dir = TempDir::new("copy-journey").unwrap();
        let mut db = MainDb::open(dir.path().to_str().unwrap()).unwrap();
        let time = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let original = JourneyHeader {
            id: "original".into(),
            revision: "original-revision".into(),
            journey_date: NaiveDate::from_ymd_opt(2023, 11, 14).unwrap(),
            created_at: time,
            updated_at: Some(time),
            start: Some(time),
            end: Some(time + chrono::Duration::hours(1)),
            journey_type: data.type_(),
            journey_kind: JourneyKind::Flight,
            note: Some("保留原始备注".into()),
            postprocessor_algo: Some("original-algorithm".into()),
            has_raw_data: true,
        };
        let raw_data = JourneyRawData::new(Vec::new(), time.timestamp_millis())
            .serialize()
            .unwrap();
        db.with_txn(|txn| {
            txn.insert_journey_with_raw_data(original.clone(), data.clone(), Some(raw_data.clone()))
        })
        .unwrap();
        let copy_id = db
            .with_txn(|txn| {
                let id = txn.copy_journey(&original.id)?;
                assert_eq!(
                    txn.action,
                    Some(Action::Invalidate {
                        entries: vec![CacheEntry {
                            date: original.journey_date,
                            kind: original.journey_kind
                        }],
                    })
                );
                Ok(id)
            })
            .unwrap();
        assert_ne!(copy_id, original.id);
        let mut copied = db
            .with_txn(|txn| txn.get_journey_header(&copy_id))
            .unwrap()
            .unwrap();
        assert_ne!(copied.revision, original.revision);
        copied.id = original.id.clone();
        copied.revision = original.revision.clone();
        assert_eq!(copied, original);
        assert_eq!(
            db.with_txn(|txn| txn.get_journey_data(&copy_id)).unwrap(),
            data
        );
        assert_eq!(
            db.with_txn(|txn| txn.get_journey_raw_data(&copy_id))
                .unwrap(),
            Some(raw_data)
        );

        // Check the actual stored bytes and query indexes, not just decoded tracks.
        let conn = Connection::open(dir.path().join("main.db")).unwrap();
        let equal: bool = conn
            .query_row(
                "SELECT a.data = b.data AND a.raw_data = b.raw_data
             AND a.journey_date = b.journey_date
             AND a.timestamp_for_ordering = b.timestamp_for_ordering
             AND a.type = b.type AND a.journey_kind = b.journey_kind
             FROM journey a JOIN journey b ON b.id = ?2 WHERE a.id = ?1",
                (&original.id, &copy_id),
                |row| row.get(0),
            )
            .unwrap();
        assert!(equal);
        drop(conn);

        db.with_txn(|txn| {
            txn.update_journey_metadata(
                &copy_id,
                original.journey_date.succ_opt().unwrap(),
                None,
                None,
                Some("only the copy changes".into()),
                JourneyKind::DefaultKind,
            )
        })
        .unwrap();
        assert_eq!(
            db.with_txn(|txn| txn.get_journey_header(&original.id))
                .unwrap(),
            Some(original.clone())
        );
        db.with_txn(|txn| txn.delete_journey(&copy_id)).unwrap();
        assert_eq!(
            db.with_txn(|txn| txn.query_journeys(None, None, None))
                .unwrap(),
            vec![original]
        );
    }
}

#[test]
fn missing_source_does_not_create_a_journey() {
    let dir = TempDir::new("copy-missing-journey").unwrap();
    let mut db = MainDb::open(dir.path().to_str().unwrap()).unwrap();
    assert!(db.with_txn(|txn| txn.copy_journey("missing")).is_err());
    assert!(db
        .with_txn(|txn| txn.query_journeys(None, None, None))
        .unwrap()
        .is_empty());
}

#[test]
fn repeated_copies_preserve_optional_metadata_and_have_distinct_ids() {
    let dir = TempDir::new("copy-journey-optional-metadata").unwrap();
    let mut db = MainDb::open(dir.path().to_str().unwrap()).unwrap();
    let original_id = db
        .with_txn(|txn| {
            txn.create_and_insert_journey(
                NaiveDate::from_ymd_opt(2023, 11, 14).unwrap(),
                None,
                None,
                None,
                JourneyKind::DefaultKind,
                None,
                JourneyData::Bitmap(JourneyBitmap::new()),
            )
        })
        .unwrap();
    let original = db
        .with_txn(|txn| txn.get_journey_header(&original_id))
        .unwrap()
        .unwrap();
    let copy_ids = db
        .with_txn(|txn| {
            Ok([
                txn.copy_journey(&original_id)?,
                txn.copy_journey(&original_id)?,
            ])
        })
        .unwrap();
    assert_ne!(copy_ids[0], copy_ids[1]);
    for id in copy_ids {
        assert_ne!(id, original_id);
        let mut copied = db
            .with_txn(|txn| txn.get_journey_header(&id))
            .unwrap()
            .unwrap();
        copied.id = original.id.clone();
        copied.revision = original.revision.clone();
        assert_eq!(copied, original);
    }
    assert_eq!(
        db.with_txn(|txn| txn.query_journeys(None, None, None))
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn copy_is_rolled_back_when_transaction_fails() {
    let dir = TempDir::new("copy-journey-rollback").unwrap();
    let mut db = MainDb::open(dir.path().to_str().unwrap()).unwrap();
    let original_id = db
        .with_txn(|txn| {
            txn.create_and_insert_journey(
                NaiveDate::from_ymd_opt(2023, 11, 14).unwrap(),
                None,
                None,
                None,
                JourneyKind::DefaultKind,
                None,
                JourneyData::Bitmap(JourneyBitmap::new()),
            )
        })
        .unwrap();
    let result: anyhow::Result<()> = db.with_txn(|txn| {
        txn.copy_journey(&original_id)?;
        anyhow::bail!("abort transaction");
    });
    assert!(result.is_err());
    let journeys = db
        .with_txn(|txn| txn.query_journeys(None, None, None))
        .unwrap();
    assert_eq!(journeys.len(), 1);
    assert_eq!(journeys[0].id, original_id);
}
