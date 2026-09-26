use crate::export_data::JourneyExport;
use crate::gpx_file_utils::MEMOLANES_NAMESPACE;
use crate::journey_vector::JourneyVector;
use crate::storage::RawCsvRow;
use anyhow::{Context, Ok, Result};
use auto_context::auto_context;
use csv::Reader;
use geo_types::Point;
use gpx::{Gpx, GpxVersion, Metadata, Track, TrackSegment, Waypoint};
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;
use std::io::{Seek, Write};
use time::{Duration, OffsetDateTime};

fn write_gpx_with_segments<T: Write + Seek>(
    segments: Vec<TrackSegment>,
    name: Option<&str>,
    writer: &mut T,
) -> Result<()> {
    if segments.is_empty() {
        anyhow::bail!("No track segments");
    }

    let track = Track {
        name: Some("MemoLanes Track".to_string()),
        comment: None,
        description: None,
        source: None,
        links: vec![],
        type_: None,
        number: None,
        segments,
    };

    let gpx = Gpx {
        version: GpxVersion::Gpx11,
        creator: Some("MemoLanes".to_string()),
        metadata: Some(Metadata {
            name: name.map(str::to_string),
            ..Default::default()
        }),
        waypoints: vec![],
        tracks: vec![track],
        routes: vec![],
    };

    gpx::write(&gpx, writer)?;
    Ok(())
}

pub use crate::gpx_file_utils::JOURNEY_TYPE_NAME;
pub const RAWDATA_TYPE_NAME: &str = "MemoLanes RawData";

#[auto_context]
pub fn journey_vector_to_gpx_file<T: Write + Seek>(
    journey_vector: &JourneyVector,
    writer: &mut T,
) -> Result<()> {
    let mut segments = Vec::new();

    for track_segment in &journey_vector.track_segments {
        let mut points = Vec::new();
        track_segment.track_points.iter().for_each(|point| {
            points.push(Waypoint::new(Point::new(point.longitude, point.latitude)));
        });
        segments.push(TrackSegment { points });
    }
    write_gpx_with_segments(segments, Some(JOURNEY_TYPE_NAME), writer)
}

fn write_text_element<W: Write>(writer: &mut Writer<W>, name: &str, value: &str) -> Result<()> {
    writer.write_event(Event::Start(BytesStart::new(name)))?;
    writer.write_event(Event::Text(BytesText::new(value)))?;
    writer.write_event(Event::End(BytesEnd::new(name)))?;
    Ok(())
}

/// The gpx crate ignores arbitrary extensions, so the MemoLanes format uses
/// quick-xml and writes one journey and segment at a time.
pub(crate) struct GpxJourneyWriter<W: Write> {
    xml: Writer<W>,
}

impl<W: Write> GpxJourneyWriter<W> {
    pub(crate) fn new(writer: W) -> Result<Self> {
        let mut xml = Writer::new(writer);
        xml.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
        let mut root = BytesStart::new("gpx");
        root.push_attribute(("version", "1.1"));
        root.push_attribute(("creator", "MemoLanes"));
        root.push_attribute(("xmlns", "http://www.topografix.com/GPX/1/1"));
        root.push_attribute(("xmlns:xsi", "http://www.w3.org/2001/XMLSchema-instance"));
        root.push_attribute(("xmlns:memolanes", MEMOLANES_NAMESPACE));
        xml.write_event(Event::Start(root))?;
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
        metadata.push_attribute(("version", "1"));
        metadata.push_attribute(("sourceJourneyId", journey.source_journey_id.as_str()));
        metadata.push_attribute(("sourceRevision", journey.source_revision.as_str()));
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
                let latitude = point.latitude.to_string();
                let longitude = point.longitude.to_string();
                let mut track_point = BytesStart::new("trkpt");
                track_point.push_attribute(("lat", latitude.as_str()));
                track_point.push_attribute(("lon", longitude.as_str()));
                xml.write_event(Event::Empty(track_point))?;
            }
            xml.write_event(Event::End(BytesEnd::new("trkseg")))?;
        }
        xml.write_event(Event::End(BytesEnd::new("trk")))?;
        Ok(())
    }

    pub(crate) fn finish(mut self) -> Result<()> {
        self.xml.write_event(Event::End(BytesEnd::new("gpx")))?;
        Ok(())
    }
}

#[auto_context]
pub(crate) fn journeys_to_gpx_file<T: Write + Seek>(
    journeys: &[JourneyExport],
    writer: &mut T,
) -> Result<()> {
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
pub fn raw_data_csv_to_gpx_file<R: std::io::Read, W: Write + Seek>(
    csv_reader: &mut Reader<R>,
    writer: &mut W,
) -> Result<()> {
    let mut segment = TrackSegment { points: Vec::new() };

    for result in csv_reader.deserialize::<RawCsvRow>() {
        let raw: RawCsvRow = result?;

        let mut wp = Waypoint::new(Point::new(raw.longitude, raw.latitude));

        if let Some(ts) = raw.timestamp_ms {
            if ts > 0 {
                let dt = OffsetDateTime::UNIX_EPOCH + Duration::milliseconds(ts);
                wp.time = Some(dt.into());
            }
        }

        if let Some(alt) = raw.altitude {
            wp.elevation = Some(alt as f64);
        }

        if let Some(acc) = raw.accuracy {
            wp.hdop = Some(acc as f64);
        }

        segment.points.push(wp);
    }
    write_gpx_with_segments(vec![segment], Some(RAWDATA_TYPE_NAME), writer)
}
