use crate::api::import::ImportPreprocessor;
use crate::export_data::JOURNEY_TYPE_NAME;
use anyhow::Result;
use chrono::Datelike;
use quick_xml::events::{BytesText, Event};
use quick_xml::{Reader, Writer};

type TimeNormalizer = fn(&str) -> Option<String>;

pub fn analyze_and_prepare_gpx(xml: &str) -> Result<(String, ImportPreprocessor)> {
    let preprocessor = detect_gpx_preprocessor(xml);

    // TODO: `normalize_gpx_time_with` is actually doing a full XML parse + rewrite.
    // We should try to avoid calling it unless necessary.
    let xml = match preprocessor {
        ImportPreprocessor::Spare => normalize_gpx_time_with(xml, normalize_step_of_my_world_time)?,
        _ => normalize_gpx_time_with(xml, normalize_generic_time)?,
    };

    Ok((xml, preprocessor))
}

fn detect_gpx_preprocessor(xml: &str) -> ImportPreprocessor {
    // TODO: This is pretty hacky: trying to match some known strings at the
    // beginning of the file.
    // Better approach could be parsing the XML / actually detecting the
    // properties of the data in it (is it actually spare?)
    const PROBE_LIMIT: usize = 8 * 1024;

    let head = xml
        .chars()
        .take(PROBE_LIMIT)
        .collect::<String>()
        .to_ascii_lowercase();

    // stepofmyworld (一生足迹), yourapp (灵感足迹)
    if head.contains("stepofmyworld") || head.contains("yourapp") {
        ImportPreprocessor::Spare
    } else if head.contains(&JOURNEY_TYPE_NAME.to_ascii_lowercase()) {
        ImportPreprocessor::None
    } else {
        ImportPreprocessor::Generic
    }
}

fn normalize_gpx_time_with(xml: &str, normalizer: TimeNormalizer) -> Result<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();

    let mut modified = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"time" => {
                let raw = reader.read_text(e.name())?;
                let normalized = match normalizer(&raw) {
                    Some(v) => {
                        modified = true;
                        v
                    }
                    None => raw.parse()?,
                };

                writer.write_event(Event::Start(e.to_owned()))?;
                writer.write_event(Event::Text(BytesText::new(&normalized)))?;
                writer.write_event(Event::End(e.to_end().to_owned()))?;
            }

            Ok(Event::Start(e)) => writer.write_event(Event::Start(e.into_owned()))?,
            Ok(Event::Empty(e)) => writer.write_event(Event::Empty(e.into_owned()))?,
            Ok(Event::End(e)) => writer.write_event(Event::End(e.into_owned()))?,
            Ok(Event::Text(e)) => writer.write_event(Event::Text(e.into_owned()))?,
            Ok(Event::CData(e)) => writer.write_event(Event::CData(e.into_owned()))?,
            Ok(Event::Decl(e)) => writer.write_event(Event::Decl(e.into_owned()))?,
            Ok(Event::Eof) => break,

            Ok(_) => {}
            Err(e) => anyhow::bail!("XML parse error during GPX time normalize: {e:?}"),
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
