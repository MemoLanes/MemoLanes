use chrono::NaiveDate;
use memolanes_core::{
    api::{api, edit_session::EditSession},
    journey_data::JourneyData,
    journey_header::JourneyKind,
    journey_vector::{JourneyVector, TrackPoint, TrackSegment},
    main_db::{MainDb, NewJourney},
};
use tempdir::TempDir;

#[test]
fn box_deletion_follows_the_visible_great_circle_and_supports_undo() {
    let dir = TempDir::new("edit_session").unwrap();
    let sub = |name| {
        let path = dir.path().join(name);
        std::fs::create_dir(&path).unwrap();
        path.to_str().unwrap().to_owned()
    };
    let support = sub("support");
    let mut main = MainDb::open(&support).unwrap();
    let mut cases = Vec::new();
    for latitude in [60.0_f64, -60.0] {
        for longitudes in [[0.0, 1.0], [1.0, 0.0]] {
            let original = JourneyVector {
                track_segments: vec![TrackSegment {
                    track_points: longitudes
                        .map(|longitude| TrackPoint {
                            latitude,
                            longitude,
                        })
                        .to_vec(),
                }],
            };
            let id = main
                .with_txn(|txn| {
                    txn.create_and_insert_journey(NewJourney {
                        journey_date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                        start: None,
                        end: None,
                        created_at: None,
                        journey_kind: JourneyKind::DefaultKind,
                        note: None,
                        journey_data: JourneyData::Vector(original.clone()),
                        raw_data: None,
                    })
                })
                .unwrap();
            cases.push((id, original, latitude));
        }
    }
    api::init(sub("temp"), sub("doc"), support, sub("cache")).unwrap();

    for (id, original, latitude) in cases {
        let mut session = EditSession::new(id.clone()).unwrap().unwrap();
        // This box intersects the old longitude/latitude chord, but is about
        // 105 metres away from the displayed great-circle midpoint.
        session
            .delete_points_in_box(latitude - 0.0001, 0.4998, latitude + 0.0001, 0.5002)
            .unwrap();
        assert!(
            !session.can_undo(),
            "deleting empty map space must be a no-op"
        );

        // Equal-latitude endpoints have midpoint tan(lat) / cos(delta_lng / 2).
        let midpoint_lat = (latitude.to_radians().tan() / 0.5_f64.to_radians().cos())
            .atan()
            .to_degrees();
        let delete_midpoint = |session: &mut EditSession| {
            session
                .delete_points_in_box(midpoint_lat - 0.0001, 0.4998, midpoint_lat + 0.0001, 0.5002)
                .unwrap();
        };
        delete_midpoint(&mut session);
        assert!(session.can_undo());
        session.undo().unwrap();
        assert!(!session.can_undo());
        session.commit().unwrap();
        assert_eq!(
            main.with_txn(|txn| txn.get_journey_data(&id)).unwrap(),
            JourneyData::Vector(original.clone())
        );

        let mut session = EditSession::new(id.clone()).unwrap().unwrap();
        delete_midpoint(&mut session);
        session.commit().unwrap();
        let JourneyData::Vector(edited) = main.with_txn(|txn| txn.get_journey_data(&id)).unwrap()
        else {
            panic!("expected a vector journey");
        };
        assert_eq!(edited.track_segments.len(), 2);
        let first = &edited.track_segments[0].track_points;
        let second = &edited.track_segments[1].track_points;
        assert_eq!(
            first.first(),
            original.track_segments[0].track_points.first()
        );
        assert_eq!(
            second.last(),
            original.track_segments[0].track_points.last()
        );
        let boundaries = if first[0].longitude < second.last().unwrap().longitude {
            [0.4998, 0.5002]
        } else {
            [0.5002, 0.4998]
        };
        for (point, longitude) in [first.last().unwrap(), &second[0]]
            .into_iter()
            .zip(boundaries)
        {
            assert!((point.longitude - longitude).abs() < 1e-9);
            assert!((point.latitude - midpoint_lat).abs() < 1e-8);
        }
    }
}
