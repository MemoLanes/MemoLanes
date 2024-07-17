pub use chrono::NaiveDate;
use flutter_rust_bridge::frb;

// TODO: frb does not support `chrono::NaiveDate`
#[frb(opaque)]
#[frb(mirror(NaiveDate))]
pub struct _NaiveDate {}

#[frb(sync)]
pub fn naive_date_to_string(date: &chrono::NaiveDate) -> String {
    date.to_string()
}
