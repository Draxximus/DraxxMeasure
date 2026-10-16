//! Headless English-language PNG/CSV reporting and safe export publication.

use std::{error::Error, fmt, io, num::NonZeroU32};

use draxx_core::{DomainError, InspectionSnapshot, UnixMillis};
use draxx_statistics::{Histogram, InspectionStatistics};

mod export;
mod render;
pub use export::{ExportedReport, VerificationCsv, export_session};
pub use render::PlottersExporter;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReportLanguage {
    English,
}

#[derive(Clone, Debug)]
pub struct ReportConfiguration {
    pub title: String,
    pub language: ReportLanguage,
    pub page_width_px: NonZeroU32,
    pub page_height_px: NonZeroU32,
}

#[derive(Clone, Debug)]
pub struct ReportMetadata {
    pub export_id: String,
    pub generated_at: UnixMillis,
    pub software_version: String,
    pub report_format_version: String,
    /// Explicitly describe inclusions, missing metadata, and any interpretation.
    pub notes: Vec<String>,
}

/// Snapshot supplies raw/effective ordered observations, bundle identities,
/// specification lines, and audit history. Statistics supplies bundle means/ranges.
/// Histogram supplies reproducible edges/counts; exporters never recalculate them.
pub struct ReportInput<'a> {
    pub snapshot: &'a InspectionSnapshot,
    pub statistics: &'a InspectionStatistics,
    pub histogram: &'a Histogram,
    pub metadata: &'a ReportMetadata,
    pub configuration: &'a ReportConfiguration,
}

impl ReportInput<'_> {
    /// Structural identity check, not a replacement for validated calculations.
    pub fn validate_identity(&self) -> Result<(), ReportError> {
        let snapshot = self.snapshot;
        snapshot.validate().map_err(ReportError::InvalidSnapshot)?;
        if self.statistics.revision != snapshot.revision
            || self.histogram.revision != snapshot.revision
            || self.statistics.item_id != snapshot.item.id()
            || self.histogram.item_id != snapshot.item.id()
            || self.statistics.specification_id != snapshot.specification.id()
            || self.statistics.unit != snapshot.specification.unit()
        {
            return Err(ReportError::SnapshotMismatch);
        }
        Ok(())
    }
}

/// Numbered pages share one export/snapshot ID. No undisclosed downsampling.
#[derive(Clone, Debug)]
pub struct PngPage {
    pub page_number: NonZeroU32,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub enum ReportError {
    InvalidSnapshot(DomainError),
    SnapshotMismatch,
    InvalidConfiguration(String),
    Rendering(String),
    Io(io::Error),
}

impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSnapshot(error) => write!(f, "invalid report snapshot: {error}"),
            Self::SnapshotMismatch => {
                f.write_str("report inputs refer to different inspection snapshots")
            }
            Self::InvalidConfiguration(message) => {
                write!(f, "invalid report configuration: {message}")
            }
            Self::Rendering(message) => write!(f, "report rendering failed: {message}"),
            Self::Io(error) => write!(f, "report export failed: {error}"),
        }
    }
}

impl Error for ReportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::InvalidSnapshot(error) => Some(error),
            _ => None,
        }
    }
}

pub trait PngExporter {
    /// Validate identity; include histogram, individuals by bundle, and bundle
    /// means/min-max ranges with nominal/LSL/USL and English metadata.
    fn render(&self, input: &ReportInput<'_>) -> Result<Vec<PngPage>, ReportError>;
}

pub trait CsvExporter {
    /// UTF-8 exact verification rows plus linked audit records, never sanitized
    /// silently. Writer can contain partial output on error: caller uses temp files.
    fn write(&self, input: &ReportInput<'_>, writer: &mut dyn io::Write)
    -> Result<(), ReportError>;
}
