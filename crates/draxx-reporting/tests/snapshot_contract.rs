use std::num::NonZeroU32;

use draxx_core::*;
use draxx_reporting::*;
use draxx_statistics::*;

fn empty_fixture() -> (InspectionSnapshot, InspectionStatistics, Histogram) {
    let length = |text| Length::parse_mm(text).unwrap();
    let specification = PartSpecification::new(
        SpecificationId::new(1).unwrap(),
        PartNumber::new("SYNTHETIC".into()).unwrap(),
        SpecificationVersion::new(1).unwrap(),
        length("10"),
        length("9"),
        length("11"),
        Resolution::new(length("0.1")).unwrap(),
    )
    .unwrap();
    let session = InspectionSession::new(SessionId::new(1).unwrap(), UnixMillis(0));
    let item =
        InspectionItem::new(InspectionItemId::new(1).unwrap(), &session, &specification).unwrap();
    let revision = SnapshotRevision::new(1).unwrap();
    let statistics = InspectionStatistics {
        revision,
        item_id: item.id(),
        specification_id: specification.id(),
        unit: Unit::Millimetre,
        algorithm: "synthetic-empty-fixture".into(),
        algorithm_version: "1".into(),
        bundle_count: 0,
        invalidated_count: 0,
        overall: DescriptiveStatistics {
            measurement_count: 0,
            mean_mm: Statistic::Unavailable(UnavailableReason::EmptyDataset),
            median_mm: Statistic::Unavailable(UnavailableReason::EmptyDataset),
            sample_standard_deviation_mm: Statistic::Unavailable(UnavailableReason::EmptyDataset),
            minimum: Statistic::Unavailable(UnavailableReason::EmptyDataset),
            maximum: Statistic::Unavailable(UnavailableReason::EmptyDataset),
            range: Statistic::Unavailable(UnavailableReason::EmptyDataset),
            mean_deviation_mm: Statistic::Unavailable(UnavailableReason::EmptyDataset),
            below_specification_count: 0,
            above_specification_count: 0,
            out_of_specification_count: 0,
            out_of_specification_percentage: Statistic::Unavailable(
                UnavailableReason::EmptyDataset,
            ),
        },
        bundles: vec![],
    };
    let histogram = Histogram {
        revision,
        item_id: item.id(),
        edges: HistogramEdges::new(vec![length("9"), length("11")]).unwrap(),
        counts: vec![0],
    };
    (
        InspectionSnapshot {
            revision,
            session,
            item,
            specification,
            bundles: vec![],
        },
        statistics,
        histogram,
    )
}

#[test]
fn report_requires_one_snapshot_identity_across_all_inputs() {
    let (snapshot, mut statistics, mut histogram) = empty_fixture();
    let metadata = ReportMetadata {
        export_id: "synthetic-test".into(),
        generated_at: UnixMillis(1),
        software_version: "test".into(),
        report_format_version: "1".into(),
        notes: vec![],
    };
    let configuration = ReportConfiguration {
        title: "Synthetic test".into(),
        language: ReportLanguage::English,
        page_width_px: NonZeroU32::new(800).unwrap(),
        page_height_px: NonZeroU32::new(600).unwrap(),
    };
    let validate = |statistics: &InspectionStatistics, histogram: &Histogram| {
        ReportInput {
            snapshot: &snapshot,
            statistics,
            histogram,
            metadata: &metadata,
            configuration: &configuration,
        }
        .validate_identity()
    };
    assert!(validate(&statistics, &histogram).is_ok());
    statistics.revision = SnapshotRevision::new(2).unwrap();
    assert!(matches!(
        validate(&statistics, &histogram),
        Err(ReportError::SnapshotMismatch)
    ));
    statistics.revision = snapshot.revision;
    histogram.item_id = InspectionItemId::new(2).unwrap();
    assert!(matches!(
        validate(&statistics, &histogram),
        Err(ReportError::SnapshotMismatch)
    ));
}
