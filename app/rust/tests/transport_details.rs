use std::io::Cursor;

use memolanes_core::{
    archive::{self, MldxReader},
    journey_data::JourneyData,
    journey_header::{JourneyHeader, JourneyKind, JourneyType},
    journey_vector::JourneyVector,
    main_db::MainDb,
    transport_details::{TransportDetails, TransportDetailsView},
};
use protobuf::{Message, UnknownFields};
use tempdir::TempDir;

fn flight() -> TransportDetails {
    TransportDetails::flight(
        Some("MU5101".into()),
        Some("CES".into()),
        Some("A359".into()),
        Some("B-304D".into()),
    )
}

fn train() -> TransportDetails {
    TransportDetails::train(Some("のぞみ12号".into()), Some("JR東海".into()))
}

fn journey(
    journey_kind: JourneyKind,
    transport_details: Option<TransportDetails>,
) -> JourneyHeader {
    JourneyHeader {
        id: "journey".into(),
        revision: "original".into(),
        journey_date: "2026-10-22".parse().unwrap(),
        created_at: "2026-10-22T01:00:00Z".parse().unwrap(),
        updated_at: None,
        start: None,
        end: None,
        journey_type: JourneyType::Vector,
        journey_kind,
        note: None,
        postprocessor_algo: None,
        has_raw_data: false,
        transport_details,
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
fn optional_transport_details_and_display_fields_survive_mldx_roundtrip() {
    for (kind, transport_details, expected_view) in [
        (JourneyKind::DefaultKind, None, None),
        (
            JourneyKind::DefaultKind,
            Some(TransportDetails::default()),
            None,
        ),
        (
            JourneyKind::Flight,
            Some(flight()),
            Some(TransportDetailsView::Flight {
                flight_number: Some("MU5101".into()),
                operating_airline_icao: Some("CES".into()),
                aircraft_type_icao: Some("A359".into()),
                registration: Some("B-304D".into()),
            }),
        ),
        (
            JourneyKind::Flight,
            Some(TransportDetails::flight(
                None,
                Some(String::new()),
                None,
                None,
            )),
            Some(TransportDetailsView::Flight {
                flight_number: None,
                operating_airline_icao: Some(String::new()),
                aircraft_type_icao: None,
                registration: None,
            }),
        ),
        (
            JourneyKind::DefaultKind,
            Some(train()),
            Some(TransportDetailsView::Train {
                service_name: Some("のぞみ12号".into()),
                operator_name: Some("JR東海".into()),
            }),
        ),
        (
            JourneyKind::DefaultKind,
            Some(TransportDetails::train(None, None)),
            Some(TransportDetailsView::Train {
                service_name: None,
                operator_name: None,
            }),
        ),
        (
            JourneyKind::DefaultKind,
            Some(TransportDetails::train(None, Some(String::new()))),
            Some(TransportDetailsView::Train {
                service_name: None,
                operator_name: Some(String::new()),
            }),
        ),
        (
            JourneyKind::DefaultKind,
            Some(TransportDetails::train(Some(String::new()), None)),
            Some(TransportDetailsView::Train {
                service_name: Some(String::new()),
                operator_name: None,
            }),
        ),
    ] {
        let header = journey(kind, transport_details);
        let mut reader = MldxReader::open(encode(header.clone())).unwrap();
        let (loaded, _, _) = reader
            .load_single_journey_with_raw_data(&header.id)
            .unwrap()
            .unwrap();
        assert_eq!(loaded, header);
        assert_eq!(
            loaded
                .transport_details
                .as_ref()
                .and_then(TransportDetails::view),
            expected_view,
        );
    }
}

#[test]
fn unknown_fields_and_categories_survive_import_edit_export_and_reimport() {
    for (transport_details, kind) in [
        (flight(), JourneyKind::Flight),
        (train(), JourneyKind::DefaultKind),
        (TransportDetails::default(), JourneyKind::DefaultKind),
    ] {
        let header = journey(kind, Some(transport_details));
        let mut expected = header.to_proto();
        let details = expected.transport_details.as_mut().unwrap();
        add_future_fields(details.special_fields.mut_unknown_fields());
        if details.has_flight() {
            add_future_fields(details.mut_flight().special_fields.mut_unknown_fields());
        } else if details.has_train() {
            add_future_fields(details.mut_train().special_fields.mut_unknown_fields());
        } else {
            // Future oneof variant: CruiseDetails { string ship_name = 1; }.
            details
                .special_fields
                .mut_unknown_fields()
                .add_length_delimited(20, b"\x0a\x04Ship".to_vec());
        }
        let bytes = expected.write_to_bytes().unwrap();
        let header = JourneyHeader::of_proto(Message::parse_from_bytes(&bytes).unwrap()).unwrap();
        if !expected.transport_details.as_ref().unwrap().has_flight()
            && !expected.transport_details.as_ref().unwrap().has_train()
        {
            assert_eq!(header.transport_details.as_ref().unwrap().view(), None);
        }

        let directory = TempDir::new("transport-details-roundtrip").unwrap();
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
        assert_eq!(
            exported.to_proto().transport_details,
            expected.transport_details
        );

        let destination = TempDir::new("transport-details-reimport").unwrap();
        let mut db = MainDb::open(destination.path().to_str().unwrap()).unwrap();
        db.with_txn(|txn| reader.import(txn, None)).unwrap();
        let stored = db
            .with_txn(|txn| txn.get_journey_header(&header.id))
            .unwrap()
            .unwrap();
        assert_eq!(
            stored.to_proto().transport_details,
            expected.transport_details
        );
    }
}
