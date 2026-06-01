use chrono::NaiveDate;

use crate::journey_header::JourneyKind;

/// Declarative filter for which journeys to load.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Scope {
    AllTime,
    Year(i32),
    DateRange {
        from: NaiveDate,
        to: NaiveDate,
    },
    Kind(JourneyKind),
    KindAndDateRange {
        kind: JourneyKind,
        from: NaiveDate,
        to: NaiveDate,
    },
}

/// Time bucket granularity for `group_by_time`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TimeBucket {
    Day,
    Week,
    Month,
    Quarter,
    Year,
}

/// Identifies one bucket. Encodes both the granularity and the position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum BucketKey {
    Day(NaiveDate),
    Week { iso_year: i32, iso_week: u32 },
    Month { year: i32, month: u32 },
    Quarter { year: i32, quarter: u32 },
    Year(i32),
}
