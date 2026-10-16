use crate::*;

pub const SUFFIX_POLICY_NAME: &str = "integer-mm-window";

/// Version 1: integer-mm suffix candidates inside an explicitly approved interval.
pub struct SuffixReconstructor;

impl MeasurementReconstructor for SuffixReconstructor {
    fn reconstruct(
        &self,
        input: &MeasurementInput,
        specification: &PartSpecification,
        policy: Option<&ReconstructionPolicy>,
    ) -> Result<ReconstructionOutcome, ReconstructionError> {
        let validate = |value| {
            specification
                .resolution()
                .validate(value)
                .map_err(ReconstructionError::Validation)
        };
        if input.mode() == InputMode::FullLength {
            let value =
                Length::parse_mm(&input.raw()[1..]).map_err(ReconstructionError::Validation)?;
            validate(value)?;
            return Ok(ReconstructionOutcome::Unique(InterpretedMeasurement {
                value,
                method: InterpretationMethod::FullLength,
            }));
        }
        let policy = policy.ok_or(ReconstructionError::PolicyRequired)?;
        if policy.name() != SUFFIX_POLICY_NAME || policy.version().get() != 1 {
            return Err(ReconstructionError::UnsupportedPolicy);
        }
        let digits = input.raw().as_bytes();
        let suffix = i128::from((digits[0] - b'0') * 10 + digits[1] - b'0') * 1_000_000;
        let period = 100_000_000_i128;
        let low = i128::from(policy.lower().nanometres()) - suffix;
        let high = i128::from(policy.upper().nanometres()) - suffix;
        let first = low.div_euclid(period) + i128::from(low.rem_euclid(period) != 0);
        let last = high.div_euclid(period);
        if first > last {
            return Ok(ReconstructionOutcome::Unresolved);
        }
        if last - first + 1 > 1024 {
            return Err(ReconstructionError::CandidateLimitExceeded);
        }
        let mut candidates = Vec::new();
        for k in first..=last {
            let nm = i64::try_from(k * period + suffix)
                .map_err(|_| ReconstructionError::Validation(DomainError::NumericOverflow))?;
            candidates.push(Length::from_nanometres(nm).map_err(ReconstructionError::Validation)?);
        }
        if candidates.len() != 1 {
            return Ok(ReconstructionOutcome::Ambiguous {
                candidates,
                policy: policy.clone(),
            });
        }
        let value = candidates[0];
        validate(value)?;
        Ok(ReconstructionOutcome::Unique(InterpretedMeasurement {
            value,
            method: InterpretationMethod::IntegerMillimetreSuffix {
                policy: policy.clone(),
                operator_selected: false,
            },
        }))
    }
}

/// Recheck a submitted interpretation inside the persistence transaction; a client
/// cannot label an ambiguous input as automatically unique and commit a guess.
pub fn validate_capture_interpretation(
    input: &MeasurementInput,
    interpreted: &InterpretedMeasurement,
    specification: &PartSpecification,
) -> Result<(), DomainError> {
    interpreted.validate(input)?;
    let (policy, selected) = match &interpreted.method {
        InterpretationMethod::FullLength => (None, false),
        InterpretationMethod::IntegerMillimetreSuffix {
            policy,
            operator_selected,
        } => (Some(policy), *operator_selected),
    };
    let outcome = SuffixReconstructor
        .reconstruct(input, specification, policy)
        .map_err(|_| DomainError::InterpretationMismatch)?;
    match outcome {
        ReconstructionOutcome::Unique(result) if result.value == interpreted.value => Ok(()),
        ReconstructionOutcome::Ambiguous { candidates, .. }
            if selected && candidates.contains(&interpreted.value) =>
        {
            specification.resolution().validate(interpreted.value)
        }
        _ => Err(DomainError::InterpretationMismatch),
    }
}
