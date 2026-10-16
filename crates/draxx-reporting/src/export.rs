use crate::{render::date, *};
use draxx_core::*;
use draxx_statistics::*;
use serde_json::{Value, json};
#[cfg(unix)]
use std::fs::File;
use std::{
    fs,
    io::Write,
    num::NonZeroU32,
    path::{Path, PathBuf},
};

pub struct VerificationCsv;

fn csv_row(writer: &mut dyn Write, cells: &[String]) -> Result<(), ReportError> {
    for (index, cell) in cells.iter().enumerate() {
        if index != 0 {
            writer.write_all(b",").map_err(ReportError::Io)?;
        }
        write!(writer, "\"{}\"", cell.replace('"', "\"\"")).map_err(ReportError::Io)?;
    }
    writer.write_all(b"\r\n").map_err(ReportError::Io)
}
struct CsvEvent<'a> {
    kind: &'a str,
    status: &'a str,
    id: String,
    previous: String,
    at: UnixMillis,
    value: Option<(&'a MeasurementInput, &'a InterpretedMeasurement)>,
    reason: &'a str,
    actor: Option<&'a str>,
}
fn event_row(
    writer: &mut dyn Write,
    base: &[String],
    event: CsvEvent<'_>,
    spec: &PartSpecification,
    provenance: &MeasurementProvenance,
) -> Result<(), ReportError> {
    let mut cells = base.to_vec();
    cells[0] = event.kind.into();
    cells.extend([
        event.status.into(),
        event.id,
        event.previous,
        date(event.at)?,
    ]);
    let (raw, value, policy_name, policy_version, lower, upper, selected, classification) =
        if let Some((input, value)) = event.value {
            let (name, version, lower, upper, selected) = match &value.method {
                InterpretationMethod::FullLength => (
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    "false".to_string(),
                ),
                InterpretationMethod::IntegerMillimetreSuffix {
                    policy,
                    operator_selected,
                } => (
                    policy.name().into(),
                    policy.version().to_string(),
                    policy.lower().to_string(),
                    policy.upper().to_string(),
                    operator_selected.to_string(),
                ),
            };
            (
                input.raw().into(),
                value.value.to_string(),
                name,
                version,
                lower,
                upper,
                selected,
                format!("{:?}", spec.classify(value.value)),
            )
        } else {
            (
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            )
        };
    cells.extend([
        raw,
        value,
        "mm".into(),
        policy_name,
        policy_version,
        lower,
        upper,
        selected,
        event.reason.into(),
        event.actor.unwrap_or("").into(),
        provenance.instrument_id.clone().unwrap_or_default(),
        provenance.calibration_reference.clone().unwrap_or_default(),
        provenance.operator.clone().unwrap_or_default(),
        classification,
    ]);
    csv_row(writer, &cells)
}
impl CsvExporter for VerificationCsv {
    fn write(&self, input: &ReportInput<'_>, writer: &mut dyn Write) -> Result<(), ReportError> {
        input.validate_identity()?;
        csv_row(
            writer,
            &[
                "row_kind",
                "export_id",
                "snapshot_revision",
                "session_id",
                "item_id",
                "part_number",
                "specification_id",
                "specification_version",
                "nominal_mm",
                "lsl_mm",
                "usl_mm",
                "resolution_mm",
                "bundle_id",
                "bundle_sequence",
                "measurement_id",
                "measurement_sequence",
                "original_capture_time_utc",
                "status",
                "event_id",
                "previous_event_id",
                "event_time_utc",
                "raw_input",
                "value_mm",
                "unit",
                "policy_name",
                "policy_version",
                "capture_lower_mm",
                "capture_upper_mm",
                "operator_selected",
                "reason",
                "actor",
                "instrument_id",
                "calibration_reference",
                "instrument_operator",
                "classification",
            ]
            .map(String::from),
        )?;
        let snapshot = input.snapshot;
        let spec = &snapshot.specification;
        for bundle in &snapshot.bundles {
            for history in &bundle.measurements {
                let original = history.original();
                let base = vec![
                    String::new(),
                    input.metadata.export_id.clone(),
                    snapshot.revision.to_string(),
                    snapshot.session.id().to_string(),
                    snapshot.item.id().to_string(),
                    spec.part_number().as_str().into(),
                    spec.id().to_string(),
                    spec.version().to_string(),
                    spec.nominal().to_string(),
                    spec.lsl().to_string(),
                    spec.usl().to_string(),
                    spec.resolution().length().to_string(),
                    bundle.bundle.id().to_string(),
                    bundle.bundle.sequence().to_string(),
                    original.id().to_string(),
                    original.sequence().to_string(),
                    date(original.captured_at())?,
                ];
                event_row(
                    writer,
                    &base,
                    CsvEvent {
                        kind: "original",
                        status: "original-observation",
                        id: String::new(),
                        previous: String::new(),
                        at: original.captured_at(),
                        value: Some((original.input(), original.interpreted())),
                        reason: "",
                        actor: original.provenance().operator.as_deref(),
                    },
                    spec,
                    original.provenance(),
                )?;
                for correction in history.corrections() {
                    let (status, value) = match correction.action() {
                        CorrectionAction::Invalidate => ("invalidate", None),
                        CorrectionAction::Replace { input, interpreted } => {
                            ("replace", Some((input, interpreted)))
                        }
                    };
                    event_row(
                        writer,
                        &base,
                        CsvEvent {
                            kind: "correction",
                            status,
                            id: correction.id().to_string(),
                            previous: correction
                                .previous_id()
                                .map(|id| id.to_string())
                                .unwrap_or_default(),
                            at: correction.recorded_at(),
                            value,
                            reason: correction.reason(),
                            actor: correction.actor(),
                        },
                        spec,
                        original.provenance(),
                    )?;
                }
                let status = match history.status() {
                    MeasurementStatus::Original => "original",
                    MeasurementStatus::Corrected => "corrected",
                    MeasurementStatus::Invalidated => "invalidated",
                };
                let latest = history.corrections().last();
                event_row(
                    writer,
                    &base,
                    CsvEvent {
                        kind: "effective",
                        status,
                        id: latest.map(|e| e.id().to_string()).unwrap_or_default(),
                        previous: latest
                            .and_then(|e| e.previous_id())
                            .map(|id| id.to_string())
                            .unwrap_or_default(),
                        at: latest
                            .map_or(original.captured_at(), MeasurementCorrection::recorded_at),
                        value: history.effective(),
                        reason: latest.map_or("", MeasurementCorrection::reason),
                        actor: latest.and_then(MeasurementCorrection::actor),
                    },
                    spec,
                    original.provenance(),
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct ExportedReport {
    pub png_files: Vec<PathBuf>,
    pub csv_files: Vec<PathBuf>,
    pub manifest: PathBuf,
}

fn statistic(value: &Statistic<f64>) -> Value {
    match value {
        Statistic::Available(value) => json!(value),
        Statistic::Unavailable(reason) => json!({"unavailable":format!("{reason:?}")}),
    }
}
fn publish(dir: &Path, path: &Path, bytes: &[u8]) -> Result<(), ReportError> {
    let mut temporary = tempfile::NamedTempFile::new_in(dir).map_err(ReportError::Io)?;
    temporary.write_all(bytes).map_err(ReportError::Io)?;
    temporary.as_file().sync_all().map_err(ReportError::Io)?;
    temporary
        .persist_noclobber(path)
        .map_err(|e| ReportError::Io(e.error))?;
    Ok(())
}

/// Publish a complete set with a manifest LAST; existing exports are never overwritten.
/// All snapshots must originate from the same read transaction and session.
pub fn export_session(
    session: SessionId,
    snapshots: &[InspectionSnapshot],
    directory: &Path,
    generated_at: UnixMillis,
    notes: &[String],
) -> Result<ExportedReport, ReportError> {
    let reference = snapshots.first().map(|s| s.revision);
    for snapshot in snapshots {
        snapshot.validate().map_err(ReportError::InvalidSnapshot)?;
        if snapshot.session.id() != session
            || Some(snapshot.revision) != reference
            || snapshot.session.status() != SessionStatus::Closed
        {
            return Err(ReportError::SnapshotMismatch);
        }
    }
    fs::create_dir_all(directory).map_err(ReportError::Io)?;
    let export_id = uuid::Uuid::new_v4().to_string();
    let mut stem = format!("session-{:03}", session.get());
    // Includes incomplete crash leftovers: retries do not silently replace them.
    let prefix = format!("{stem}.");
    let page_prefix = format!("{stem}-");
    let exists = fs::read_dir(directory)
        .map_err(ReportError::Io)?
        .collect::<std::io::Result<Vec<_>>>()
        .map_err(ReportError::Io)?
        .into_iter()
        .any(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with(&prefix) || name.starts_with(&page_prefix)
        });
    if exists {
        stem = format!("{stem}-{export_id}");
    }
    let metadata = ReportMetadata {
        export_id: export_id.clone(),
        generated_at,
        software_version: env!("CARGO_PKG_VERSION").into(),
        report_format_version: "1".into(),
        notes: notes.to_vec(),
    };
    let configuration = ReportConfiguration {
        title: "DraxxMeasure - Dimensional Inspection Report".into(),
        language: ReportLanguage::English,
        page_width_px: NonZeroU32::new(1800)
            .ok_or_else(|| ReportError::InvalidConfiguration("width".into()))?,
        page_height_px: NonZeroU32::new(2000)
            .ok_or_else(|| ReportError::InvalidConfiguration("height".into()))?,
    };
    let mut files: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    let mut png_files = Vec::new();
    let mut csv_files = Vec::new();
    let mut items = Vec::new();
    for snapshot in snapshots {
        let statistics = DescriptiveEngine
            .analyze(snapshot)
            .map_err(|e| ReportError::Rendering(e.to_string()))?;
        let histogram = DescriptiveEngine
            .histogram(
                snapshot,
                automatic_histogram_edges(snapshot)
                    .map_err(|e| ReportError::Rendering(e.to_string()))?,
            )
            .map_err(|e| ReportError::Rendering(e.to_string()))?;
        let input = ReportInput {
            snapshot,
            statistics: &statistics,
            histogram: &histogram,
            metadata: &metadata,
            configuration: &configuration,
        };
        for page in PlottersExporter.render(&input)? {
            let page_number = png_files.len() + 1;
            let name = if page_number == 1 {
                format!("{stem}.png")
            } else {
                format!("{stem}-page-{page_number:03}.png")
            };
            let path = directory.join(name);
            png_files.push(path.clone());
            files.push((path, page.bytes));
        }
        let mut csv = Vec::new();
        VerificationCsv.write(&input, &mut csv)?;
        let path = directory.join(format!("{stem}-item-{:03}.csv", snapshot.item.id().get()));
        csv_files.push(path.clone());
        files.push((path, csv));
        let summary = &statistics.overall;
        items.push(json!({"item_id":snapshot.item.id().get(),"part_number":snapshot.specification.part_number().as_str(),"specification_id":snapshot.specification.id().get(),"specification_version":snapshot.specification.version().get(),"nominal_mm":snapshot.specification.nominal().to_string(),"lsl_mm":snapshot.specification.lsl().to_string(),"usl_mm":snapshot.specification.usl().to_string(),"resolution_mm":snapshot.specification.resolution().length().to_string(),"bundle_count":statistics.bundle_count,"measurement_count":summary.measurement_count,"invalidated_count":statistics.invalidated_count,"mean_mm":statistic(&summary.mean_mm),"median_mm":statistic(&summary.median_mm),"sample_standard_deviation_mm":statistic(&summary.sample_standard_deviation_mm),"out_of_specification_count":summary.out_of_specification_count,"out_of_specification_percentage":statistic(&summary.out_of_specification_percentage),"algorithm":statistics.algorithm,"algorithm_version":statistics.algorithm_version,"histogram_edges_nm":histogram.edges.as_slice().iter().map(|l|l.nanometres()).collect::<Vec<_>>(),"histogram_counts":histogram.counts,"capture_start_utc":date(snapshot.session.created_at())?,"session_closed_utc":snapshot.session.closed_at().map(date).transpose()?}));
    }
    if snapshots.is_empty() {
        let path = directory.join(format!("{stem}.png"));
        let bytes = crate::render::empty_session(session, generated_at)?;
        png_files.push(path.clone());
        files.push((path, bytes));
    }
    let manifest = directory.join(format!("{stem}.manifest.json"));
    let manifest_data=serde_json::to_vec_pretty(&json!({"format_version":1,"complete":true,"export_id":export_id,"session_id":session.get(),"revision":reference.map(|r|r.get()),"generated_at_utc":date(generated_at)?,"notes":notes,"unit":"mm","items":items,"files":files.iter().map(|(p,b)|json!({"name":p.file_name().map(|v|v.to_string_lossy()),"bytes":b.len()})).collect::<Vec<_>>(),"verification_filter":"row_kind=effective AND value_mm is nonempty","csv_dialect":"UTF-8, quoted comma-delimited, CRLF, ASCII decimal point, blank=unavailable","spreadsheet_warning":"Canonical CSV preserves operator text including '='; import every column as text, do not open as trusted formulas."})).map_err(|e|ReportError::Rendering(e.to_string()))?;
    let mut published = Vec::new();
    let result = (|| {
        for (path, bytes) in &files {
            publish(directory, path, bytes)?;
            published.push(path.clone());
        }
        publish(directory, &manifest, &manifest_data)?;
        published.push(manifest.clone());
        #[cfg(unix)]
        File::open(directory)
            .and_then(|file| file.sync_all())
            .map_err(ReportError::Io)?;
        Ok(ExportedReport {
            png_files,
            csv_files,
            manifest,
        })
    })();
    if result.is_err() {
        for path in published {
            let _ = fs::remove_file(path);
        }
    }
    result
}
