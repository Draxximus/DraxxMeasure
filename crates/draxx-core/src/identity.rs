use std::{fmt, num::NonZeroI64};

use crate::DomainError;

macro_rules! positive_id {
    ($($name:ident),+ $(,)?) => {$(
        #[doc = concat!("Positive, SQLite-compatible ", stringify!($name), ". Allocation is an adapter responsibility.")]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(NonZeroI64);

        impl $name {
            pub fn new(value: i64) -> Result<Self, DomainError> {
                if value <= 0 {
                    return Err(DomainError::InvalidIdentifier);
                }
                NonZeroI64::new(value).map(Self).ok_or(DomainError::InvalidIdentifier)
            }

            pub const fn get(self) -> i64 { self.0.get() }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    )+};
}

positive_id!(
    SpecificationId,
    SpecificationVersion,
    SessionId,
    InspectionItemId,
    BundleId,
    MeasurementId,
    CorrectionId,
    Sequence,
    SnapshotRevision,
    PolicyVersion,
);

/// Milliseconds since Unix epoch, UTC. Timestamps are not ordering keys.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct UnixMillis(pub i64);

/// Opaque caller-generated token, reused verbatim only for a retry of the same command.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct RequestId(String);

impl RequestId {
    pub fn new(value: String) -> Result<Self, DomainError> {
        if value.trim().is_empty() {
            return Err(DomainError::EmptyField("request ID"));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}
