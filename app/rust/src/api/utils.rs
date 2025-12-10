use anyhow::Result;
pub use chrono::NaiveDate;
use flutter_rust_bridge::frb;

// TODO: frb does not support `chrono::NaiveDate`
#[frb(opaque)]
#[frb(mirror(NaiveDate))]
pub struct _NaiveDate {}

#[frb(sync)]
pub fn naive_date_to_string(date: &NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

#[frb(sync)]
pub fn naive_date_of_string(str: &str) -> Result<NaiveDate> {
    Ok(NaiveDate::parse_from_str(str, "%Y-%m-%d")?)
}
