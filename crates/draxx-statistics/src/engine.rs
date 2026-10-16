use crate::*;
use draxx_core::{Conformance, PartSpecification};

pub struct DescriptiveEngine;

fn unavailable<T>(reason: UnavailableReason) -> Statistic<T> {
    Statistic::Unavailable(reason)
}
fn sum(values: impl Iterator<Item = f64>) -> f64 {
    let (mut result, mut correction) = (0.0, 0.0);
    for value in values {
        let adjusted = value - correction;
        let next = result + adjusted;
        correction = (next - result) - adjusted;
        result = next;
    }
    result
}
fn summarize(
    values: &[Length],
    spec: &PartSpecification,
) -> Result<DescriptiveStatistics, AnalysisError> {
    let n = values.len();
    let below = values
        .iter()
        .filter(|v| spec.classify(**v) == Conformance::BelowLimit)
        .count() as u64;
    let above = values
        .iter()
        .filter(|v| spec.classify(**v) == Conformance::AboveLimit)
        .count() as u64;
    let mut result = DescriptiveStatistics {
        measurement_count: n as u64,
        mean_mm: unavailable(UnavailableReason::EmptyDataset),
        median_mm: unavailable(UnavailableReason::EmptyDataset),
        sample_standard_deviation_mm: unavailable(UnavailableReason::EmptyDataset),
        minimum: unavailable(UnavailableReason::EmptyDataset),
        maximum: unavailable(UnavailableReason::EmptyDataset),
        range: unavailable(UnavailableReason::EmptyDataset),
        mean_deviation_mm: unavailable(UnavailableReason::EmptyDataset),
        below_specification_count: below,
        above_specification_count: above,
        out_of_specification_count: below + above,
        out_of_specification_percentage: unavailable(UnavailableReason::EmptyDataset),
    };
    if n == 0 {
        return Ok(result);
    }
    // Reject unsupported descriptive precision rather than silently losing nm at huge scales.
    if spec.nominal().nanometres() > 1_i64 << 53
        || values.iter().any(|v| v.nanometres() > 1_i64 << 53)
    {
        return Err(AnalysisError::NumericOverflow);
    }
    let origin = values[0].nanometres();
    let centered = |v: &Length| (v.nanometres() - origin) as f64 / 1_000_000.0;
    let mean_offset = sum(values.iter().map(centered)) / n as f64;
    let mean = origin as f64 / 1_000_000.0 + mean_offset;
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let median = if n % 2 == 1 {
        sorted[n / 2].nanometres() as f64 / 1_000_000.0
    } else {
        (i128::from(sorted[n / 2 - 1].nanometres()) + i128::from(sorted[n / 2].nanometres())) as f64
            / 2_000_000.0
    };
    let min = sorted[0];
    let max = sorted[n - 1];
    result.mean_mm = Statistic::Available(mean);
    result.median_mm = Statistic::Available(median);
    result.minimum = Statistic::Available(min);
    result.maximum = Statistic::Available(max);
    result.range = Statistic::Available(
        Length::from_nanometres(max.nanometres() - min.nanometres())
            .map_err(AnalysisError::InvalidDataset)?,
    );
    result.mean_deviation_mm = Statistic::Available(
        (origin - spec.nominal().nanometres()) as f64 / 1_000_000.0 + mean_offset,
    );
    result.out_of_specification_percentage =
        Statistic::Available(100.0 * (below + above) as f64 / n as f64);
    result.sample_standard_deviation_mm = if n < 2 {
        unavailable(UnavailableReason::InsufficientSample)
    } else {
        let squares = sum(values.iter().map(|v| {
            let d = centered(v) - mean_offset;
            d * d
        }));
        Statistic::Available((squares / (n - 1) as f64).sqrt())
    };
    Ok(result)
}

