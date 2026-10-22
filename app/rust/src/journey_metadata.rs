//! Typed journey display information, retaining unknown Protobuf data.

use flutter_rust_bridge::frb;

use crate::protos;

/// A read-only view of the metadata understood by this version.
/// Keep the owning `JourneyMetadata` when storing or exporting a journey.
#[derive(Clone, Debug, PartialEq, Eq)]
#[frb(non_opaque)]
pub enum JourneyMetadataDetails {
    Flight {
        flight_number: Option<String>,
        operating_airline_icao: Option<String>,
        aircraft_type_icao: Option<String>,
        registration: Option<String>,
    },
}

/// Owns the complete message so newer fields and categories survive round trips.
#[derive(Clone, Debug, Default, PartialEq)]
#[frb(opaque)]
pub struct JourneyMetadata {
    pub(crate) proto: protos::journey::JourneyMetadata,
}

impl JourneyMetadata {
    pub fn flight(
        flight_number: Option<String>,
        operating_airline_icao: Option<String>,
        aircraft_type_icao: Option<String>,
        registration: Option<String>,
    ) -> Self {
        let mut proto = protos::journey::JourneyMetadata::new();
        proto.set_flight(protos::journey::FlightMetadata {
            flight_number,
            operating_airline_icao,
            aircraft_type_icao,
            registration,
            ..Default::default()
        });
        Self { proto }
    }

    pub fn details(&self) -> Option<JourneyMetadataDetails> {
        use protos::journey::journey_metadata::Details;
        self.proto
            .details
            .as_ref()
            .map(|Details::Flight(flight)| JourneyMetadataDetails::Flight {
                flight_number: flight.flight_number.clone(),
                operating_airline_icao: flight.operating_airline_icao.clone(),
                aircraft_type_icao: flight.aircraft_type_icao.clone(),
                registration: flight.registration.clone(),
            })
    }
}
