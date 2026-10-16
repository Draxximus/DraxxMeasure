//! Reproducible descriptive analysis over exact, traceable inspection snapshots.
//!
//! Use effective measurements, include valid out-of-spec values, and count empty
//! bundles. Never pool different inspection items/specification revisions.

use std::{error::Error, fmt};

use draxx_core::{
    BundleId, DomainError, InspectionItemId, InspectionSnapshot, Length, SnapshotRevision,
    SpecificationId, Unit,
};

mod engine;
pub use engine::{DescriptiveEngine, automatic_histogram_edges};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UnavailableReason {
    EmptyDataset,
    InsufficientSample,
    ZeroVariation,
    AssumptionsNotMet,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Statistic<T> {
    Available(T),
    Unavailable(UnavailableReason),
}

/// Floating outputs are descriptive values in millimetres, never classification inputs.
#[derive(Clone, Debug, PartialEq)]
pub struct DescriptiveStatistics {
    pub measurement_count: u64,
    pub mean_mm: Statistic<f64>,
    pub median_mm: Statistic<f64>,
    pub sample_standard_deviation_mm: Statistic<f64>,
    pub minimum: Statistic<Length>,
    pub maximum: Statistic<Length>,
    pub range: Statistic<Length>,
    pub mean_deviation_mm: Statistic<f64>,
    pub below_specification_count: u64,
    pub above_specification_count: u64,
    pub out_of_specification_count: u64,
    /// 0..=100, unavailable when the effective count is zero.
    pub out_of_specification_percentage: Statistic<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BundleStatistics {
    pub bundle_id: BundleId,
    pub summary: DescriptiveStatistics,
}

#[derive(Clone, Debug, PartialEq)]
pub struct InspectionStatistics {
    pub revision: SnapshotRevision,
    pub item_id: InspectionItemId,
    pub specification_id: SpecificationId,
    pub unit: Unit,
    pub algorithm: String,
    pub algorithm_version: String,
    pub bundle_count: u64,
    pub invalidated_count: u64,
    pub overall: DescriptiveStatistics,
    pub bundles: Vec<BundleStatistics>,
}

/// Left-closed/right-open bins, except the final right edge is included.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HistogramEdges(Vec<Length>);

impl HistogramEdges {
    pub fn new(edges: Vec<Length>) -> Result<Self, AnalysisError> {
        if edges.len() < 2 || edges.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(AnalysisError::InvalidHistogramEdges);
        }
        Ok(Self(edges))
    }
    pub fn as_slice(&self) -> &[Length] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Histogram {
    pub revision: SnapshotRevision,
    pub item_id: InspectionItemId,
    pub edges: HistogramEdges,
    /// Exactly edges.len()-1 counts, summing to effective measurement count.
    pub counts: Vec<u64>,
}

#[derive(Debug)]
pub enum AnalysisError {
    InvalidDataset(DomainError),
    InvalidHistogramEdges,
    HistogramDoesNotCoverData,
    NumericOverflow,
}

impl fmt::Display for AnalysisError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDataset(error) => write!(f, "invalid analysis snapshot: {error}"),
            Self::InvalidHistogramEdges => {
                f.write_str("histogram needs at least two strictly increasing edges")
            }
            Self::HistogramDoesNotCoverData => {
                f.write_str("histogram must cover every effective measurement")
            }
            Self::NumericOverflow => {
                f.write_str("statistical result exceeds the supported numerical range")
            }
        }
    }
}

impl Error for AnalysisError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidDataset(error) => Some(error),
            _ => None,
        }
    }
}

pub trait StatisticsEngine {
    /// Validate snapshot ownership/order, calculate sample SD (n-1), and use exact
    /// inclusive LSL/USL comparisons. Undefined metrics need explicit reasons.
    fn analyze(&self, snapshot: &InspectionSnapshot)
    -> Result<InspectionStatistics, AnalysisError>;
    fn histogram(
        &self,
        snapshot: &InspectionSnapshot,
        edges: HistogramEdges,
    ) -> Result<Histogram, AnalysisError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_edges_reject_empty_duplicate_and_descending_bounds() {
        let zero = Length::from_nanometres(0).unwrap();
        let one = Length::from_nanometres(1).unwrap();
        for edges in [vec![], vec![zero], vec![zero, zero], vec![one, zero]] {
            assert!(HistogramEdges::new(edges).is_err());
        }
        assert_eq!(
            HistogramEdges::new(vec![zero, one]).unwrap().as_slice(),
            &[zero, one]
        );
    }
}
