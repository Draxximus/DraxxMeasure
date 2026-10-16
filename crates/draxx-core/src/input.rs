use std::{error::Error, fmt};

use crate::{DomainError, Length, PartSpecification, PolicyVersion};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputMode {
    Abbreviated,
    FullLength,
}

/// Raw input is preserved exactly, including the full-override marker and zeroes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasurementInput {
    raw: String,
    mode: InputMode,
}

impl MeasurementInput {
    pub fn abbreviated(raw: String) -> Result<Self, DomainError> {
        if raw.len() != 2 || !raw.bytes().all(|b| b.is_ascii_digit()) {
            return Err(DomainError::InvalidAbbreviation);
        }
        Ok(Self {
            raw,
            mode: InputMode::Abbreviated,
        })
    }

    /// Full overrides use `=9368.0`; this constructor validates exact representation.
    pub fn full_length(raw: String) -> Result<Self, DomainError> {
        let value = raw
            .strip_prefix('=')
            .ok_or(DomainError::InvalidFullOverride)?;
        Length::parse_mm(value)?;
        Ok(Self {
            raw,
            mode: InputMode::FullLength,
        })
    }

    pub fn raw(&self) -> &str {
        &self.raw
    }
    pub const fn mode(&self) -> InputMode {
        self.mode
    }
}

/// Explicit approved configuration, never inferred from specification limits.
/// Candidate policy: the last two integer-mm digits within these inclusive bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReconstructionPolicy {
    name: String,
    version: PolicyVersion,
    lower: Length,
    upper: Length,
}

impl ReconstructionPolicy {
    pub fn new(
        name: String,
        version: PolicyVersion,
        lower: Length,
        upper: Length,
    ) -> Result<Self, DomainError> {
        if name.trim().is_empty() {
            return Err(DomainError::EmptyField("policy name"));
        }
        if lower > upper {
            return Err(DomainError::InvalidCaptureInterval);
        }
        Ok(Self {
            name,
            version,
            lower,
            upper,
        })
    }

    pub fn name(&self) -> &str {
        &self.name
    }
    pub const fn version(&self) -> PolicyVersion {
        self.version
    }
    pub const fn lower(&self) -> Length {
        self.lower
    }
    pub const fn upper(&self) -> Length {
        self.upper
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterpretationMethod {
    FullLength,
    IntegerMillimetreSuffix {
        policy: ReconstructionPolicy,
        operator_selected: bool,
    },
}

/// A reconstruction result, not evidence that an instrument is calibrated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterpretedMeasurement {
    pub value: Length,
    pub method: InterpretationMethod,
}

impl InterpretedMeasurement {
    /// Validate provenance consistency, not candidate uniqueness (the engine owns that).
    pub fn validate(&self, input: &MeasurementInput) -> Result<(), DomainError> {
        match (&self.method, input.mode()) {
            (InterpretationMethod::FullLength, InputMode::FullLength) => {
                let text = input
                    .raw()
                    .strip_prefix('=')
                    .ok_or(DomainError::InvalidFullOverride)?;
                if self.value != Length::parse_mm(text)? {
                    return Err(DomainError::InterpretationMismatch);
                }
            }
            (
                InterpretationMethod::IntegerMillimetreSuffix { policy, .. },
                InputMode::Abbreviated,
            ) => {
                // Check a claimed result's suffix and bounds without reconstructing candidates.
                let suffix = i64::from(input.raw().as_bytes()[0] - b'0') * 10
                    + i64::from(input.raw().as_bytes()[1] - b'0');
                if self.value < policy.lower
                    || self.value > policy.upper
                    || self.value.nanometres() % 100_000_000 != suffix * 1_000_000
                {
                    return Err(DomainError::InterpretationMismatch);
                }
            }
            _ => return Err(DomainError::InterpretationMismatch),
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReconstructionOutcome {
    Unique(InterpretedMeasurement),
    /// An engine must return at least two candidates, never select by tolerance.
    Ambiguous {
        candidates: Vec<Length>,
        policy: ReconstructionPolicy,
    },
    /// Valid abbreviation, but no candidate in the configured capture interval.
    Unresolved,
}

#[derive(Debug)]
pub enum ReconstructionError {
    Validation(DomainError),
    PolicyRequired,
    UnsupportedPolicy,
    CandidateLimitExceeded,
}

impl fmt::Display for ReconstructionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(error) => error.fmt(f),
            Self::PolicyRequired => {
                f.write_str("abbreviation requires an explicitly approved policy")
            }
            Self::UnsupportedPolicy => f.write_str("unsupported reconstruction policy/version"),
            Self::CandidateLimitExceeded => {
                f.write_str("candidate set is too large; request full entry")
            }
        }
    }
}

impl Error for ReconstructionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

/// Pure deterministic interface, implemented by SuffixReconstructor in Phase 2.
pub trait MeasurementReconstructor {
    /// Full mode bypasses abbreviation bounds, not precision/resolution checks.
    /// An out-of-specification value is a valid observation, not an error.
    fn reconstruct(
        &self,
        input: &MeasurementInput,
        specification: &PartSpecification,
        policy: Option<&ReconstructionPolicy>,
    ) -> Result<ReconstructionOutcome, ReconstructionError>;
}
