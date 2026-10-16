use crate::{
    BundleId, CorrectionId, DomainError, InspectionItemId, InterpretedMeasurement, Length,
    MeasurementId, MeasurementInput, Resolution, Sequence, SessionId, SpecificationId,
    SpecificationVersion, Unit, UnixMillis,
};

/// Business identifier, not a number: leading zeroes and exact spelling survive.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct PartNumber(String);

impl PartNumber {
    pub fn new(value: String) -> Result<Self, DomainError> {
        if value.trim().is_empty() {
            return Err(DomainError::EmptyField("Part Number"));
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Immutable once referenced; a specification change creates a new ID/version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartSpecification {
    id: SpecificationId,
    part_number: PartNumber,
    version: SpecificationVersion,
    nominal: Length,
    lsl: Length,
    usl: Length,
    resolution: Resolution,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Conformance {
    BelowLimit,
    WithinLimits,
    AboveLimit,
}

impl PartSpecification {
    pub fn symmetric_limits(
        nominal: Length,
        tolerance: Length,
    ) -> Result<(Length, Length), DomainError> {
        if tolerance.nanometres() == 0 {
            return Err(DomainError::InvalidSpecification);
        }
        let lower = nominal
            .nanometres()
            .checked_sub(tolerance.nanometres())
            .ok_or(DomainError::NumericOverflow)?;
        let upper = nominal
            .nanometres()
            .checked_add(tolerance.nanometres())
            .ok_or(DomainError::NumericOverflow)?;
        Ok((
            Length::from_nanometres(lower)?,
            Length::from_nanometres(upper)?,
        ))
    }
    pub fn new(
        id: SpecificationId,
        part_number: PartNumber,
        version: SpecificationVersion,
        nominal: Length,
        lsl: Length,
        usl: Length,
        resolution: Resolution,
    ) -> Result<Self, DomainError> {
        if lsl >= usl || nominal < lsl || nominal > usl {
            return Err(DomainError::InvalidSpecification);
        }
        Ok(Self {
            id,
            part_number,
            version,
            nominal,
            lsl,
            usl,
            resolution,
        })
    }
    pub const fn id(&self) -> SpecificationId {
        self.id
    }
    pub fn part_number(&self) -> &PartNumber {
        &self.part_number
    }
    pub const fn version(&self) -> SpecificationVersion {
        self.version
    }
    pub const fn nominal(&self) -> Length {
        self.nominal
    }
    pub const fn lsl(&self) -> Length {
        self.lsl
    }
    pub const fn usl(&self) -> Length {
        self.usl
    }
    pub const fn resolution(&self) -> Resolution {
        self.resolution
    }
    pub const fn unit(&self) -> Unit {
        Unit::Millimetre
    }

    /// Simple inclusive specification comparison; no uncertainty guard band implied.
    pub fn classify(&self, value: Length) -> Conformance {
        if value < self.lsl {
            Conformance::BelowLimit
        } else if value > self.usl {
            Conformance::AboveLimit
        } else {
            Conformance::WithinLimits
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionStatus {
    Active,
    Suspended,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionTransition {
    Suspend,
    Resume,
    Close,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectionSession {
    id: SessionId,
    status: SessionStatus,
    created_at: UnixMillis,
    closed_at: Option<UnixMillis>,
}

impl InspectionSession {
    pub const fn new(id: SessionId, created_at: UnixMillis) -> Self {
        Self {
            id,
            status: SessionStatus::Active,
            created_at,
            closed_at: None,
        }
    }
    pub const fn id(&self) -> SessionId {
        self.id
    }
    pub const fn status(&self) -> SessionStatus {
        self.status
    }
    pub const fn created_at(&self) -> UnixMillis {
        self.created_at
    }
    pub const fn closed_at(&self) -> Option<UnixMillis> {
        self.closed_at
    }

    /// Rehydrate persisted state without pretending to perform new transitions.
    pub fn restore(
        id: SessionId,
        status: SessionStatus,
        created_at: UnixMillis,
        closed_at: Option<UnixMillis>,
    ) -> Result<Self, DomainError> {
        if (status == SessionStatus::Closed) != closed_at.is_some() {
            return Err(DomainError::InvalidTransition(
                "closed session and closure timestamp must agree",
            ));
        }
        Ok(Self {
            id,
            status,
            created_at,
            closed_at,
        })
    }

    /// The storage adapter must obtain the authoritative open count in the same transaction.
    pub fn transition(
        &mut self,
        event: SessionTransition,
        open_bundle_count: usize,
        at: UnixMillis,
    ) -> Result<(), DomainError> {
        let next = match (self.status, event) {
            (SessionStatus::Active, SessionTransition::Suspend) => SessionStatus::Suspended,
            (SessionStatus::Suspended, SessionTransition::Resume) => SessionStatus::Active,
            (SessionStatus::Active, SessionTransition::Close) => {
                if open_bundle_count != 0 {
                    return Err(DomainError::OpenBundlesRemain);
                }
                SessionStatus::Closed
            }
            _ => {
                return Err(DomainError::InvalidTransition(
                    "session transition is not allowed",
                ));
            }
        };
        self.status = next;
        if next == SessionStatus::Closed {
            self.closed_at = Some(at);
        }
        Ok(())
    }
}

/// One specification snapshot for a Part Number within a session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectionItem {
    id: InspectionItemId,
    session_id: SessionId,
    specification_id: SpecificationId,
    part_number: PartNumber,
}

impl InspectionItem {
    pub fn new(
        id: InspectionItemId,
        session: &InspectionSession,
        specification: &PartSpecification,
    ) -> Result<Self, DomainError> {
        if session.status() != SessionStatus::Active {
            return Err(DomainError::InvalidTransition(
                "add items only to active sessions",
            ));
        }
        Ok(Self::restore(id, session, specification))
    }

    /// Existing items remain valid when the owning session is no longer Active.
    pub fn restore(
        id: InspectionItemId,
        session: &InspectionSession,
        specification: &PartSpecification,
    ) -> Self {
        Self {
            id,
            session_id: session.id(),
            specification_id: specification.id(),
            part_number: specification.part_number().clone(),
        }
    }
    pub const fn id(&self) -> InspectionItemId {
        self.id
    }
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }
    pub const fn specification_id(&self) -> SpecificationId {
        self.specification_id
    }
    pub fn part_number(&self) -> &PartNumber {
        &self.part_number
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BundleStatus {
    Open,
    Closed,
}

/// Persisted bundle state, validated by Bundle::restore rather than blindly trusted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BundleRecord {
    pub id: BundleId,
    pub sequence: Sequence,
    pub label: String,
    pub status: BundleStatus,
    pub started_at: UnixMillis,
    pub closed_at: Option<UnixMillis>,
}

/// Independently identified group of sequential measurements (one spreadsheet column).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Bundle {
    id: BundleId,
    session_id: SessionId,
    item_id: InspectionItemId,
    sequence: Sequence,
    label: String,
    status: BundleStatus,
    started_at: UnixMillis,
    closed_at: Option<UnixMillis>,
}

impl Bundle {
    pub fn start(
        id: BundleId,
        session: &InspectionSession,
        item: &InspectionItem,
        sequence: Sequence,
        label: String,
        at: UnixMillis,
    ) -> Result<Self, DomainError> {
        if session.id() != item.session_id() {
            return Err(DomainError::AssociationMismatch);
        }
        if session.status() != SessionStatus::Active {
            return Err(DomainError::InvalidTransition(
                "start bundles only in active sessions",
            ));
        }
        Self::restore(
            BundleRecord {
                id,
                sequence,
                label,
                status: BundleStatus::Open,
                started_at: at,
                closed_at: None,
            },
            session,
            item,
        )
    }

    /// Restore an existing bundle without requiring the session to be Active.
    pub fn restore(
        record: BundleRecord,
        session: &InspectionSession,
        item: &InspectionItem,
    ) -> Result<Self, DomainError> {
        if session.id() != item.session_id() {
            return Err(DomainError::AssociationMismatch);
        }
        if record.label.trim().is_empty() {
            return Err(DomainError::EmptyField("bundle label"));
        }
        if (record.status == BundleStatus::Closed) != record.closed_at.is_some()
            || (record.status == BundleStatus::Open && session.status() == SessionStatus::Closed)
        {
            return Err(DomainError::InvalidTransition(
                "bundle closure state is inconsistent",
            ));
        }
        Ok(Self {
            id: record.id,
            session_id: session.id(),
            item_id: item.id(),
            sequence: record.sequence,
            label: record.label,
            status: record.status,
            started_at: record.started_at,
            closed_at: record.closed_at,
        })
    }
    pub const fn id(&self) -> BundleId {
        self.id
    }
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }
    pub const fn item_id(&self) -> InspectionItemId {
        self.item_id
    }
    pub const fn sequence(&self) -> Sequence {
        self.sequence
    }
    pub fn label(&self) -> &str {
        &self.label
    }
    pub const fn status(&self) -> BundleStatus {
        self.status
    }
    pub const fn started_at(&self) -> UnixMillis {
        self.started_at
    }
    pub const fn closed_at(&self) -> Option<UnixMillis> {
        self.closed_at
    }

    pub fn close(
        &mut self,
        session: &InspectionSession,
        at: UnixMillis,
        capacity: crate::BundleCapacity,
    ) -> Result<(), DomainError> {
        if self.session_id != session.id() {
            return Err(DomainError::AssociationMismatch);
        }
        if session.status() != SessionStatus::Active || self.status != BundleStatus::Open {
            return Err(DomainError::InvalidTransition(
                "close an open bundle in an active session",
            ));
        }
        capacity.ensure_can_close()?;
        self.status = BundleStatus::Closed;
        self.closed_at = Some(at);
        Ok(())
    }
}

/// Unknown instrument/calibration/operator metadata remains None, never fabricated.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MeasurementProvenance {
    pub instrument_id: Option<String>,
    pub calibration_reference: Option<String>,
    pub operator: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasurementDraft {
    pub id: MeasurementId,
    pub sequence: Sequence,
    pub input: MeasurementInput,
    pub interpreted: InterpretedMeasurement,
    pub captured_at: UnixMillis,
    pub provenance: MeasurementProvenance,
}

pub struct CaptureContext<'a> {
    pub session: &'a InspectionSession,
    pub item: &'a InspectionItem,
    pub bundle: &'a Bundle,
    pub specification: &'a PartSpecification,
}

impl CaptureContext<'_> {
    fn validate_associations(&self) -> Result<(), DomainError> {
        if self.session.id() != self.item.session_id()
            || self.session.id() != self.bundle.session_id()
            || self.item.id() != self.bundle.item_id()
            || self.item.specification_id() != self.specification.id()
            || self.item.part_number() != self.specification.part_number()
        {
            return Err(DomainError::AssociationMismatch);
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), DomainError> {
        self.validate_associations()?;
        if self.session.status() != SessionStatus::Active
            || self.bundle.status() != BundleStatus::Open
        {
            return Err(DomainError::InvalidTransition(
                "capture requires an active session and open bundle",
            ));
        }
        Ok(())
    }
}

/// Immutable original observation. Corrections are separate append-only entities.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Measurement {
    session_id: SessionId,
    item_id: InspectionItemId,
    bundle_id: BundleId,
    specification_id: SpecificationId,
    draft: MeasurementDraft,
}

impl Measurement {
    pub fn new(context: &CaptureContext<'_>, draft: MeasurementDraft) -> Result<Self, DomainError> {
        context.validate()?;
        Self::restore(context, draft)
    }

    /// Validate historical values/ownership without reapplying current capture state.
    /// This does not establish that a database committed the observation.
    pub fn restore(
        context: &CaptureContext<'_>,
        draft: MeasurementDraft,
    ) -> Result<Self, DomainError> {
        context.validate_associations()?;
        draft.interpreted.validate(&draft.input)?;
        context
            .specification
            .resolution()
            .validate(draft.interpreted.value)?;
        Ok(Self {
            session_id: context.session.id(),
            item_id: context.item.id(),
            bundle_id: context.bundle.id(),
            specification_id: context.specification.id(),
            draft,
        })
    }
    pub const fn id(&self) -> MeasurementId {
        self.draft.id
    }
    pub const fn session_id(&self) -> SessionId {
        self.session_id
    }
    pub const fn item_id(&self) -> InspectionItemId {
        self.item_id
    }
    pub const fn bundle_id(&self) -> BundleId {
        self.bundle_id
    }
    pub const fn specification_id(&self) -> SpecificationId {
        self.specification_id
    }
    pub const fn sequence(&self) -> Sequence {
        self.draft.sequence
    }
    pub fn input(&self) -> &MeasurementInput {
        &self.draft.input
    }
    pub fn interpreted(&self) -> &InterpretedMeasurement {
        &self.draft.interpreted
    }
    pub const fn unit(&self) -> Unit {
        Unit::Millimetre
    }
    pub const fn captured_at(&self) -> UnixMillis {
        self.draft.captured_at
    }
    pub fn provenance(&self) -> &MeasurementProvenance {
        &self.draft.provenance
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CorrectionAction {
    Replace {
        input: MeasurementInput,
        interpreted: InterpretedMeasurement,
    },
    Invalidate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorrectionDraft {
    pub action: CorrectionAction,
    pub reason: String,
    pub actor: Option<String>,
    pub recorded_at: UnixMillis,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasurementCorrection {
    id: CorrectionId,
    measurement_id: MeasurementId,
    previous_id: Option<CorrectionId>,
    draft: CorrectionDraft,
}

impl MeasurementCorrection {
    /// Repository must also verify `previous` is the latest committed event.
    pub fn new(
        id: CorrectionId,
        target: &Measurement,
        specification: &PartSpecification,
        previous: Option<&Self>,
        draft: CorrectionDraft,
    ) -> Result<Self, DomainError> {
        if specification.id() != target.specification_id() {
            return Err(DomainError::AssociationMismatch);
        }
        if draft.reason.trim().is_empty() {
            return Err(DomainError::EmptyField("correction reason"));
        }
        if let Some(previous) = previous
            && (previous.measurement_id != target.id() || previous.id == id)
        {
            return Err(DomainError::InvalidCorrectionChain);
        }
        if let CorrectionAction::Replace { input, interpreted } = &draft.action {
            interpreted.validate(input)?;
            specification.resolution().validate(interpreted.value)?;
        }
        Ok(Self {
            id,
            measurement_id: target.id(),
            previous_id: previous.map(Self::id),
            draft,
        })
    }
    pub const fn id(&self) -> CorrectionId {
        self.id
    }
    pub const fn measurement_id(&self) -> MeasurementId {
        self.measurement_id
    }
    pub const fn previous_id(&self) -> Option<CorrectionId> {
        self.previous_id
    }
    pub fn action(&self) -> &CorrectionAction {
        &self.draft.action
    }
    pub fn reason(&self) -> &str {
        &self.draft.reason
    }
    pub fn actor(&self) -> Option<&str> {
        self.draft.actor.as_deref()
    }
    pub const fn recorded_at(&self) -> UnixMillis {
        self.draft.recorded_at
    }
}
