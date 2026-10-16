use std::collections::BTreeSet;

use crate::{
    Bundle, BundleStatus, CorrectionAction, DomainError, InspectionItem, InspectionSession,
    InterpretedMeasurement, Measurement, MeasurementCorrection, MeasurementInput,
    PartSpecification, SessionStatus, SnapshotRevision,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeasurementStatus {
    Original,
    Corrected,
    Invalidated,
}

/// A complete, ordered correction chain as of the snapshot cutoff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasurementHistory {
    original: Measurement,
    corrections: Vec<MeasurementCorrection>,
}

impl MeasurementHistory {
    pub fn new(
        original: Measurement,
        corrections: Vec<MeasurementCorrection>,
    ) -> Result<Self, DomainError> {
        let mut previous = None;
        let mut ids = BTreeSet::new();
        for correction in &corrections {
            if correction.measurement_id() != original.id() || correction.previous_id() != previous
            {
                return Err(DomainError::InvalidCorrectionChain);
            }
            if !ids.insert(correction.id()) {
                return Err(DomainError::DuplicateIdentity);
            }
            previous = Some(correction.id());
        }
        Ok(Self {
            original,
            corrections,
        })
    }
    pub fn original(&self) -> &Measurement {
        &self.original
    }
    pub fn corrections(&self) -> &[MeasurementCorrection] {
        &self.corrections
    }

    pub fn status(&self) -> MeasurementStatus {
        match self.corrections.last().map(MeasurementCorrection::action) {
            None => MeasurementStatus::Original,
            Some(CorrectionAction::Replace { .. }) => MeasurementStatus::Corrected,
            Some(CorrectionAction::Invalidate) => MeasurementStatus::Invalidated,
        }
    }

    /// Replacement does not change the original identity or capture position.
    pub fn effective(&self) -> Option<(&MeasurementInput, &InterpretedMeasurement)> {
        match self.corrections.last().map(MeasurementCorrection::action) {
            None => Some((self.original.input(), self.original.interpreted())),
            Some(CorrectionAction::Replace { input, interpreted }) => Some((input, interpreted)),
            Some(CorrectionAction::Invalidate) => None,
        }
    }
}

/// Read DTO: adapter must validate ownership, uniqueness, and sequence ordering.
#[derive(Clone, Debug)]
pub struct SnapshotBundle {
    pub bundle: Bundle,
    pub measurements: Vec<MeasurementHistory>,
}

/// One item/specification only: never implicitly pool unlike parts or revisions.
/// Produced from one database read transaction at `revision`; all consumers use it.
/// Public DTOs are not a validated storage deserialization format.
#[derive(Clone, Debug)]
pub struct InspectionSnapshot {
    pub revision: SnapshotRevision,
    pub session: InspectionSession,
    pub item: InspectionItem,
    pub specification: PartSpecification,
    pub bundles: Vec<SnapshotBundle>,
}

impl InspectionSnapshot {
    /// Validate the public read DTO before analysis/export. Chronological order
    /// uses increasing sequence keys, never timestamps or contiguous row counts.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.item.session_id() != self.session.id()
            || self.item.specification_id() != self.specification.id()
            || self.item.part_number() != self.specification.part_number()
        {
            return Err(DomainError::AssociationMismatch);
        }
        let mut bundle_ids = BTreeSet::new();
        let mut measurement_ids = BTreeSet::new();
        let mut correction_ids = BTreeSet::new();
        let mut previous_bundle_sequence = 0;
        for data in &self.bundles {
            let bundle = &data.bundle;
            if bundle.session_id() != self.session.id() || bundle.item_id() != self.item.id() {
                return Err(DomainError::AssociationMismatch);
            }
            if !bundle_ids.insert(bundle.id()) {
                return Err(DomainError::DuplicateIdentity);
            }
            if bundle.sequence().get() <= previous_bundle_sequence {
                return Err(DomainError::InvalidTransition(
                    "snapshot bundles must be in sequence order",
                ));
            }
            previous_bundle_sequence = bundle.sequence().get();
            let capacity = crate::BundleCapacity::new(
                data.measurements
                    .iter()
                    .filter(|m| m.effective().is_some())
                    .count(),
            )?;
            if bundle.status() == BundleStatus::Closed {
                capacity.ensure_can_close()?;
            }
            if self.session.status() == SessionStatus::Closed
                && bundle.status() == BundleStatus::Open
            {
                return Err(DomainError::OpenBundlesRemain);
            }
            let mut previous_measurement_sequence = 0;
            for history in &data.measurements {
                let measurement = history.original();
                if measurement.session_id() != self.session.id()
                    || measurement.item_id() != self.item.id()
                    || measurement.bundle_id() != bundle.id()
                    || measurement.specification_id() != self.specification.id()
                {
                    return Err(DomainError::AssociationMismatch);
                }
                if !measurement_ids.insert(measurement.id()) {
                    return Err(DomainError::DuplicateIdentity);
                }
                if measurement.sequence().get() <= previous_measurement_sequence {
                    return Err(DomainError::InvalidTransition(
                        "snapshot measurements must be in sequence order",
                    ));
                }
                previous_measurement_sequence = measurement.sequence().get();
                for correction in history.corrections() {
                    if !correction_ids.insert(correction.id()) {
                        return Err(DomainError::DuplicateIdentity);
                    }
                }
            }
        }
        Ok(())
    }
}
