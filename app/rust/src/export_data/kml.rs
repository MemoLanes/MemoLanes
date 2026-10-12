use crate::export_data::JourneyExport;
use crate::raw_data::JourneyRawData;
use anyhow::{Context, Ok, Result};
use auto_context::auto_context;
use chrono::{DateTime, SecondsFormat, Utc};
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use std::io::Write;

fn write_text<W: Write>(xml: &mut Writer<W>, tag: &str, value: &str) -> Result<()> {
    xml.write_event(Event::Start(BytesStart::new(tag)))?;
    xml.write_event(Event::Text(BytesText::new(value)))?;
    xml.write_event(Event::End(BytesEnd::new(tag)))?;
    Ok(())
}

fn write_data<W: Write>(xml: &mut Writer<W>, name: &str, value: &str) -> Result<()> {
    let mut data = BytesStart::new("Data");
    let name = format!("memolanes:{name}");
    data.push_attribute(("name", name.as_str()));
    xml.write_event(Event::Start(data))?;
    write_text(xml, "value", value)?;
    xml.write_event(Event::End(BytesEnd::new("Data")))?;
    Ok(())
}

/// Streams each Folder, Placemark and coordinate without building a KML tree
/// or a segment-sized coordinate string in memory.
pub(crate) struct KmlJourneyWriter<W: Write> {
    xml: Writer<W>,
}

impl<W: Write> KmlJourneyWriter<W> {
    pub(crate) fn new(writer: W) -> Result<Self> {
        Self::new_document(writer, false)
    }

    pub(crate) fn new_raw_data(writer: W) -> Result<Self> {
        Self::new_document(writer, true)
    }

    fn new_document(writer: W, raw_data: bool) -> Result<Self> {
        let mut xml = Writer::new(writer);
        xml.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
        let mut root = BytesStart::new("kml");
        root.push_attribute(("xmlns", "http://www.opengis.net/kml/2.2"));
        if raw_data {
            root.push_attribute(("xmlns:gx", "http://www.google.com/kml/ext/2.2"));
        }
        xml.write_event(Event::Start(root))?;
        xml.write_event(Event::Start(BytesStart::new("Document")))?;
        Ok(Self { xml })
    }

    pub(crate) fn write_journey(&mut self, journey: &JourneyExport) -> Result<()> {
        let xml = &mut self.xml;
        xml.write_event(Event::Start(BytesStart::new("Folder")))?;
        write_text(
            xml,
            "name",
            &format!("MemoLanes Journey {}", journey.journey_date),
        )?;
        xml.write_event(Event::Start(BytesStart::new("TimeStamp")))?;
        write_text(xml, "when", &journey.journey_date.to_string())?;
        xml.write_event(Event::End(BytesEnd::new("TimeStamp")))?;
        xml.write_event(Event::Start(BytesStart::new("ExtendedData")))?;
        write_data(xml, "date", &journey.journey_date.to_string())?;
        if let Some(start) = journey.start {
            write_data(xml, "start", &start.to_rfc3339())?;
        }
        if let Some(end) = journey.end {
            write_data(xml, "end", &end.to_rfc3339())?;
        }
        xml.write_event(Event::End(BytesEnd::new("ExtendedData")))?;

        for segment in &journey.vector.track_segments {
            xml.write_event(Event::Start(BytesStart::new("Placemark")))?;
            write_text(xml, "name", "MemoLanes Track Segment")?;
            xml.write_event(Event::Start(BytesStart::new("LineString")))?;
            write_text(xml, "tessellate", "1")?;
            xml.write_event(Event::Start(BytesStart::new("coordinates")))?;
            for point in &segment.track_points {
                // Coordinate text contains only numbers and separators; no XML escaping is needed.
                writeln!(xml.get_mut(), "{},{}", point.longitude, point.latitude)?;
            }
            xml.write_event(Event::End(BytesEnd::new("coordinates")))?;
            xml.write_event(Event::End(BytesEnd::new("LineString")))?;
            xml.write_event(Event::End(BytesEnd::new("Placemark")))?;
        }
        xml.write_event(Event::End(BytesEnd::new("Folder")))?;
        Ok(())
    }

    pub(crate) fn write_raw_data(&mut self, data: &JourneyRawData, name: &str) -> Result<()> {
        let xml = &mut self.xml;
        xml.write_event(Event::Start(BytesStart::new("Folder")))?;
        write_text(xml, "name", name)?;
        write_text(xml, "description", "Raw GPS data exported by MemoLanes")?;
        xml.write_event(Event::Start(BytesStart::new("Placemark")))?;
        write_text(xml, "name", "MemoLanes Raw Data")?;
        xml.write_event(Event::Start(BytesStart::new("gx:Track")))?;
        // gx:Track requires every `when` before every `gx:coord`.
        for point in &data.points {
            let timestamp_ms = point
                .raw_gps_point
                .timestamp_ms
                .unwrap_or(point.received_timestamp_ms);
            let timestamp = DateTime::<Utc>::from_timestamp_millis(timestamp_ms)
                .ok_or_else(|| anyhow!("Raw GPS timestamp is out of range: {timestamp_ms}"))?;
            write_text(
                xml,
                "when",
                &timestamp.to_rfc3339_opts(SecondsFormat::Millis, true),
            )?;
        }
        for point in &data.points {
            let raw = &point.raw_gps_point;
            write_text(
                xml,
                "gx:coord",
                &format!(
                    "{} {} {}",
                    raw.point.longitude,
                    raw.point.latitude,
                    raw.altitude.unwrap_or_default()
                ),
            )?;
        }
        xml.write_event(Event::End(BytesEnd::new("gx:Track")))?;
        xml.write_event(Event::Start(BytesStart::new("LineString")))?;
        write_text(xml, "tessellate", "1")?;
        xml.write_event(Event::Start(BytesStart::new("coordinates")))?;
        for point in &data.points {
            let raw = &point.raw_gps_point;
            match raw.altitude {
                Some(altitude) => writeln!(
                    xml.get_mut(),
                    "{},{},{}",
                    raw.point.longitude,
                    raw.point.latitude,
                    f64::from(altitude)
                )?,
                None => writeln!(
                    xml.get_mut(),
                    "{},{}",
                    raw.point.longitude,
                    raw.point.latitude
                )?,
            }
        }
        xml.write_event(Event::End(BytesEnd::new("coordinates")))?;
        xml.write_event(Event::End(BytesEnd::new("LineString")))?;
        xml.write_event(Event::End(BytesEnd::new("Placemark")))?;
        xml.write_event(Event::End(BytesEnd::new("Folder")))?;
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<()> {
        self.xml
            .write_event(Event::End(BytesEnd::new("Document")))?;
        self.xml.write_event(Event::End(BytesEnd::new("kml")))?;
        self.xml.get_mut().flush()?;
        Ok(())
    }
}

#[auto_context]
pub fn journeys_to_kml_file<T: Write>(journeys: &[JourneyExport], writer: &mut T) -> Result<()> {
    if journeys.is_empty() {
        anyhow::bail!("No track segments");
    }
    let mut xml = KmlJourneyWriter::new(writer)?;
    for journey in journeys {
        xml.write_journey(journey)?;
    }
    xml.finish()
}

#[auto_context]
pub fn journey_raw_data_to_kml_file<T: Write>(
    raw_data: &JourneyRawData,
    writer: &mut T,
) -> Result<()> {
    let mut writer = KmlJourneyWriter::new_raw_data(writer)?;
    writer.write_raw_data(raw_data, "MemoLanes Raw Data")?;
    writer.finish()
}
