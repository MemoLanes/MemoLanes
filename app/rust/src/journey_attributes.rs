//! Typed journey display information, retaining unknown Protobuf data.

use flutter_rust_bridge::frb;

use crate::protos;

/// A read-only view of the attributes understood by this version.
/// Keep the owning `JourneyAttributes` when storing or exporting a journey.
#[derive(Clone, Debug, PartialEq, Eq)]
#[frb(non_opaque)]
pub enum JourneyAttributesView {
    Flight {
        flight_number: Option<String>,
        operating_airline_icao: Option<String>,
        aircraft_type_icao: Option<String>,
        registration: Option<String>,
    },
    Train {
        service_name: Option<String>,
        operator_name: Option<String>,
    },
}

/// Owns the complete message so newer fields and categories survive round trips.
/// Boxing keeps optional attributes pointer-sized in journey headers.
#[derive(Clone, Debug, Default, PartialEq)]
#[frb(opaque)]
pub struct JourneyAttributes {
    pub(crate) proto: Box<protos::journey::JourneyAttributes>,
}

impl JourneyAttributes {
    pub fn flight(
        flight_number: Option<String>,
        operating_airline_icao: Option<String>,
        aircraft_type_icao: Option<String>,
        registration: Option<String>,
    ) -> Self {
        let mut proto = Box::new(protos::journey::JourneyAttributes::new());
        proto.set_flight(protos::journey::FlightAttributes {
            flight_number,
            operating_airline_icao,
            aircraft_type_icao,
            registration,
            ..Default::default()
        });
        Self { proto }
    }

    pub fn train(service_name: Option<String>, operator_name: Option<String>) -> Self {
        let mut proto = Box::new(protos::journey::JourneyAttributes::new());
        proto.set_train(protos::journey::TrainAttributes {
            service_name,
            operator_name,
            ..Default::default()
        });
        Self { proto }
    }

    pub fn details(&self) -> Option<JourneyAttributesView> {
        use protos::journey::journey_attributes::Details;
        self.proto.details.as_ref().map(|details| match details {
            Details::Flight(flight) => JourneyAttributesView::Flight {
                flight_number: flight.flight_number.clone(),
                operating_airline_icao: flight.operating_airline_icao.clone(),
                aircraft_type_icao: flight.aircraft_type_icao.clone(),
                registration: flight.registration.clone(),
            },
            Details::Train(train) => JourneyAttributesView::Train {
                service_name: train.service_name.clone(),
                operator_name: train.operator_name.clone(),
            },
        })
    }
}
