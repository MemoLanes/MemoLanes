use anyhow::Result;
use chrono::{DateTime, Utc};
use flutter_rust_bridge::frb;
use protobuf::EnumOrUnknown;
use strum_macros::EnumIter;

use crate::{protos, utils};

#[derive(Copy, Clone, Debug, EnumIter, PartialEq, Eq, Hash)]
#[repr(i8)]
pub enum JourneyType {
    Vector = 0,
    Bitmap = 1,
}

impl JourneyType {
    pub fn to_int(&self) -> i8 {
        *self as i8
    }

    pub fn of_int(i: i8) -> Result<Self> {
        match i {
            0 => Ok(JourneyType::Vector),
            1 => Ok(JourneyType::Bitmap),
            _ => bail!("Invalid int for `JourneyType` {}", i),
        }
    }

    pub fn to_proto(self) -> protos::journey::header::Type {
        use protos::journey::header::Type;
        match self {
            JourneyType::Vector => Type::Vector,
            JourneyType::Bitmap => Type::BITMAP,
        }
    }

    pub fn of_proto(proto: protos::journey::header::Type) -> Self {
        use protos::journey::header::Type;
        match proto {
            Type::Vector => JourneyType::Vector,
            Type::BITMAP => JourneyType::Bitmap,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::JourneyType;
    use strum::IntoEnumIterator;

    #[test]
    fn int_conversion() {
        for type_ in JourneyType::iter() {
            assert_eq!(
                type_,
                JourneyType::of_int(JourneyType::to_int(&type_)).unwrap()
            )
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum JourneyKind {
    DefaultKind,
    Flight,
    Custom(String),
}

impl JourneyKind {
    pub fn to_proto(self) -> protos::journey::header::Kind {
        use protos::journey::header::{kind, Kind};
        let mut kind = Kind::new();
        match self {
            JourneyKind::DefaultKind => kind.set_build_in(kind::BuiltIn::DEFAULT),
            JourneyKind::Flight => kind.set_build_in(kind::BuiltIn::FLIGHT),
            JourneyKind::Custom(str) => kind.set_custom_kind(str),
        };
        kind
    }

    pub fn of_proto(mut proto: protos::journey::header::Kind) -> Self {
        use protos::journey::header::kind;
        if proto.has_build_in() {
            match proto.build_in() {
                kind::BuiltIn::DEFAULT => JourneyKind::DefaultKind,
                kind::BuiltIn::FLIGHT => JourneyKind::Flight,
            }
        } else {
            JourneyKind::Custom(proto.take_custom_kind())
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
#[frb(non_opaque)]
pub struct JourneyHeader {
    pub id: String,
    pub revision: String,
    pub journey_date: chrono::NaiveDate,
    pub created_at: DateTime<Utc>,
    pub updated_at: Option<DateTime<Utc>>,
    pub start: Option<DateTime<Utc>>,
    pub end: Option<DateTime<Utc>>,
    pub journey_type: JourneyType,
    pub journey_kind: JourneyKind,
    pub note: Option<String>,
}

impl JourneyHeader {
    pub fn of_proto(mut proto: protos::journey::Header) -> Result<Self> {
        let journey_type = proto
            .type_
            .enum_value()
            .map_err(|x| anyhow!("Unknown proto journey type: {}", x))?;
        Ok(JourneyHeader {
            id: proto.id,
            revision: proto.revision,
            journey_date: utils::date_of_days_since_epoch(proto.journey_date__days_since_epoch)
                .unwrap(),
            created_at: DateTime::from_timestamp(proto.created_at__timestamp_sec, 0).unwrap(),
            updated_at: proto
                .updated_at__timestamp_sec
                .and_then(|sec| DateTime::from_timestamp(sec, 0)),
            end: proto
                .end__timestamp_sec
                .and_then(|sec| DateTime::from_timestamp(sec, 0)),
            start: proto
                .start__timestamp_sec
                .and_then(|sec| DateTime::from_timestamp(sec, 0)),
            journey_type: JourneyType::of_proto(journey_type),
            journey_kind: JourneyKind::of_proto(match proto.kind.take() {
                None => bail!("Missing `kind`"),
                Some(kind) => kind,
            }),
            note: proto.note,
        })
    }

    pub fn to_proto(self) -> protos::journey::Header {
        let mut proto = protos::journey::Header::new();
        proto.id = self.id;
        proto.revision = self.revision;
        proto.journey_date__days_since_epoch = utils::date_to_days_since_epoch(self.journey_date);
        proto.created_at__timestamp_sec = self.created_at.timestamp();
        proto.updated_at__timestamp_sec = self.updated_at.map(|x| x.timestamp());
        proto.end__timestamp_sec = self.end.map(|x| x.timestamp());
        proto.start__timestamp_sec = self.start.map(|x| x.timestamp());
        proto.type_ = EnumOrUnknown::new(self.journey_type.to_proto());
        proto.kind.0 = Some(Box::new(self.journey_kind.to_proto()));
        proto.note = self.note;
        proto
    }
}
