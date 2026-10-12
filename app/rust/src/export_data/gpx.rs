use crate::export_data::JourneyExport;
use crate::gps::ExtendedRawGPSPoint;
use crate::gpx_file_utils::MEMOLANES_NAMESPACE;
use crate::raw_data::JourneyRawData;
use anyhow::{Context, Ok, Result};
use auto_context::auto_context;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use std::io::Write;
use time::OffsetDateTime;

pub use crate::gpx_file_utils::JOURNEY_TYPE_NAME;
pub const RAW_DATA_TYPE_NAME: &str = "MemoLanes RawData";

fn write_text_element<W: Write>(writer: &mut Writer<W>, name: &str, value: &str) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new(name)))?;
    writer.write_event(Event::Text(BytesText::new(value)))?;
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

fn track_point_element(latitude: f64, longitude: f64) -> BytesStart<'static> {
    let mut point = BytesStart::new("trkpt");
    point.push_attribute(("lat", latitude.to_string().as_str()));
    point.push_attribute(("lon", longitude.to_string().as_str()));
    point
}

/// The gpx crate ignores arbitrary extensions, so the MemoLanes format uses
/// quick-xml and writes one journey and segment at a time.
pub(crate) struct GpxJourneyWriter<W: Write> {
    xml: Writer<W>,
}

impl<W: Write> GpxJourneyWriter<W> {
    pub(crate) fn new(writer: W) -> Result<Self> {
        Self::new_document(writer, false)
    }

    pub(crate) fn new_raw_data(writer: W) -> Result<Self> {
        Self::new_document(writer, true)
    }

    fn new_document(writer: W, raw_data: bool) -> Result<Self> {
        let mut xml = Writer::new(writer);
        xml.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
        let mut root = BytesStart::new("gpx");
        root.push_attribute(("version", "1.1"));
        root.push_attribute(("creator", "MemoLanes"));
        root.push_attribute(("xmlns", "http://www.topografix.com/GPX/1/1"));
        root.push_attribute(("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"));
        if !raw_data {
            root.push_attribute(("xmlns:memolanes", MEMOLANES_NAMESPACE));
        }
        xml.write_event(Event::Start(root))?;
        if raw_data {
            xml.write_event(Event::Start(BytesStart::new("metadata")))?;
            write_text_element(&mut xml, "name", RAW_DATA_TYPE_NAME)?;
            xml.write_event(Event::End(BytesEnd::new("metadata")))?;
        }
        Ok(Self { xml })
    }

    pub(crate) fn write_journey(&mut self, journey: &JourneyExport) -> Result<()> {
        let xml = &mut self.xml;
        xml.write_event(Event::Start(BytesStart::new("trk")))?;
        write_text_element(
            xml,
            "name",
            &format!("MemoLanes Journey {}", journey.journey_date),
        )?;
        write_text_element(xml, "type", JOURNEY_TYPE_NAME)?;
        xml.write_event(Event::Start(BytesStart::new("extensions")))?;
        let date = journey.journey_date.to_string();
        let start = journey.start.map(|value| value.to_rfc3339());
        let end = journey.end.map(|value| value.to_rfc3339());
        let mut metadata = BytesStart::new("memolanes:journey");
        metadata.push_attribute(("date", date.as_str()));
        if let Some(value) = start.as_deref() {
            metadata.push_attribute(("start", value));
        }
        if let Some(value) = end.as_deref() {
            metadata.push_attribute(("end", value));
        }
        xml.write_event(Event::Empty(metadata))?;
        xml.write_event(Event::End(BytesEnd::new("extensions")))?;

        for segment in &journey.vector.track_segments {
            xml.write_event(Event::Start(BytesStart::new("trkseg")))?;
            for point in &segment.track_points {
                xml.write_event(Event::Empty(track_point_element(
                    point.latitude,
                    point.longitude,
                )))?;
            }
            xml.write_event(Event::End(BytesEnd::new("trkseg")))?;
        }
        xml.write_event(Event::End(BytesEnd::new("trk")))?;
        Ok(())
    }

    pub(crate) fn write_raw_data(&mut self, data: &JourneyRawData, name: &str) -> Result<()> {
        self.write_raw_data_points(data.points.iter().cloned().map(Ok), name)
    }

    /// Accepts a fallible iterator so legacy CSV can be exported without buffering it.
    pub(crate) fn write_raw_data_points(
        &mut self,
        points: impl IntoIterator<Item = Result<ExtendedRawGPSPoint>>,
        name: &str,
    ) -> Result<()> {
        let xml = &mut self.xml;
        xml.write_event(Event::Start(BytesStart::new("trk")))?;
        write_text_element(xml, "name", name)?;
        write_text_element(xml, "type", RAW_DATA_TYPE_NAME)?;
        xml.write_event(Event::Start(BytesStart::new("trkseg")))?;
        for point in points {
            let point = point?;
            let raw = &point.raw_gps_point;
            xml.write_event(Event::Start(track_point_element(
                raw.point.latitude,
                raw.point.longitude,
            )))?;
            if let Some(elevation) = raw.altitude {
                write_text_element(xml, "ele", &f64::from(elevation).to_string())?;
            }
            let timestamp_ms = raw.timestamp_ms.unwrap_or(point.received_timestamp_ms);
            if timestamp_ms > 0 {
                let time: gpx::Time = OffsetDateTime::from_unix_timestamp_nanos(
                    i128::from(timestamp_ms) * 1_000_000,
                )?
                .into();
                write_text_element(xml, "time", &time.format()?)?;
            }
            if let Some(accuracy) = raw.accuracy {
                write_text_element(xml, "hdop", &f64::from(accuracy).to_string())?;
            }
            xml.write_event(Event::End(BytesEnd::new("trkpt")))?;
        }
        xml.write_event(Event::End(BytesEnd::new("trkseg")))?;
        xml.write_event(Event::End(BytesEnd::new("trk")))?;
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<()> {
        self.xml.write_event(Event::End(BytesEnd::new("gpx")))?;
        self.xml.get_mut().flush()?;
        Ok(())
    }
}

#[auto_context]
pub fn journeys_to_gpx_file<T: Write>(journeys: &[JourneyExport], writer: &mut T) -> Result<()> {
    if journeys.is_empty() {
        anyhow::bail!("No track segments");
    }
    let mut xml = GpxJourneyWriter::new(writer)?;
    for journey in journeys {
        xml.write_journey(journey)?;
    }
    xml.finish()
}

#[auto_context]
pub fn journey_raw_data_to_gpx_file<W: Write>(
    raw_data: &JourneyRawData,
    writer: &mut W,
) -> Result<()> {
    let mut writer = GpxJourneyWriter::new_raw_data(writer)?;
    writer.write_raw_data(raw_data, "MemoLanes Raw Data")?;
    writer.finish()
}
