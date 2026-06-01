use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::{Datelike, NaiveDate};

use super::journey::Journey;
use super::scope::{BucketKey, TimeBucket};

/// Group journeys into time buckets. Returns one entry per non-empty
/// bucket, in ascending chronological order. Empty buckets are NOT
/// returned.
///
/// Bucketing key: uses `Journey::date` (a `NaiveDate`, no time component).
///
/// **ISO week vs calendar year (`TimeBucket::Week`):** uses ISO 8601 weeks.
/// A journey on calendar date 2024-12-30 belongs to ISO week
/// `(iso_year=2025, iso_week=1)`, so late-December dates may surface in the
/// next ISO year's bucket.
pub fn group_by_time(
    journeys: &[Arc<Journey>],
    bucket: TimeBucket,
) -> Vec<(BucketKey, Vec<Arc<Journey>>)> {
    let mut groups: BTreeMap<BucketKey, Vec<Arc<Journey>>> = BTreeMap::new();
    for j in journeys {
        let key = bucket_for_date(bucket, j.date);
        groups.entry(key).or_default().push(j.clone());
    }
    groups.into_iter().collect()
}

fn bucket_for_date(bucket: TimeBucket, d: NaiveDate) -> BucketKey {
    match bucket {
        TimeBucket::Day => BucketKey::Day(d),
        TimeBucket::Week => {
            let iso = d.iso_week();
            BucketKey::Week {
                iso_year: iso.year(),
                iso_week: iso.week(),
            }
        }
        TimeBucket::Month => BucketKey::Month {
            year: d.year(),
            month: d.month(),
        },
        TimeBucket::Quarter => BucketKey::Quarter {
            year: d.year(),
            quarter: ((d.month() - 1) / 3) + 1,
        },
        TimeBucket::Year => BucketKey::Year(d.year()),
    }
}

#[cfg(test)]
mod tests {
    use crate::achievement::scope::{BucketKey, TimeBucket};
    use crate::achievement::test_strategies::{arb_journey_list, date_to_bucket_key, Grid};
    use proptest::prelude::*;
    use std::collections::HashSet;
    use std::sync::Arc;

    fn buckets_for(
        journeys: &[Arc<crate::achievement::journey::Journey>],
        bucket: TimeBucket,
    ) -> Vec<(BucketKey, Vec<Arc<crate::achievement::journey::Journey>>)> {
        super::group_by_time(journeys, bucket)
    }

    proptest! {
        // §8.3 partition: every input journey appears in exactly one bucket.
        #[test]
        fn partition_count(
            journeys in arb_journey_list(Grid::default_4x4()),
            bucket in prop_oneof![
                Just(TimeBucket::Day),
                Just(TimeBucket::Week),
                Just(TimeBucket::Month),
                Just(TimeBucket::Quarter),
                Just(TimeBucket::Year),
            ],
        ) {
            let groups = buckets_for(&journeys, bucket);
            let total: usize = groups.iter().map(|(_, js)| js.len()).sum();
            prop_assert_eq!(total, journeys.len());
        }

        // §8.3 key correctness: every key matches `bucket_key_for_scope` of
        // its journeys' dates.
        #[test]
        fn key_correctness(
            journeys in arb_journey_list(Grid::default_4x4()),
            bucket in prop_oneof![
                Just(TimeBucket::Day),
                Just(TimeBucket::Week),
                Just(TimeBucket::Month),
                Just(TimeBucket::Quarter),
                Just(TimeBucket::Year),
            ],
        ) {
            let groups = buckets_for(&journeys, bucket);
            for (k, js) in &groups {
                for j in js {
                    let expected = date_to_bucket_key(j.date, bucket);
                    prop_assert_eq!(*k, expected);
                }
            }
        }

        // §8.3 permutation invariance.
        #[test]
        fn permutation_invariance(
            journeys in arb_journey_list(Grid::default_4x4()),
            bucket in prop_oneof![
                Just(TimeBucket::Day),
                Just(TimeBucket::Week),
                Just(TimeBucket::Month),
                Just(TimeBucket::Quarter),
                Just(TimeBucket::Year),
            ],
        ) {
            let mut shuffled = journeys.clone();
            shuffled.reverse();
            let a = buckets_for(&journeys, bucket);
            let b = buckets_for(&shuffled, bucket);
            // Compare as set of (key, journey-id-set).
            let to_set = |gs: Vec<(BucketKey, Vec<Arc<crate::achievement::journey::Journey>>)>|
                -> HashSet<(BucketKey, Vec<String>)>
            {
                gs.into_iter().map(|(k, js)| {
                    let mut ids: Vec<String> = js.iter().map(|j| j.id.as_str().to_string()).collect();
                    ids.sort();
                    (k, ids)
                }).collect()
            };
            prop_assert_eq!(to_set(a), to_set(b));
        }
    }
}
