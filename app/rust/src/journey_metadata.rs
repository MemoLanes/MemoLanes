//! Optional journey display information stored in the Header.
//!
//! Keys are extensible; preserve unknown entries and omit missing information.
//! Values are display strings. Adding keys requires no schema or MLDX version
//! change. Older readers can import the track but may discard this information.

use std::collections::HashMap;

use flutter_rust_bridge::frb;

use crate::protos;

/// Keys understood by this version of the application. The stored names are
/// part of the format contract and must not be repurposed for other meanings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JourneyMetadataKey {
    FlightNumber,
    FlightAirline,
    FlightAircraftType,
    FlightRegistration,
}

impl JourneyMetadataKey {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FlightNumber => "flight.number",
            Self::FlightAirline => "flight.airline",
            Self::FlightAircraftType => "flight.aircraft_type",
            Self::FlightRegistration => "flight.registration",
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[frb(non_opaque)]
pub struct JourneyMetadata {
    pub fields: HashMap<String, String>,
}

impl JourneyMetadata {
    pub fn get(&self, key: JourneyMetadataKey) -> Option<&str> {
        self.fields.get(key.as_str()).map(String::as_str)
    }

    pub fn set(&mut self, key: JourneyMetadataKey, value: impl Into<String>) {
        self.fields.insert(key.as_str().to_owned(), value.into());
    }

    pub fn remove(&mut self, key: JourneyMetadataKey) -> Option<String> {
        self.fields.remove(key.as_str())
    }

    pub(crate) fn of_proto(proto: protos::journey::JourneyMetadata) -> Self {
        Self {
            fields: proto.fields,
        }
    }

    pub(crate) fn to_proto(self) -> protos::journey::JourneyMetadata {
        let mut proto = protos::journey::JourneyMetadata::new();
        proto.fields = self.fields;
        proto
    }
}
