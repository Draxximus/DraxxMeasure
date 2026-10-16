//! Synchronous persistence ports and transactional SQLite implementation.
//!
//! A successful mutation receipt means COMMIT succeeded. Implementations must
//! validate ownership/state and allocate IDs/sequences inside the transaction.
//! A repeated request ID with identical payload returns the original receipt;
//! changed payload is a conflict. Equal measurement values are not duplicates.

use std::{error::Error, fmt};

mod sqlite;
mod wire;
pub use sqlite::SqliteStore;

use draxx_core::{
    Bundle, BundleId, CorrectionAction, CorrectionId, DomainError, InspectionItem,
    InspectionItemId, InspectionSession, InspectionSnapshot, InterpretedMeasurement, Length,
    Measurement, MeasurementCorrection, MeasurementInput, MeasurementProvenance, PartNumber,
    PartSpecification, RequestId, Resolution, SessionId, SessionTransition, SnapshotRevision,
    SpecificationId, SpecificationVersion, UnixMillis,
};

#[derive(Debug)]
pub enum StorageError {
    Domain(DomainError),
    NotFound,
    Conflict,
    Busy,
    Integrity(String),
    UnsupportedSchema {
        found: u32,
        supported: u32,
    },
    /// Retry/lookup with this same token; never claim failure means rollback occurred.
    CommitUnknown {
        request_id: RequestId,
        cause: Box<dyn Error + Send + Sync>,
    },
    Backend(Box<dyn Error + Send + Sync>),
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Domain(error) => error.fmt(f),
            Self::NotFound => f.write_str("inspection entity not found"),
            Self::Conflict => {
                f.write_str("stale state or request ID reused with different content")
            }
            Self::Busy => f.write_str("inspection database is busy; no acknowledgement issued"),
            Self::Integrity(message) => write!(f, "inspection integrity failure: {message}"),
            Self::UnsupportedSchema { found, supported } => {
                write!(f, "schema {found} is not supported (maximum {supported})")
            }
            Self::CommitUnknown { request_id, cause } => write!(
                f,
                "commit outcome unknown for request {}; retry with the SAME request ID: {cause}",
                request_id.as_str()
            ),
            Self::Backend(error) => write!(f, "storage failure: {error}"),
        }
    }
}

impl Error for StorageError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Domain(error) => Some(error),
            Self::Backend(error) => Some(error.as_ref()),
            Self::CommitUnknown { cause, .. } => Some(cause.as_ref()),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct CommitReceipt<T> {
    pub request_id: RequestId,
    pub revision: SnapshotRevision,
    pub value: T,
    pub replayed: bool,
}

#[derive(Clone, Debug)]
pub enum CommittedEntity {
    Part(PartNumber),
    Specification(draxx_core::SpecificationId),
    Session(SessionId),
    Item(InspectionItemId),
    Bundle(BundleId),
    Measurement(draxx_core::MeasurementId),
    Correction(CorrectionId),
}

#[derive(Clone, Debug)]
pub struct CommitRecord {
    pub request_id: RequestId,
    pub revision: SnapshotRevision,
    pub entity: CommittedEntity,
}

/// IDs and capture sequence are assigned by the adapter, not by a CLI counter.
#[derive(Clone, Debug)]
pub struct RecordMeasurement {
    pub request_id: RequestId,
    pub session_id: SessionId,
    pub item_id: InspectionItemId,
    pub bundle_id: BundleId,
    pub input: MeasurementInput,
    pub interpreted: InterpretedMeasurement,
    pub captured_at: UnixMillis,
    pub provenance: MeasurementProvenance,
}

/// Specification contents; the adapter allocates its immutable ID on commit.
#[derive(Clone, Debug)]
pub struct CreateSpecification {
    pub part_number: PartNumber,
    pub version: SpecificationVersion,
    pub nominal: Length,
    pub lsl: Length,
    pub usl: Length,
    pub resolution: Resolution,
}

#[derive(Clone, Debug)]
pub struct CorrectMeasurement {
    pub request_id: RequestId,
    pub measurement_id: draxx_core::MeasurementId,
    /// None means the original is still current; checked atomically before append.
    pub expected_previous: Option<CorrectionId>,
    pub action: CorrectionAction,
    pub reason: String,
    pub actor: Option<String>,
    pub recorded_at: UnixMillis,
}

