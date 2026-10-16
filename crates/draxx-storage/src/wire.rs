//! Versioned JSON payloads contain exact integer quantities, never domain Deserialize.
use crate::StorageError;
use draxx_core::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct PolicyWire {
    pub name: String,
    pub version: i64,
    pub lower_nm: i64,
    pub upper_nm: i64,
}
impl PolicyWire {
    pub fn encode(policy: &ReconstructionPolicy) -> Self {
        Self {
            name: policy.name().into(),
            version: policy.version().get(),
            lower_nm: policy.lower().nanometres(),
            upper_nm: policy.upper().nanometres(),
        }
    }
    pub fn decode(self) -> Result<ReconstructionPolicy, StorageError> {
        Ok(ReconstructionPolicy::new(
            self.name,
            PolicyVersion::new(self.version)?,
            Length::from_nanometres(self.lower_nm)?,
            Length::from_nanometres(self.upper_nm)?,
        )?)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ValueWire {
    pub raw: String,
    pub value_nm: i64,
    pub policy: Option<PolicyWire>,
    pub operator_selected: bool,
}
impl ValueWire {
    pub fn encode(input: &MeasurementInput, interpreted: &InterpretedMeasurement) -> Self {
        let (policy, operator_selected) = match &interpreted.method {
            InterpretationMethod::FullLength => (None, false),
            InterpretationMethod::IntegerMillimetreSuffix {
                policy,
                operator_selected,
            } => (Some(PolicyWire::encode(policy)), *operator_selected),
        };
        Self {
            raw: input.raw().into(),
            value_nm: interpreted.value.nanometres(),
            policy,
            operator_selected,
        }
    }
    pub fn decode(self) -> Result<(MeasurementInput, InterpretedMeasurement), StorageError> {
        let (input, method) = match self.policy {
            Some(policy) => (
                MeasurementInput::abbreviated(self.raw)?,
                InterpretationMethod::IntegerMillimetreSuffix {
                    policy: policy.decode()?,
                    operator_selected: self.operator_selected,
                },
            ),
            None if !self.operator_selected => (
                MeasurementInput::full_length(self.raw)?,
                InterpretationMethod::FullLength,
            ),
            _ => {
                return Err(StorageError::Integrity(
                    "full input cannot have an operator candidate selection".into(),
                ));
            }
        };
        let interpreted = InterpretedMeasurement {
            value: Length::from_nanometres(self.value_nm)?,
            method,
        };
        interpreted.validate(&input)?;
        Ok((input, interpreted))
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct SessionWire {
    pub id: i64,
    pub status: String,
    pub created_at: i64,
    pub closed_at: Option<i64>,
}
impl SessionWire {
    pub fn encode(session: &InspectionSession) -> Self {
        Self {
            id: session.id().get(),
            status: status_name(session.status()).into(),
            created_at: session.created_at().0,
            closed_at: session.closed_at().map(|v| v.0),
        }
    }
    pub fn decode(self) -> Result<InspectionSession, StorageError> {
        let status = match self.status.as_str() {
            "active" => SessionStatus::Active,
            "suspended" => SessionStatus::Suspended,
            "closed" => SessionStatus::Closed,
            _ => return Err(StorageError::Integrity("unknown session status".into())),
        };
        Ok(InspectionSession::restore(
            SessionId::new(self.id)?,
            status,
            UnixMillis(self.created_at),
            self.closed_at.map(UnixMillis),
        )?)
    }
}
pub(crate) fn status_name(status: SessionStatus) -> &'static str {
    match status {
        SessionStatus::Active => "active",
        SessionStatus::Suspended => "suspended",
        SessionStatus::Closed => "closed",
    }
}

#[derive(Serialize, Deserialize)]
pub(crate) struct BundleWire {
    pub id: i64,
    pub item_id: i64,
    pub sequence: i64,
    pub label: String,
    pub closed: bool,
    pub started_at: i64,
    pub closed_at: Option<i64>,
}
impl BundleWire {
    pub fn encode(bundle: &Bundle) -> Self {
        Self {
            id: bundle.id().get(),
            item_id: bundle.item_id().get(),
            sequence: bundle.sequence().get(),
            label: bundle.label().into(),
            closed: bundle.status() == BundleStatus::Closed,
            started_at: bundle.started_at().0,
            closed_at: bundle.closed_at().map(|v| v.0),
        }
    }
    pub fn decode(
        self,
        session: &InspectionSession,
        item: &InspectionItem,
    ) -> Result<Bundle, StorageError> {
        Ok(Bundle::restore(
            BundleRecord {
                id: BundleId::new(self.id)?,
                sequence: Sequence::new(self.sequence)?,
                label: self.label,
                status: if self.closed {
                    BundleStatus::Closed
                } else {
                    BundleStatus::Open
                },
                started_at: UnixMillis(self.started_at),
                closed_at: self.closed_at.map(UnixMillis),
            },
            session,
            item,
        )?)
    }
}

pub(crate) fn json<T: Serialize>(value: &T) -> Result<String, StorageError> {
    serde_json::to_string(value).map_err(|e| StorageError::Backend(Box::new(e)))
}
pub(crate) fn parse<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, StorageError> {
    serde_json::from_str(value)
        .map_err(|e| StorageError::Integrity(format!("invalid stored JSON: {e}")))
}
