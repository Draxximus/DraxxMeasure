use crate::*;
use draxx_core::*;
use draxx_statistics::*;
use plotters::{coord::Shift, prelude::*};
use std::{fmt::Debug, io::Cursor};

type Area<'a> = DrawingArea<BitMapBackend<'a>, Shift>;
const INK: RGBColor = RGBColor(30, 45, 62);
const BLUE: RGBColor = RGBColor(35, 105, 170);
const RED: RGBColor = RGBColor(185, 40, 45);
const GREEN: RGBColor = RGBColor(25, 125, 85);
pub struct PlottersExporter;

fn drawing<T, E: Debug>(value: Result<T, E>) -> Result<T, ReportError> {
    value.map_err(|e| ReportError::Rendering(format!("{e:?}")))
}
fn mm(value: Length) -> f64 {
    value.nanometres() as f64 / 1_000_000.0
}
pub(crate) fn metric(value: &Statistic<f64>) -> String {
    match value {
        Statistic::Available(v) => format!("{v:.4}"),
        Statistic::Unavailable(reason) => format!("N/A ({reason:?})"),
    }
}
fn exact(value: &Statistic<Length>) -> String {
    match value {
        Statistic::Available(v) => v.to_string(),
        Statistic::Unavailable(_) => "N/A".into(),
    }
}
pub(crate) fn date(value: UnixMillis) -> Result<String, ReportError> {
    time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(value.0) * 1_000_000)
        .map_err(|e| ReportError::InvalidConfiguration(e.to_string()))?
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|e| ReportError::InvalidConfiguration(e.to_string()))
}
fn font() -> Result<(), ReportError> {
    plotters::style::register_font(
        "sans-serif",
        FontStyle::Normal,
        include_bytes!("../assets/DejaVuSans.ttf"),
    )
    .map_err(|_| ReportError::Rendering("bundled DejaVu Sans could not be loaded".into()))
}
fn text(area: &Area<'_>, value: &str, x: i32, y: i32, size: u32) -> Result<(), ReportError> {
    drawing(area.draw(&Text::new(
        value,
        (x, y),
        ("sans-serif", size).into_font().color(&INK),
    )))
}
fn bounds(input: &ReportInput<'_>) -> (f64, f64) {
    let edges = input.histogram.edges.as_slice();
    let low = mm(edges[0]);
    let high = mm(edges[edges.len() - 1]);
    let padding = ((high - low) * 0.08).max(0.000001);
    (low - padding, high + padding)
}
fn validate(input: &ReportInput<'_>) -> Result<(), ReportError> {
    input.validate_identity()?;
    let n = input
        .snapshot
        .bundles
        .iter()
        .flat_map(|b| &b.measurements)
        .filter(|m| m.effective().is_some())
        .count() as u64;
    if input.statistics.overall.measurement_count != n
        || input.statistics.bundle_count != input.snapshot.bundles.len() as u64
        || input.histogram.counts.len() + 1 != input.histogram.edges.as_slice().len()
        || input
            .histogram
            .counts
            .iter()
            .try_fold(0_u64, |a, b| a.checked_add(*b))
            != Some(n)
        || input.statistics.bundles.len() != input.snapshot.bundles.len()
    {
        return Err(ReportError::InvalidConfiguration(
            "inconsistent report population/counts".into(),
        ));
    }
    for (bundle, statistics) in input.snapshot.bundles.iter().zip(&input.statistics.bundles) {
        if bundle.bundle.id() != statistics.bundle_id
            || statistics.summary.measurement_count
                != bundle
                    .measurements
                    .iter()
                    .filter(|m| m.effective().is_some())
                    .count() as u64
        {
            return Err(ReportError::SnapshotMismatch);
        }
    }
    for summary in std::iter::once(&input.statistics.overall)
        .chain(input.statistics.bundles.iter().map(|b| &b.summary))
    {
        for statistic in [
            &summary.mean_mm,
            &summary.median_mm,
            &summary.sample_standard_deviation_mm,
            &summary.mean_deviation_mm,
            &summary.out_of_specification_percentage,
        ] {
            if let Statistic::Available(value) = statistic
                && !value.is_finite()
            {
                return Err(ReportError::InvalidConfiguration(
                    "nonfinite calculated report value".into(),
                ));
            }
        }
        if summary
            .below_specification_count
            .checked_add(summary.above_specification_count)
            != Some(summary.out_of_specification_count)
            || summary.out_of_specification_count > summary.measurement_count
        {
            return Err(ReportError::InvalidConfiguration(
                "inconsistent out-of-specification counts".into(),
            ));
        }
    }
    Ok(())
}

