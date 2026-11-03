use std::io::Cursor;

use memolanes_core::{
    archive::{self, MldxReader},
    journey_attributes::{JourneyAttributes, JourneyAttributesView},
    journey_data::JourneyData,
    journey_header::{JourneyHeader, JourneyKind, JourneyType},
    journey_vector::JourneyVector,
    main_db::MainDb,
};
use protobuf::{Message, UnknownFields};
use tempdir::TempDir;

fn flight() -> JourneyAttributes {
    JourneyAttributes::flight(
        Some("MU5101".into()),
        Some("CES".into()),
        Some("A359".into()),
        Some("B-304D".into()),
    )
}

fn train() -> JourneyAttributes {
    JourneyAttributes::train(Some("のぞみ12号".into()), Some("JR東海".into()))
}

fn journey(attributes: Option<JourneyAttributes>) -> JourneyHeader {
    JourneyHeader {
        id: "flight".into(),
        revision: "original".into(),
        journey_date: "2026-10-22".parse().unwrap(),
        created_at: "2026-10-22T01:00:00Z".parse().unwrap(),
        updated_at: None,
        start: None,
        end: None,
        journey_type: JourneyType::Vector,
        journey_kind: JourneyKind::Flight,
        note: None,
        postprocessor_algo: None,
        has_raw_data: false,
        attributes,
    }
}

fn encode(header: JourneyHeader) -> Cursor<Vec<u8>> {
    let mut output = Cursor::new(Vec::new());
    let data = JourneyData::Vector(JourneyVector {
        track_segments: vec![],
    });
    archive::export_single_journey_as_mldx(header, data, None, &mut output, false).unwrap();
    output.set_position(0);
    output
}

fn add_future_fields(fields: &mut UnknownFields) {
    fields.add_varint(100, 42);
    fields.add_varint(100, 99);
    fields.add_fixed32(101, 123);
    fields.add_fixed64(102, 456);
    fields.add_length_delimited(103, "Future info ✈️\nSecond line".as_bytes().to_vec());
    fields.add_length_delimited(103, Vec::new());
}

#[test]
fn typed_attributes_exposes_optional_display_fields() {
    assert_eq!(
        flight().details(),
        Some(JourneyAttributesView::Flight {
            flight_number: Some("MU5101".into()),
            operating_airline_icao: Some("CES".into()),
            aircraft_type_icao: Some("A359".into()),
            registration: Some("B-304D".into()),
        })
    );
    assert_eq!(
        JourneyAttributes::flight(None, Some(String::new()), None, None).details(),
        Some(JourneyAttributesView::Flight {
            flight_number: None,
            operating_airline_icao: Some(String::new()),
            aircraft_type_icao: None,
            registration: None,
        })
    );
    assert_eq!(JourneyAttributes::default().details(), None);
    assert_eq!(
        train().details(),
        Some(JourneyAttributesView::Train {
            service_name: Some("のぞみ12号".into()),
            operator_name: Some("JR東海".into()),
        })
    );
    assert_eq!(
        JourneyAttributes::train(None, Some(String::new())).details(),
        Some(JourneyAttributesView::Train {
            service_name: None,
            operator_name: Some(String::new()),
        })
    );
}

#[test]
fn optional_attributes_survives_mldx_roundtrip() {
    for attributes in [
        None,
        Some(JourneyAttributes::default()),
        Some(flight()),
        Some(JourneyAttributes::flight(
            None,
            Some(String::new()),
            None,
            None,
        )),
        Some(train()),
        Some(JourneyAttributes::train(None, None)),
        Some(JourneyAttributes::train(None, Some(String::new()))),
        Some(JourneyAttributes::train(Some(String::new()), None)),
    ] {
        let is_train = matches!(
            attributes.as_ref().and_then(JourneyAttributes::details),
            Some(JourneyAttributesView::Train { .. })
        );
        let mut header = journey(attributes);
        if is_train {
            header.journey_kind = JourneyKind::DefaultKind;
        }
        let mut reader = MldxReader::open(encode(header.clone())).unwrap();
        let (loaded, _, _) = reader
            .load_single_journey_with_raw_data(&header.id)
            .unwrap()
            .unwrap();
        assert_eq!(loaded, header);
    }
}

#[test]
fn attributes_unknown_fields_and_categories_survive_import_edit_export_and_reimport() {
    for (attributes, kind) in [
        (flight(), JourneyKind::Flight),
        (train(), JourneyKind::DefaultKind),
        (JourneyAttributes::default(), JourneyKind::DefaultKind),
    ] {
        let mut header = journey(Some(attributes));
        header.journey_kind = kind;
        let mut expected = header.to_proto();
        let attributes = expected.attributes.as_mut().unwrap();
        add_future_fields(attributes.special_fields.mut_unknown_fields());
        if attributes.has_flight() {
            add_future_fields(attributes.mut_flight().special_fields.mut_unknown_fields());
        } else if attributes.has_train() {
            add_future_fields(attributes.mut_train().special_fields.mut_unknown_fields());
        } else {
            // Future oneof variant: CruiseAttributes { string ship_name = 1; }.
            attributes
                .special_fields
                .mut_unknown_fields()
                .add_length_delimited(20, b"\x0a\x04Ship".to_vec());
        }
        let bytes = expected.write_to_bytes().unwrap();
        let header = JourneyHeader::of_proto(Message::parse_from_bytes(&bytes).unwrap()).unwrap();
        if !expected.attributes.as_ref().unwrap().has_flight()
            && !expected.attributes.as_ref().unwrap().has_train()
        {
            assert_eq!(header.attributes.as_ref().unwrap().details(), None);
        }

        let directory = TempDir::new("attributes-roundtrip").unwrap();
        {
            let mut db = MainDb::open(directory.path().to_str().unwrap()).unwrap();
            let mut reader = MldxReader::open(encode(header.clone())).unwrap();
            db.with_txn(|txn| reader.import(txn, None)).unwrap();
        }
        let mut db = MainDb::open(directory.path().to_str().unwrap()).unwrap();
        db.with_txn(|txn| {
            txn.update_journey_metadata(
                &header.id,
                header.journey_date,
                header.start,
                header.end,
                Some("Edited note".into()),
                header.journey_kind,
            )
        })
        .unwrap();
        let mut output = Cursor::new(Vec::new());
        db.with_txn(|txn| archive::export_all_journeys_as_mldx(txn, &mut output, false))
            .unwrap();
        let mut reader = MldxReader::open(Cursor::new(output.into_inner())).unwrap();
        let (exported, _, _) = reader
            .load_single_journey_with_raw_data(&header.id)
            .unwrap()
            .unwrap();
        assert_eq!(exported.note.as_deref(), Some("Edited note"));
        assert_eq!(exported.to_proto().attributes, expected.attributes);

        let destination = TempDir::new("attributes-reimport").unwrap();
        let mut db = MainDb::open(destination.path().to_str().unwrap()).unwrap();
        db.with_txn(|txn| reader.import(txn, None)).unwrap();
        let stored = db
            .with_txn(|txn| txn.get_journey_header(&header.id))
            .unwrap()
            .unwrap();
        assert_eq!(stored.to_proto().attributes, expected.attributes);
    }
}
