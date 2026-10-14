use crate::import_data::preprocessor::{detect_preprocessor, TrackFileFormat};
use crate::import_data::ImportPreprocessor;
use anyhow::Result;
use chrono::Datelike;
use quick_xml::events::{BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};

type TimeNormalizer = fn(&str) -> Option<String>;

pub fn analyze_and_prepare_gpx(xml: &str) -> Result<(String, ImportPreprocessor)> {
    let preprocessor = detect_preprocessor(xml, TrackFileFormat::Gpx);

    // TODO: `normalize_gpx_with` is actually doing a full XML parse + rewrite.
    // We should try to avoid calling it unless necessary.
    let xml = match preprocessor {
        ImportPreprocessor::Spare => normalize_gpx_with(xml, normalize_step_of_my_world_time)?,
        _ => normalize_gpx_with(xml, normalize_generic_time)?,
    };

    Ok((xml, preprocessor))
}

/// Accept the +180 endpoint in files from any source. GPX's longitude upper
/// bound is exclusive; keep the point on the eastern side of the dateline.
fn normalize_point_longitude<'a>(
    element: BytesStart<'a>,
    modified: &mut bool,
) -> Result<BytesStart<'a>> {
    if !matches!(element.local_name().as_ref(), "trkpt" | "rtept" | "wpt") {
        return Ok(element);
    }
    let Some(longitude) = element.try_get_attribute("lon")? else {
        return Ok(element);
    };
    if longitude
        .normalized_value(quick_xml::XmlVersion::Implicit1_0)?
        .parse::<f64>()
        != Ok(180.0)
    {
        return Ok(element);
    }

    let mut normalized = element.to_owned();
    normalized.clear_attributes();
    for attribute in element.attributes() {
        let attribute = attribute?;
        let value = if attribute.key.as_ref() == "lon" {
            180.0_f64.next_down().to_string().into()
        } else {
            attribute.normalized_value(quick_xml::XmlVersion::Implicit1_0)?
        };
        normalized.push_attribute((attribute.key.as_ref(), value));
    }
    *modified = true;
    Ok(normalized)
}

fn normalize_gpx_with(xml: &str, normalizer: TimeNormalizer) -> Result<String> {
    let mut reader = Reader::from_str(xml);

    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();

    let mut modified = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == "time" => {
                let raw = reader.read_text(e.name())?;
                let raw = raw.into_inner();
                let normalized = match normalizer(&raw) {
                    Some(v) => {
                        modified = true;
                        v
                    }
                    None => raw.into_owned(),
                };

                writer.write_event(Event::Start(e.to_owned()))?;
                writer.write_event(Event::Text(BytesText::new(&normalized)))?;
                writer.write_event(Event::End(e.to_end().to_owned()))?;
            }

            Ok(Event::Start(e)) => {
                writer.write_event(Event::Start(normalize_point_longitude(e, &mut modified)?))?
            }
            Ok(Event::Empty(e)) => {
                writer.write_event(Event::Empty(normalize_point_longitude(e, &mut modified)?))?
            }
            Ok(Event::Eof) => break,
            Ok(event) => writer.write_event(event)?,
            Err(e) => anyhow::bail!("XML parse error during GPX normalization: {e:?}"),
        }

        buf.clear();
    }

    if !modified {
        Ok(xml.to_owned())
    } else {
        Ok(String::from_utf8(writer.into_inner())?)
    }
}

/// Step Of My World：
/// <time>2023-08-01T下午3:12:45</time>
pub fn normalize_step_of_my_world_time(input: &str) -> Option<String> {
    let input = input.trim();

    if !input.contains('上') && !input.contains('下') {
        return None;
    }

    let input = input.trim_end_matches('Z');
    let (date, rest) = input.split_once('T')?;

    let (period, time) = if rest.starts_with("上午") {
        ("AM", rest.trim_start_matches("上午"))
    } else if rest.starts_with("下午") {
        ("PM", rest.trim_start_matches("下午"))
    } else {
        return None;
    };

    let mut parts = time.split(':');
    let hour: i32 = parts.next()?.parse().ok()?;
    let min = parts.next()?;
    let sec = parts.next()?;

    let hour_24 = match (period, hour) {
        ("AM", 12) => 0,
        ("AM", h) => h,
        ("PM", 12) => 12,
        ("PM", h) => h + 12,
        _ => return None,
    };

    Some(format!("{date}T{hour_24:02}:{min}:{sec}Z"))
}

pub fn normalize_generic_time(input: &str) -> Option<String> {
    let input = input.trim();

    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(input) {
        return Some(dt.with_timezone(&chrono::Utc).to_rfc3339());
    }

    let mut s = input.to_string();
    if s.len() >= 10 {
        let date = s[..10].replace('/', "-");
        s.replace_range(0..10, &date);
    }

    if let Some(idx) = s.rfind(" +") {
        if idx >= 19 {
            s.remove(idx);
        }
    }
    if let Some(idx) = s.rfind(" -") {
        if idx >= 19 {
            s.remove(idx);
        }
    }

    const WITH_OFFSET: &[&str] = &[
        "%Y-%m-%d %H:%M:%S %z",
        "%Y-%m-%dT%H:%M:%S%.f%z",
        "%Y-%m-%dT%H:%M:%S%z",
    ];

    const UTC_FORMATS: &[&str] = &[
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
    ];

    for fmt in WITH_OFFSET {
        if let Ok(dt) = chrono::DateTime::parse_from_str(&s, fmt) {
            if dt.year() < 0 {
                continue;
            }
            return Some(dt.with_timezone(&chrono::Utc).to_rfc3339());
        }
    }

    for fmt in UTC_FORMATS {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(&s, fmt) {
            if dt.year() < 0 {
                continue;
            }
            return Some(
                chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(dt, chrono::Utc)
                    .to_rfc3339(),
            );
        }
    }

    None
}