impl PngExporter for PlottersExporter {
    fn render(&self, input: &ReportInput<'_>) -> Result<Vec<PngPage>, ReportError> {
        validate(input)?;
        font()?;
        let width = input.configuration.page_width_px.get();
        let height = input.configuration.page_height_px.get();
        if width < 1400 || height < 1800 || u64::from(width) * u64::from(height) > 25_000_000 {
            return Err(ReportError::InvalidConfiguration(
                "PNG pages need >=1400x1800 pixels and <=25 million pixels".into(),
            ));
        }
        let mut pixels = vec![255_u8; width as usize * height as usize * 3];
        {
            let root = BitMapBackend::with_buffer(&mut pixels, (width, height)).into_drawing_area();
            drawing(root.fill(&WHITE))?;
            let (header, body) = root.split_vertically(280);
            text(&header, &input.configuration.title, 28, 36, 32)?;
            let part = input.snapshot.specification.part_number().as_str();
            let part_lines: Vec<String> = part
                .chars()
                .collect::<Vec<_>>()
                .chunks(85)
                .map(|c| c.iter().collect())
                .collect();
            for (line, value) in part_lines.iter().take(2).enumerate() {
                text(
                    &header,
                    &format!(
                        "Part Number{}: {value}",
                        if line == 0 { "" } else { " (continued)" }
                    ),
                    28,
                    76 + line as i32 * 26,
                    24,
                )?;
            }
            if part_lines.len() > 2 {
                text(
                    &header,
                    "Part Number continued in exact CSV/manifest (too long for header)",
                    28,
                    125,
                    18,
                )?;
            }
            let spec = &input.snapshot.specification;
            text(
                &header,
                &format!(
                    "Session {:03} | Item {} | Spec v{} | Nominal {} mm | LSL {} | USL {} | Resolution {} mm",
                    input.snapshot.session.id().get(),
                    input.snapshot.item.id(),
                    spec.version(),
                    spec.nominal(),
                    spec.lsl(),
                    spec.usl(),
                    spec.resolution().length()
                ),
                28,
                145,
                21,
            )?;
            text(
                &header,
                &format!(
                    "Generated {} | Revision {} | Session state {:?} | All bundles combined",
                    date(input.metadata.generated_at)?,
                    input.snapshot.revision,
                    input.snapshot.session.status()
                ),
                28,
                178,
                20,
            )?;
            let summary = &input.statistics.overall;
            text(
                &header,
                &format!(
                    "Bundles {} | Measurements {} | Mean {} mm | Sample SD {} mm",
                    input.statistics.bundle_count,
                    summary.measurement_count,
                    metric(&summary.mean_mm),
                    metric(&summary.sample_standard_deviation_mm)
                ),
                28,
                212,
                21,
            )?;
            text(
                &header,
                &format!(
                    "Min {} mm | Max {} mm | Median {} mm | OOS {} ({}%) | Invalidated {}",
                    exact(&summary.minimum),
                    exact(&summary.maximum),
                    metric(&summary.median_mm),
                    summary.out_of_specification_count,
                    metric(&summary.out_of_specification_percentage),
                    input.statistics.invalidated_count
                ),
                28,
                247,
                21,
            )?;
            let (histogram, body) = body.split_vertically(420);
            histogram_chart(&histogram, input)?;
            let (individuals, body) = body.split_vertically(height as i32 - 280 - 420 - 470);
            individual_chart(&individuals, input)?;
            let (summary, footer) = body.split_vertically(340);
            summary_panel(&summary, input)?;
            text(
                &footer,
                "Observed measurements and descriptive statistics; specification limits are not statistical control limits.",
                28,
                28,
                19,
            )?;
            text(
                &footer,
                "Simple inclusive LSL/USL decisions. Instrument/calibration/uncertainty unknown unless documented in CSV.",
                28,
                54,
                19,
            )?;
            text(
                &footer,
                &format!(
                    "Export {} | {} v{} | Software {} | Report v{} | Effective values; audit history retained in CSV.",
                    input.metadata.export_id,
                    input.statistics.algorithm,
                    input.statistics.algorithm_version,
                    input.metadata.software_version,
                    input.metadata.report_format_version
                ),
                28,
                80,
                17,
            )?;
            if let Some(note) = input.metadata.notes.first() {
                text(
                    &footer,
                    &note.chars().take(140).collect::<String>(),
                    28,
                    106,
                    17,
                )?;
            }
            drawing(root.present())?;
        }
        let image = image::RgbImage::from_raw(width, height, pixels)
            .ok_or_else(|| ReportError::Rendering("invalid RGB buffer".into()))?;
        let mut encoded = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgb8(image)
            .write_to(&mut encoded, image::ImageFormat::Png)
            .map_err(|e| ReportError::Rendering(e.to_string()))?;
        Ok(vec![PngPage {
            page_number: std::num::NonZeroU32::MIN,
            bytes: encoded.into_inner(),
        }])
    }
}

