use std::{error::Error, fmt};

/// Recoverable domain failures; no operator input should cause a panic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainError {
    EmptyField(&'static str),
    InvalidIdentifier,
    InvalidDecimal,
    PrecisionExceeded,
    NumericOverflow,
    NegativeLength,
    InvalidSpecification,
    InvalidResolution,
    ResolutionMismatch,
    InvalidAbbreviation,
    InvalidFullOverride,
    InterpretationMismatch,
    InvalidCaptureInterval,
    InvalidTransition(&'static str),
    OpenBundlesRemain,
    AssociationMismatch,
    InvalidCorrectionChain,
    DuplicateIdentity,
    BundleFull,
    EmptyBundle,
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyField(field) => write!(f, "{field} must not be empty"),
            Self::InvalidIdentifier => f.write_str("identifier/index must be in 1..=i64::MAX"),
            Self::InvalidDecimal => f.write_str("expected an unsigned decimal with an ASCII point"),
            Self::PrecisionExceeded => f.write_str(
                "at most six millimetre decimal places are supported; no rounding performed",
            ),
            Self::NumericOverflow => f.write_str("dimension exceeds the supported exact range"),
            Self::NegativeLength => f.write_str("length must not be negative"),
            Self::InvalidSpecification => {
                f.write_str("require LSL < USL and LSL <= nominal <= USL")
            }
            Self::InvalidResolution => f.write_str("measurement resolution must be positive"),
            Self::ResolutionMismatch => {
                f.write_str("measurement is not an exact multiple of instrument resolution")
            }
            Self::InvalidAbbreviation => {
                f.write_str("abbreviated input requires exactly two ASCII digits")
            }
            Self::InvalidFullOverride => {
                f.write_str("full input requires '=' followed by an exact millimetre decimal")
            }
            Self::InterpretationMismatch => {
                f.write_str("interpretation does not match input or its recorded policy")
            }
            Self::InvalidCaptureInterval => {
                f.write_str("capture interval lower bound exceeds upper bound")
            }
            Self::InvalidTransition(reason) => write!(f, "invalid lifecycle transition: {reason}"),
            Self::OpenBundlesRemain => {
                f.write_str("close all open bundles before closing the session")
            }
            Self::AssociationMismatch => {
                f.write_str("session/item/bundle/specification association mismatch")
            }
            Self::InvalidCorrectionChain => {
                f.write_str("correction does not extend this measurement's history")
            }
            Self::DuplicateIdentity => f.write_str("duplicate identifier in history"),
            Self::BundleFull => f.write_str(
                "bundle is full (42 active measurements); use /close or explicitly undo an entry",
            ),
            Self::EmptyBundle => {
                f.write_str("a closed bundle must contain 1 to 42 active measurements")
            }
        }
    }
}

impl Error for DomainError {}