impl StatisticsEngine for DescriptiveEngine {
    fn analyze(
        &self,
        snapshot: &InspectionSnapshot,
    ) -> Result<InspectionStatistics, AnalysisError> {
        snapshot.validate().map_err(AnalysisError::InvalidDataset)?;
        let mut all = Vec::new();
        let mut bundles = Vec::new();
        let mut invalidated = 0;
        for bundle in &snapshot.bundles {
            let values: Vec<Length> = bundle
                .measurements
                .iter()
                .filter_map(|m| m.effective().map(|(_, v)| v.value))
                .collect();
            invalidated += bundle.measurements.len() - values.len();
            bundles.push(BundleStatistics {
                bundle_id: bundle.bundle.id(),
                summary: summarize(&values, &snapshot.specification)?,
            });
            all.extend(values);
        }
        Ok(InspectionStatistics {
            revision: snapshot.revision,
            item_id: snapshot.item.id(),
            specification_id: snapshot.specification.id(),
            unit: snapshot.specification.unit(),
            algorithm: "centered-compensated-two-pass".into(),
            algorithm_version: "1".into(),
            bundle_count: bundles.len() as u64,
            invalidated_count: invalidated as u64,
            overall: summarize(&all, &snapshot.specification)?,
            bundles,
        })
    }
    fn histogram(
        &self,
        snapshot: &InspectionSnapshot,
        edges: HistogramEdges,
    ) -> Result<Histogram, AnalysisError> {
        snapshot.validate().map_err(AnalysisError::InvalidDataset)?;
        let boundaries = edges.as_slice();
        let mut counts = vec![0; boundaries.len() - 1];
        for (_, value) in snapshot
            .bundles
            .iter()
            .flat_map(|b| &b.measurements)
            .filter_map(|m| m.effective())
        {
            if value.value < boundaries[0] || value.value > boundaries[boundaries.len() - 1] {
                return Err(AnalysisError::HistogramDoesNotCoverData);
            }
            let index =
                (boundaries.partition_point(|edge| *edge <= value.value) - 1).min(counts.len() - 1);
            counts[index] += 1;
        }
        Ok(Histogram {
            revision: snapshot.revision,
            item_id: snapshot.item.id(),
            edges,
            counts,
        })
    }
}

/// Up to 20 equal-width integer-nm bins covering observations AND specification limits.
pub fn automatic_histogram_edges(
    snapshot: &InspectionSnapshot,
) -> Result<HistogramEdges, AnalysisError> {
    snapshot.validate().map_err(AnalysisError::InvalidDataset)?;
    let (mut low, mut high) = (
        snapshot.specification.lsl().nanometres(),
        snapshot.specification.usl().nanometres(),
    );
    for (_, v) in snapshot
        .bundles
        .iter()
        .flat_map(|b| &b.measurements)
        .filter_map(|m| m.effective())
    {
        low = low.min(v.value.nanometres());
        high = high.max(v.value.nanometres());
    }
    let span = i128::from(high) - i128::from(low);
    let bins = span.min(20);
    let mut edges = Vec::new();
    for index in 0..=bins {
        let nm = i128::from(low) + span * index / bins;
        edges.push(
            Length::from_nanometres(i64::try_from(nm).map_err(|_| AnalysisError::NumericOverflow)?)
                .map_err(AnalysisError::InvalidDataset)?,
        );
    }
    HistogramEdges::new(edges)
}

#[cfg(test)]
mod tests {
    use super::*;
    use draxx_core::*;
    fn spec(offset: i64) -> PartSpecification {
        let l = |v| Length::from_nanometres((offset + v) * 1_000_000).unwrap();
        PartSpecification::new(
            SpecificationId::new(1).unwrap(),
            PartNumber::new("SYNTHETIC".into()).unwrap(),
            SpecificationVersion::new(1).unwrap(),
            l(3),
            l(2),
            l(4),
            Resolution::new(Length::from_nanometres(1_000_000).unwrap()).unwrap(),
        )
        .unwrap()
    }
    #[test]
    fn reference_statistics_and_large_offset_variance() {
        for offset in [0, 1_000_000_000] {
            let values: Vec<_> = (1..=5)
                .map(|v| Length::from_nanometres((offset + v) * 1_000_000).unwrap())
                .collect();
            let result = summarize(&values, &spec(offset)).unwrap();
            assert_eq!(result.mean_mm, Statistic::Available((offset + 3) as f64));
            assert_eq!(result.median_mm, Statistic::Available((offset + 3) as f64));
            let Statistic::Available(sd) = result.sample_standard_deviation_mm else {
                panic!("missing SD")
            };
            assert!((sd - 1.5811388300841898).abs() < 1e-12);
            assert_eq!(result.out_of_specification_count, 2);
            assert_eq!(
                result.out_of_specification_percentage,
                Statistic::Available(40.0)
            );
        }
    }
    #[test]
    fn empty_singleton_constant_and_even_median() {
        let spec = spec(0);
        let value = Length::parse_mm("3").unwrap();
        assert_eq!(
            summarize(&[], &spec).unwrap().mean_mm,
            Statistic::Unavailable(UnavailableReason::EmptyDataset)
        );
        assert_eq!(
            summarize(&[value], &spec)
                .unwrap()
                .sample_standard_deviation_mm,
            Statistic::Unavailable(UnavailableReason::InsufficientSample)
        );
        assert_eq!(
            summarize(&[value, value], &spec)
                .unwrap()
                .sample_standard_deviation_mm,
            Statistic::Available(0.0)
        );
        assert_eq!(
            summarize(&[value, Length::parse_mm("4").unwrap()], &spec)
                .unwrap()
                .median_mm,
            Statistic::Available(3.5)
        );
    }

