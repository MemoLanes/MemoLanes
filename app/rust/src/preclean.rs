use crate::api::import::ImportPreprocessor;
use anyhow::Result;
use quick_xml::events::{BytesText, Event};
use quick_xml::{Reader, Writer};

pub fn analyze_and_prepare_gpx(xml: &str) -> Result<(String, ImportPreprocessor)> {
    let preprocessor = detect_gpx_preprocessor(xml);
    let xml = if matches!(preprocessor, ImportPreprocessor::StepOfMyWorld) {
        normalize_step_of_my_world_gpx(xml)?
    } else {
        xml.to_owned()
    };
    Ok((xml, preprocessor))
}

fn detect_gpx_preprocessor(xml: &str) -> ImportPreprocessor {
    const PROBE_LIMIT: usize = 8 * 1024;
    let head = &xml[..xml.len().min(PROBE_LIMIT)];

    if head.contains("stepofmyworld") || head.contains("StepOfMyWorld") || head.contains("yourapp")
    {
        ImportPreprocessor::StepOfMyWorld
    } else {
        ImportPreprocessor::Generic
    }
}

/// Step Of My World GPX contains localized Chinese time strings in <time>,
/// which breaks the GPX parser. Normalize them before parsing.
fn normalize_step_of_my_world_gpx(xml: &str) -> Result<String> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut writer = Writer::new(Vec::new());
    let mut buf = Vec::new();

    let mut modified = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) if e.name().as_ref() == b"time" => {
                let raw = reader.read_text(e.name())?;

                let normalized = match normalize_step_of_my_world_time(&raw) {
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
            Err(e) => anyhow::bail!("XML parse error during pre-clean: {e:?}"),
        }

        buf.clear();
    }

    if !modified {
        return Ok(xml.to_owned());
    }

    Ok(String::from_utf8(writer.into_inner())?)
}

fn normalize_step_of_my_world_time(input: &str) -> Option<String> {
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

    Some(format!("{}T{:02}:{}:{}Z", date, hour_24, min, sec))
}
