//! Shared synchronous inspection use cases, independent of terminal and GUI.

use draxx_core::*;
use draxx_storage::*;
use std::{
    error::Error,
    fmt,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn now() -> Result<UnixMillis, ApplicationError> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| ApplicationError::Clock)?;
    Ok(UnixMillis(
        i64::try_from(duration.as_millis()).map_err(|_| ApplicationError::Clock)?,
    ))
}
pub fn request_id() -> Result<RequestId, DomainError> {
    RequestId::new(uuid::Uuid::new_v4().to_string())
}

#[derive(Debug)]
pub enum ApplicationError {
    Domain(DomainError),
    Storage(StorageError),
    Reconstruction(ReconstructionError),
    Context(&'static str),
    Ambiguous(Vec<Length>),
    Unresolved,
    PendingWrite,
    Clock,
}
impl fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain(e)=>e.fmt(f),Self::Storage(e)=>e.fmt(f),Self::Reconstruction(e)=>e.fmt(f),Self::Context(message)=>f.write_str(message),
            Self::Ambiguous(candidates)=>{write!(f,"ambiguous input ({} candidates: ",candidates.len())?;for (i,value) in candidates.iter().take(8).enumerate(){if i!=0 {f.write_str(", ")?;}write!(f,"{value}")?;}f.write_str(" mm); use an explicit full override such as =9368")},
            Self::Unresolved=>f.write_str("no candidate in the approved capture interval; use =full_length or change /policy explicitly"),
            Self::PendingWrite=>f.write_str("an unacknowledged measurement is pending; use /retry with its original request ID before another mutation"),
            Self::Clock=>f.write_str("system time cannot be represented as UTC epoch milliseconds"),
        }
    }
}
impl Error for ApplicationError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Domain(e) => Some(e),
            Self::Storage(e) => Some(e),
            Self::Reconstruction(e) => Some(e),
            _ => None,
        }
    }
}
impl From<DomainError> for ApplicationError {
    fn from(e: DomainError) -> Self {
        Self::Domain(e)
    }
}
impl From<StorageError> for ApplicationError {
    fn from(e: StorageError) -> Self {
        Self::Storage(e)
    }
}
impl From<ReconstructionError> for ApplicationError {
    fn from(e: ReconstructionError) -> Self {
        Self::Reconstruction(e)
    }
}

pub struct PartSetup {
    pub part_number: PartNumber,
    pub nominal: Length,
    pub tolerance: Length,
    pub resolution: Resolution,
    pub policy: Option<ReconstructionPolicy>,
}
pub struct CaptureState {
    pub session: InspectionSession,
    pub item: Option<InspectionItem>,
    pub bundle: Option<Bundle>,
    pub active_count: usize,
}
pub enum ClosedContext {
    Bundle {
        bundle: Bundle,
        measurements: usize,
    },
    Part {
        item: InspectionItem,
        bundles: usize,
        measurements: usize,
    },
    Session(SessionId),
}