fn histogram_chart(area: &Area<'_>, input: &ReportInput<'_>) -> Result<(), ReportError> {
    let (low, high) = bounds(input);
    let maximum = input
        .histogram
        .counts
        .iter()
        .copied()
        .max()
        .unwrap_or(0)
        .max(1) as f64
        * 1.2;
    let mut chart = drawing(
        ChartBuilder::on(area)
            .caption(
                "Measured-length histogram - entire Part Number/specification",
                ("sans-serif", 24),
            )
            .margin(18)
            .x_label_area_size(45)
            .y_label_area_size(85)
            .build_cartesian_2d(low..high, 0.0..maximum),
    )?;
    drawing(
        chart
            .configure_mesh()
            .x_desc("Measured length (mm)")
            .y_desc("Frequency")
            .axis_desc_style(("sans-serif", 20))
            .label_style(("sans-serif", 18))
            .draw(),
    )?;
    let edges = input.histogram.edges.as_slice();
    drawing(
        chart.draw_series(input.histogram.counts.iter().enumerate().map(|(i, n)| {
            Rectangle::new(
                [(mm(edges[i]), 0.0), (mm(edges[i + 1]), *n as f64)],
                BLUE.mix(0.65).filled(),
            )
        })),
    )?;
    for (value, color, label) in [
        (input.snapshot.specification.nominal(), GREEN, "Nominal"),
        (input.snapshot.specification.lsl(), RED, "LSL"),
        (input.snapshot.specification.usl(), RED, "USL"),
    ] {
        drawing(chart.draw_series(LineSeries::new(
            [(mm(value), 0.0), (mm(value), maximum)],
            color.stroke_width(2),
        )))?
        .label(label)
        .legend(move |(x, y)| PathElement::new(vec![(x, y), (x + 24, y)], color.stroke_width(2)));
    }
    drawing(
        chart
            .configure_series_labels()
            .background_style(WHITE.mix(0.9))
            .border_style(INK)
            .label_font(("sans-serif", 18))
            .draw(),
    )?;
    Ok(())
}
/// Concatenate original capture positions across bundles, retaining invalidated gaps.
struct CapturePositions<'a> {
    last: i64,
    measurements: Vec<(i64, &'a MeasurementHistory)>,
}