    #[test]
    fn unequal_bundle_populations_use_weighted_overall_mean_and_exact_histogram_edges() {
        use draxx_core::*;
        let specification = spec(0);
        let mut session = InspectionSession::new(SessionId::new(1).unwrap(), UnixMillis(0));
        let item = InspectionItem::new(InspectionItemId::new(1).unwrap(), &session, &specification)
            .unwrap();
        let mut bundles = Vec::new();
        let mut id = 0;
        for (index, values) in [vec![1, 2], vec![3, 4, 5]].into_iter().enumerate() {
            let mut bundle = Bundle::start(
                BundleId::new(index as i64 + 1).unwrap(),
                &session,
                &item,
                Sequence::new(index as i64 + 1).unwrap(),
                "Synthetic bundle".into(),
                UnixMillis(1),
            )
            .unwrap();
            let context = CaptureContext {
                session: &session,
                item: &item,
                bundle: &bundle,
                specification: &specification,
            };
            let mut measurements = Vec::new();
            for (sequence, value) in values.iter().enumerate() {
                id += 1;
                let value = Length::parse_mm(&value.to_string()).unwrap();
                let measurement = Measurement::new(
                    &context,
                    MeasurementDraft {
                        id: MeasurementId::new(id).unwrap(),
                        sequence: Sequence::new(sequence as i64 + 1).unwrap(),
                        input: MeasurementInput::full_length(format!("={value}")).unwrap(),
                        interpreted: InterpretedMeasurement {
                            value,
                            method: InterpretationMethod::FullLength,
                        },
                        captured_at: UnixMillis(2),
                        provenance: MeasurementProvenance::default(),
                    },
                )
                .unwrap();
                measurements.push(MeasurementHistory::new(measurement, vec![]).unwrap());
            }
            bundle
                .close(
                    &session,
                    UnixMillis(3),
                    BundleCapacity::new(values.len()).unwrap(),
                )
                .unwrap();
            bundles.push(SnapshotBundle {
                bundle,
                measurements,
            });
        }
        session
            .transition(SessionTransition::Close, 0, UnixMillis(4))
            .unwrap();
        let snapshot = InspectionSnapshot {
            revision: SnapshotRevision::new(1).unwrap(),
            session,
            item,
            specification,
            bundles,
        };
        let result = DescriptiveEngine.analyze(&snapshot).unwrap();
        assert_eq!(result.overall.mean_mm, Statistic::Available(3.0));
        assert_eq!(result.bundles[0].summary.mean_mm, Statistic::Available(1.5));
        assert_eq!(result.bundles[1].summary.mean_mm, Statistic::Available(4.0));
        let edges = HistogramEdges::new(
            ["1", "3", "5"]
                .map(|v| Length::parse_mm(v).unwrap())
                .to_vec(),
        )
        .unwrap();
        assert_eq!(
            DescriptiveEngine
                .histogram(&snapshot, edges)
                .unwrap()
                .counts,
            vec![2, 3]
        );
        let narrow =
            HistogramEdges::new(["2", "4"].map(|v| Length::parse_mm(v).unwrap()).to_vec()).unwrap();
        assert!(matches!(
            DescriptiveEngine.histogram(&snapshot, narrow),
            Err(AnalysisError::HistogramDoesNotCoverData)
        ));
    }
}