/// The same transaction-oriented service can be used by a later Tauri adapter.
pub struct InspectionService<S: InspectionStore> {
    store: S,
    session: Option<SessionId>,
    pending: Option<RecordMeasurement>,
    last_closed: Option<SessionId>,
}
impl<S: InspectionStore> InspectionService<S> {
    pub fn new(store: S) -> Self {
        Self {
            store,
            session: None,
            pending: None,
            last_closed: None,
        }
    }
    pub fn store(&self) -> &S {
        &self.store
    }
    pub fn last_closed(&self) -> Option<SessionId> {
        self.last_closed
    }
    fn no_pending(&self) -> Result<(), ApplicationError> {
        if self.pending.is_some() {
            Err(ApplicationError::PendingWrite)
        } else {
            Ok(())
        }
    }
    fn session_id(&self) -> Result<SessionId, ApplicationError> {
        self.session.ok_or(ApplicationError::Context(
            "start /session or /resume SESSION_ID first",
        ))
    }
    pub fn state(&self) -> Result<Option<CaptureState>, ApplicationError> {
        let Some(id) = self.session else {
            return Ok(None);
        };
        let recovered = self.store.recover_session(id)?;
        let item = match recovered.active_item {
            Some(selected) => Some(
                recovered
                    .items
                    .into_iter()
                    .find(|item| item.id() == selected)
                    .ok_or(DomainError::AssociationMismatch)?,
            ),
            None => None,
        };
        if let Some(bundle) = &recovered.open_bundle
            && item.as_ref().map(InspectionItem::id) != Some(bundle.item_id())
        {
            return Err(DomainError::AssociationMismatch.into());
        }
        let count = recovered
            .open_bundle
            .as_ref()
            .map(|b| self.store.bundle_capacity(b.id()))
            .transpose()?
            .map_or(0, BundleCapacity::active);
        Ok(Some(CaptureState {
            session: recovered.session,
            item,
            bundle: recovered.open_bundle,
            active_count: count,
        }))
    }
    pub fn start_session(&mut self) -> Result<InspectionSession, ApplicationError> {
        self.no_pending()?;
        if self.session.is_some() {
            return Err(ApplicationError::Context(
                "close the current session context before starting another",
            ));
        }
        let session = self.store.create_session(request_id()?, now()?)?.value;
        self.session = Some(session.id());
        Ok(session)
    }
    pub fn resume(&mut self, id: SessionId) -> Result<(), ApplicationError> {
        self.no_pending()?;
        if self.session.is_some() {
            return Err(ApplicationError::Context(
                "use /exit and restart before switching to another session",
            ));
        }
        let state = self.store.recover_session(id)?;
        match state.session.status() {
            SessionStatus::Closed => {
                return Err(ApplicationError::Context(
                    "session is closed; use /report SESSION_ID to retry exports",
                ));
            }
            SessionStatus::Suspended => {
                self.store.transition_session(
                    request_id()?,
                    id,
                    SessionTransition::Resume,
                    now()?,
                )?;
            }
            SessionStatus::Active => {}
        }
        self.session = Some(id);
        Ok(())
    }
    pub fn existing_item(
        &self,
        part: &PartNumber,
    ) -> Result<Option<InspectionItem>, ApplicationError> {
        let state = self.store.recover_session(self.session_id()?)?;
        Ok(state
            .items
            .into_iter()
            .rev()
            .find(|item| item.part_number() == part))
    }
    pub fn select_part(&mut self, item: InspectionItemId) -> Result<(), ApplicationError> {
        self.no_pending()?;
        self.store
            .select_item(request_id()?, self.session_id()?, Some(item), now()?)?;
        Ok(())
    }
    pub fn create_part(&mut self, setup: PartSetup) -> Result<InspectionItem, ApplicationError> {
        self.no_pending()?;
        let state = self
            .state()?
            .ok_or(ApplicationError::Context("start /session first"))?;
        if state.item.is_some() || state.bundle.is_some() {
            return Err(ApplicationError::Context(
                "close the active Part Number context first",
            ));
        }
        let (lsl, usl) = PartSpecification::symmetric_limits(setup.nominal, setup.tolerance)?;
        let versions = self.store.specifications_for_part(&setup.part_number)?;
        let version = versions.last().map_or(Ok(1), |v| {
            v.version()
                .get()
                .checked_add(1)
                .ok_or(DomainError::NumericOverflow)
        })?;
        self.store
            .add_part(request_id()?, setup.part_number.clone())?;
        let specification = self
            .store
            .add_specification(
                request_id()?,
                CreateSpecification {
                    part_number: setup.part_number,
                    version: SpecificationVersion::new(version)?,
                    nominal: setup.nominal,
                    lsl,
                    usl,
                    resolution: setup.resolution,
                },
            )?
            .value;
        let item = self
            .store
            .add_item(request_id()?, state.session.id(), specification.id())?
            .value;
        self.store
            .configure_policy(request_id()?, item.id(), setup.policy, now()?)?;
        Ok(item)
    }
    pub fn start_bundle(&mut self) -> Result<Bundle, ApplicationError> {
        self.no_pending()?;
        let state = self
            .state()?
            .ok_or(ApplicationError::Context("start /session first"))?;
        let item = state
            .item
            .ok_or(ApplicationError::Context("select /partnumber first"))?;
        if state.bundle.is_some() {
            return Err(ApplicationError::Context(
                "close the current bundle before starting another",
            ));
        }
        Ok(self
            .store
            .start_bundle(request_id()?, item.id(), "Bundle".into(), now()?)?
            .value)
    }
    fn interpret(
        &self,
        raw: String,
        item: &InspectionItem,
    ) -> Result<(MeasurementInput, InterpretedMeasurement), ApplicationError> {
        let input = if raw.starts_with('=') {
            MeasurementInput::full_length(raw)?
        } else {
            MeasurementInput::abbreviated(raw)?
        };
        let spec = self.store.specification(item.specification_id())?;
        let policy = self.store.policy(item.id())?;
        match SuffixReconstructor.reconstruct(&input, &spec, policy.as_ref())? {
            ReconstructionOutcome::Unique(value) => Ok((input, value)),
            ReconstructionOutcome::Ambiguous { candidates, .. } => {
                Err(ApplicationError::Ambiguous(candidates))
            }
            ReconstructionOutcome::Unresolved => Err(ApplicationError::Unresolved),
        }
    }
    pub fn record(&mut self, raw: String) -> Result<CommitReceipt<Measurement>, ApplicationError> {
        self.no_pending()?;
        let state = self
            .state()?
            .ok_or(ApplicationError::Context("start /session first"))?;
        let item = state
            .item
            .ok_or(ApplicationError::Context("select /partnumber first"))?;
        let bundle = state
            .bundle
            .ok_or(ApplicationError::Context("start /bundle first"))?;
        BundleCapacity::new(state.active_count)?.ensure_can_append()?;
        let (input, interpreted) = self.interpret(raw, &item)?;
        let command = RecordMeasurement {
            request_id: request_id()?,
            session_id: state.session.id(),
            item_id: item.id(),
            bundle_id: bundle.id(),
            input,
            interpreted,
            captured_at: now()?,
            provenance: MeasurementProvenance::default(),
        };
        self.submit(command)
    }
    fn submit(
        &mut self,
        command: RecordMeasurement,
    ) -> Result<CommitReceipt<Measurement>, ApplicationError> {
        match self.store.record_measurement(command.clone()) {
            Ok(receipt) => {
                self.pending = None;
                Ok(receipt)
            }
            Err(error) => {
                if matches!(
                    error,
                    StorageError::Busy
                        | StorageError::Backend(_)
                        | StorageError::CommitUnknown { .. }
                ) {
                    self.pending = Some(command);
                } else {
                    self.pending = None;
                }
                Err(error.into())
            }
        }
    }
    pub fn retry(&mut self) -> Result<CommitReceipt<Measurement>, ApplicationError> {
        self.submit(self.pending.clone().ok_or(ApplicationError::Context(
            "no pending storage submission; re-enter invalid input explicitly",
        ))?)
    }
    pub fn pending_input(&self) -> Option<&str> {
        self.pending.as_ref().map(|p| p.input.raw())
    }
    pub fn current_snapshot(&self) -> Result<InspectionSnapshot, ApplicationError> {
        let state = self
            .state()?
            .ok_or(ApplicationError::Context("start /session first"))?;
        let item = state
            .item
            .ok_or(ApplicationError::Context("select /partnumber first"))?;
        Ok(self.store.snapshot(item.id())?)
    }
    pub fn target(&self, sequence: Option<i64>) -> Result<MeasurementHistory, ApplicationError> {
        let state = self
            .state()?
            .ok_or(ApplicationError::Context("start /session first"))?;
        let current = state.bundle.ok_or(ApplicationError::Context(
            "correction commands require an active bundle context",
        ))?;
        let snapshot = self.current_snapshot()?;
        let data = snapshot
            .bundles
            .into_iter()
            .find(|b| b.bundle.id() == current.id())
            .ok_or(ApplicationError::Context("bundle not found"))?;
        data.measurements
            .into_iter()
            .rev()
            .find(|m| match sequence {
                Some(index) => m.original().sequence().get() == index,
                None => m.effective().is_some(),
            })
            .ok_or(ApplicationError::Context(
                "no matching measurement in the current bundle",
            ))
    }
    pub fn correct(
        &mut self,
        target: MeasurementHistory,
        replacement: Option<String>,
        reason: String,
    ) -> Result<MeasurementCorrection, ApplicationError> {
        self.no_pending()?;
        let state = self
            .state()?
            .ok_or(ApplicationError::Context("no session"))?;
        let item = state
            .item
            .ok_or(ApplicationError::Context("no Part Number"))?;
        if state.bundle.as_ref().map(Bundle::id) != Some(target.original().bundle_id())
            || item.id() != target.original().item_id()
        {
            return Err(DomainError::AssociationMismatch.into());
        }
        let action = if let Some(raw) = replacement {
            let (input, interpreted) = self.interpret(raw, &item)?;
            CorrectionAction::Replace { input, interpreted }
        } else {
            CorrectionAction::Invalidate
        };
        Ok(self
            .store
            .correct_measurement(CorrectMeasurement {
                request_id: request_id()?,
                measurement_id: target.original().id(),
                expected_previous: target.corrections().last().map(MeasurementCorrection::id),
                action,
                reason,
                actor: None,
                recorded_at: now()?,
            })?
            .value)
    }
    pub fn configure_policy(
        &mut self,
        policy: Option<ReconstructionPolicy>,
    ) -> Result<(), ApplicationError> {
        self.no_pending()?;
        let state = self
            .state()?
            .ok_or(ApplicationError::Context("start /session first"))?;
        let item = state
            .item
            .ok_or(ApplicationError::Context("select /partnumber first"))?;
        self.store
            .configure_policy(request_id()?, item.id(), policy, now()?)?;
        Ok(())
    }
    pub fn close(&mut self) -> Result<ClosedContext, ApplicationError> {
        self.no_pending()?;
        let state = self
            .state()?
            .ok_or(ApplicationError::Context("no context to close"))?;
        if let Some(bundle) = state.bundle {
            let closed = self
                .store
                .close_bundle(request_id()?, bundle.id(), now()?)?
                .value;
            return Ok(ClosedContext::Bundle {
                bundle: closed,
                measurements: state.active_count,
            });
        }
        if let Some(item) = state.item {
            let snapshot = self.store.snapshot(item.id())?;
            let measurements = snapshot
                .bundles
                .iter()
                .flat_map(|b| &b.measurements)
                .filter(|m| m.effective().is_some())
                .count();
            self.store
                .select_item(request_id()?, state.session.id(), None, now()?)?;
            return Ok(ClosedContext::Part {
                item,
                bundles: snapshot.bundles.len(),
                measurements,
            });
        }
        self.store.transition_session(
            request_id()?,
            state.session.id(),
            SessionTransition::Close,
            now()?,
        )?;
        self.session = None;
        self.last_closed = Some(state.session.id());
        Ok(ClosedContext::Session(state.session.id()))
    }
}