#[derive(Clone, Debug)]
pub struct RecoveryState {
    pub session: InspectionSession,
    pub items: Vec<InspectionItem>,
    /// At most one open bundle per session in the Phase 1 design.
    pub open_bundle: Option<Bundle>,
    pub revision: SnapshotRevision,
    pub active_item: Option<InspectionItemId>,
}

/// Deliberately one port, not a generic CRUD framework or split transaction API.
pub trait InspectionStore {
    fn add_part(
        &mut self,
        request: RequestId,
        part: PartNumber,
    ) -> Result<CommitReceipt<PartNumber>, StorageError>;
    fn add_specification(
        &mut self,
        request: RequestId,
        specification: CreateSpecification,
    ) -> Result<CommitReceipt<PartSpecification>, StorageError>;
    fn create_session(
        &mut self,
        request: RequestId,
        at: UnixMillis,
    ) -> Result<CommitReceipt<InspectionSession>, StorageError>;
    fn add_item(
        &mut self,
        request: RequestId,
        session: SessionId,
        specification: draxx_core::SpecificationId,
    ) -> Result<CommitReceipt<InspectionItem>, StorageError>;
    /// Allocates the next per-item bundle sequence and enforces one open per session.
    fn start_bundle(
        &mut self,
        request: RequestId,
        item: InspectionItemId,
        label: String,
        at: UnixMillis,
    ) -> Result<CommitReceipt<Bundle>, StorageError>;
    fn close_bundle(
        &mut self,
        request: RequestId,
        bundle: BundleId,
        at: UnixMillis,
    ) -> Result<CommitReceipt<Bundle>, StorageError>;
    /// Close must check authoritative open-bundle count within this transaction.
    fn transition_session(
        &mut self,
        request: RequestId,
        session: SessionId,
        transition: SessionTransition,
        at: UnixMillis,
    ) -> Result<CommitReceipt<InspectionSession>, StorageError>;
    fn record_measurement(
        &mut self,
        command: RecordMeasurement,
    ) -> Result<CommitReceipt<Measurement>, StorageError>;
    /// Phase 1 policy: corrections require an active session; closed bundles may be corrected.
    fn correct_measurement(
        &mut self,
        command: CorrectMeasurement,
    ) -> Result<CommitReceipt<MeasurementCorrection>, StorageError>;
    fn lookup_request(&self, request: &RequestId) -> Result<Option<CommitRecord>, StorageError>;
    fn specification(&self, id: SpecificationId) -> Result<PartSpecification, StorageError>;
    fn specifications_for_part(
        &self,
        part: &PartNumber,
    ) -> Result<Vec<PartSpecification>, StorageError>;
    fn recover_session(&self, session: SessionId) -> Result<RecoveryState, StorageError>;
    /// Current consistent snapshot, not an unimplemented historical time-travel API.
    fn snapshot(&self, item: InspectionItemId) -> Result<InspectionSnapshot, StorageError>;
    fn sessions(&self) -> Result<Vec<InspectionSession>, StorageError>;
    fn select_item(
        &mut self,
        request: RequestId,
        session: SessionId,
        item: Option<InspectionItemId>,
        at: UnixMillis,
    ) -> Result<CommitReceipt<InspectionSession>, StorageError>;
    fn configure_policy(
        &mut self,
        request: RequestId,
        item: InspectionItemId,
        policy: Option<draxx_core::ReconstructionPolicy>,
        at: UnixMillis,
    ) -> Result<CommitReceipt<InspectionItem>, StorageError>;
    fn policy(
        &self,
        item: InspectionItemId,
    ) -> Result<Option<draxx_core::ReconstructionPolicy>, StorageError>;
    /// All items from one consistent session read transaction for a multi-part report.
    fn session_snapshots(
        &self,
        session: SessionId,
    ) -> Result<Vec<InspectionSnapshot>, StorageError>;
    fn bundle_capacity(&self, bundle: BundleId)
    -> Result<draxx_core::BundleCapacity, StorageError>;
}

impl From<DomainError> for StorageError {
    fn from(error: DomainError) -> Self {
        Self::Domain(error)
    }
}

/// Migration descriptor; SqliteStore accepts only its shipped immutable migration.
#[derive(Clone, Debug)]
pub struct Migration {
    pub version: u32,
    pub description: &'static str,
    pub sql: &'static str,
}

pub trait SchemaMigrator {
    fn current_version(&self) -> Result<u32, StorageError>;
    /// Backup before risky changes, reject newer schemas, preserve history atomically.
    fn migrate(&mut self, migrations: &[Migration]) -> Result<(), StorageError>;
}