fn capture_positions(snapshot: &InspectionSnapshot) -> Result<CapturePositions<'_>, ReportError> {
    let mut offset = 0_i64;
    let mut positions = Vec::new();
    for bundle in &snapshot.bundles {
        for history in &bundle.measurements {
            let position = offset
                .checked_add(history.original().sequence().get())
                .ok_or_else(|| {
                    ReportError::InvalidConfiguration("too many capture positions".into())
                })?;
            positions.push((position, history));
        }
        offset = offset
            .checked_add(
                bundle
                    .measurements
                    .last()
                    .map_or(0, |history| history.original().sequence().get()),
            )
            .ok_or_else(|| {
                ReportError::InvalidConfiguration("too many capture positions".into())
            })?;
    }
    Ok(CapturePositions {
        last: offset.max(1),
        measurements: positions,
    })
}

fn individual_chart(area: &Area<'_>, input: &ReportInput<'_>) -> Result<(), ReportError> {
    let (low, high) = bounds(input);
    let positions = capture_positions(input.snapshot)?;
    let last = positions.last;
    let mut chart = drawing(
        ChartBuilder::on(area)
            .caption(
                "Individual measurements - entire Part Number/specification",
                ("sans-serif", 24),
            )
            .margin(20)
            .x_label_area_size(60)
            .y_label_area_size(110)
            .build_cartesian_2d(0.0..last as f64 + 1.0, low..high),
    )?;
    drawing(
        chart
            .configure_mesh()
            .x_desc("Capture position across all bundles (retracted positions retained)")
            .y_desc("Length (mm)")
            .axis_desc_style(("sans-serif", 20))
            .label_style(("sans-serif", 18))
            .draw(),
    )?;
    for (value, color) in [
        (input.snapshot.specification.nominal(), GREEN),
        (input.snapshot.specification.lsl(), RED),
        (input.snapshot.specification.usl(), RED),
    ] {
        drawing(chart.draw_series(LineSeries::new(
            [(0.0, mm(value)), (last as f64 + 1.0, mm(value))],
            color.stroke_width(1),
        )))?;
    }
    drawing(
        chart.draw_series(
            positions
                .measurements
                .into_iter()
                .filter_map(|(position, history)| {
                    history.effective().map(|(_, value)| {
                        let color = if input.snapshot.specification.classify(value.value)
                            == Conformance::WithinLimits
                        {
                            BLUE
                        } else {
                            RED
                        };
                        Circle::new((position as f64, mm(value.value)), 4, color.filled())
                    })
                }),
        ),
    )?;
    Ok(())
}

fn summary_panel(area: &Area<'_>, input: &ReportInput<'_>) -> Result<(), ReportError> {
    let summary = &input.statistics.overall;
    text(
        area,
        "Part Number summary - all bundles combined",
        28,
        34,
        26,
    )?;
    for (y, line) in [
        (
            85,
            format!(
                "Mean {} mm | Median {} mm | Sample SD {} mm",
                metric(&summary.mean_mm),
                metric(&summary.median_mm),
                metric(&summary.sample_standard_deviation_mm)
            ),
        ),
        (
            130,
            format!(
                "Minimum {} mm | Maximum {} mm | Range {} mm",
                exact(&summary.minimum),
                exact(&summary.maximum),
                exact(&summary.range)
            ),
        ),
        (
            175,
            format!(
                "Signed mean deviation from nominal: {} mm",
                metric(&summary.mean_deviation_mm)
            ),
        ),
        (
            220,
            format!(
                "Below LSL {} | Above USL {} | Total out of specification {} ({}%)",
                summary.below_specification_count,
                summary.above_specification_count,
                summary.out_of_specification_count,
                metric(&summary.out_of_specification_percentage)
            ),
        ),
        (
            265,
            format!(
                "Effective observations {} | Invalidated {} | Source bundles {}",
                summary.measurement_count,
                input.statistics.invalidated_count,
                input.statistics.bundle_count
            ),
        ),
    ] {
        text(area, &line, 28, y, 22)?;
    }
    text(
        area,
        "All effective points included; coincident points may overlap. Exact observations and bundle provenance are in CSV.",
        28,
        310,
        18,
    )?;
    Ok(())
}

