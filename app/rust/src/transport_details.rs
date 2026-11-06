//! Typed transport details, retaining unknown Protobuf data.

use flutter_rust_bridge::frb;

use crate::protos;

/// A read-only view of the transport details understood by this version.
/// Keep the owning `TransportDetails` when storing or exporting a journey.
#[derive(Clone, Debug, PartialEq, Eq)]
#[frb(non_opaque)]
pub enum TransportDetailsView {
    Flight {
        flight_number: Option<String>,
        operating_airline_icao: Option<String>,
        aircraft_type_icao: Option<String>,
        registration: Option<String>,
    },
    Train {
        /// Full passenger-facing service name or number, e.g. "G123" or "のぞみ12号".
        service_name: Option<String>,
        /// Operator display name, e.g. "中国铁路" or "JR東海".
        operator_name: Option<String>,
    },
}

/// Owns the complete message so newer fields and categories survive round trips.
/// Boxing keeps optional transport details pointer-sized in journey headers.
#[derive(Clone, Debug, Default, PartialEq)]
#[frb(opaque)]
pub struct TransportDetails {
    pub(crate) proto: Box<protos::journey::TransportDetailsProto>,
}

impl TransportDetails {
    pub fn flight(
        flight_number: Option<String>,
        operating_airline_icao: Option<String>,
        aircraft_type_icao: Option<String>,
        registration: Option<String>,
    ) -> Self {
        let mut proto = Box::new(protos::journey::TransportDetailsProto::new());
        proto.set_flight(protos::journey::FlightDetails {
            flight_number,
            operating_airline_icao,
            aircraft_type_icao,
            registration,
            ..Default::default()
        });
        Self { proto }
    }

    pub fn train(service_name: Option<String>, operator_name: Option<String>) -> Self {
        let mut proto = Box::new(protos::journey::TransportDetailsProto::new());
        proto.set_train(protos::journey::TrainDetails {
            service_name,
            operator_name,
            ..Default::default()
        });
        Self { proto }
    }

    pub fn view(&self) -> Option<TransportDetailsView> {
        use protos::journey::transport_details_proto::Details;
        self.proto.details.as_ref().map(|details| match details {
            Details::Flight(flight) => TransportDetailsView::Flight {
                flight_number: flight.flight_number.clone(),
                operating_airline_icao: flight.operating_airline_icao.clone(),
                aircraft_type_icao: flight.aircraft_type_icao.clone(),
                registration: flight.registration.clone(),
            },
            Details::Train(train) => TransportDetailsView::Train {
                service_name: train.service_name.clone(),
                operator_name: train.operator_name.clone(),
            },
        })
    }
}