pub(crate) fn empty_session(session: SessionId, at: UnixMillis) -> Result<Vec<u8>, ReportError> {
    font()?;
    let mut pixels = vec![255_u8; 1800 * 2000 * 3];
    {
        let area = BitMapBackend::with_buffer(&mut pixels, (1800, 2000)).into_drawing_area();
        text(
            &area,
            "DraxxMeasure - Dimensional Inspection Report",
            40,
            70,
            32,
        )?;
        text(
            &area,
            &format!("Session {session} | Generated {}", date(at)?),
            40,
            130,
            24,
        )?;
        text(
            &area,
            "No Part Numbers or measurements recorded. Statistical results: N/A.",
            40,
            200,
            26,
        )?;
        drawing(area.present())?;
    }
    let image = image::RgbImage::from_raw(1800, 2000, pixels)
        .ok_or_else(|| ReportError::Rendering("invalid RGB buffer".into()))?;
    let mut output = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image)
        .write_to(&mut output, image::ImageFormat::Png)
        .map_err(|e| ReportError::Rendering(e.to_string()))?;
    Ok(output.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combined_capture_positions_preserve_retractions_and_continue_across_bundles() {
        let length = |value| Length::parse_mm(value).unwrap();
        let session = InspectionSession::new(SessionId::new(1).unwrap(), UnixMillis(0));
        let spec = PartSpecification::new(
            SpecificationId::new(1).unwrap(),
            PartNumber::new("SYNTHETIC".into()).unwrap(),
            SpecificationVersion::new(1).unwrap(),
            length("10"),
            length("9"),
            length("11"),
            Resolution::new(length("0.1")).unwrap(),
        )
        .unwrap();
        let item = InspectionItem::new(InspectionItemId::new(1).unwrap(), &session, &spec).unwrap();
        let mut bundles = Vec::new();
        let mut id = 0;
        for (bundle_id, count) in [(1, 3), (2, 1)] {
            let bundle = Bundle::start(
                BundleId::new(bundle_id).unwrap(),
                &session,
                &item,
                Sequence::new(bundle_id).unwrap(),
                "Synthetic bundle".into(),
                UnixMillis(0),
            )
            .unwrap();
            let context = CaptureContext {
                session: &session,
                item: &item,
                bundle: &bundle,
                specification: &spec,
            };
            let mut measurements = Vec::new();
            for sequence in 1..=count {
                id += 1;
                let original = Measurement::new(
                    &context,
                    MeasurementDraft {
                        id: MeasurementId::new(id).unwrap(),
                        sequence: Sequence::new(sequence).unwrap(),
                        input: MeasurementInput::full_length("=10".into()).unwrap(),
                        interpreted: InterpretedMeasurement {
                            value: length("10"),
                            method: InterpretationMethod::FullLength,
                        },
                        captured_at: UnixMillis(0),
                        provenance: MeasurementProvenance::default(),
                    },
                )
                .unwrap();
                let corrections = if id == 2 {
                    vec![
                        MeasurementCorrection::new(
                            CorrectionId::new(1).unwrap(),
                            &original,
                            &spec,
                            None,
                            CorrectionDraft {
                                action: CorrectionAction::Invalidate,
                                reason: "Synthetic invalidation".into(),
                                actor: None,
                                recorded_at: UnixMillis(1),
                            },
                        )
                        .unwrap(),
                    ]
                } else {
                    Vec::new()
                };
                measurements.push(MeasurementHistory::new(original, corrections).unwrap());
            }
            bundles.push(SnapshotBundle {
                bundle,
                measurements,
            });
        }
        let snapshot = InspectionSnapshot {
            revision: SnapshotRevision::new(1).unwrap(),
            session,
            item,
            specification: spec,
            bundles,
        };
        snapshot.validate().unwrap();
        let positions = capture_positions(&snapshot).unwrap();
        assert_eq!(positions.last, 4);
        let effective: Vec<_> = positions
            .measurements
            .into_iter()
            .filter_map(|(position, history)| history.effective().map(|_| position))
            .collect();
        assert_eq!(effective, vec![1, 3, 4]);
    }
}
