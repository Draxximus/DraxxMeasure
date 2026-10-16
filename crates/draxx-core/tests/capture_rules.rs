use draxx_core::*;
fn length(text: &str) -> Length {
    Length::parse_mm(text).unwrap()
}
fn make_spec(nominal: &str) -> PartSpecification {
    let nominal = length(nominal);
    let (lsl, usl) = PartSpecification::symmetric_limits(nominal, length("5")).unwrap();
    PartSpecification::new(
        SpecificationId::new(1).unwrap(),
        PartNumber::new("SYNTHETIC".into()).unwrap(),
        SpecificationVersion::new(1).unwrap(),
        nominal,
        lsl,
        usl,
        Resolution::new(length("0.1")).unwrap(),
    )
    .unwrap()
}
fn policy(lower: &str, upper: &str) -> ReconstructionPolicy {
    ReconstructionPolicy::new(
        SUFFIX_POLICY_NAME.into(),
        PolicyVersion::new(1).unwrap(),
        length(lower),
        length(upper),
    )
    .unwrap()
}
#[test]
fn reconstruction_reference_cases_are_exact_and_do_not_filter_by_specification() {
    let spec = make_spec("9368.9");
    for (lower, upper, input, expected) in [
        ("9300", "9399", "68", Some("9368")),
        ("9300", "9399", "80", Some("9380")),
        ("9950", "10049", "00", Some("10000")),
        ("9370", "9399", "68", None),
    ] {
        let result = SuffixReconstructor
            .reconstruct(
                &MeasurementInput::abbreviated(input.into()).unwrap(),
                &spec,
                Some(&policy(lower, upper)),
            )
            .unwrap();
        if let Some(value) = expected {
            let ReconstructionOutcome::Unique(value_result) = result else {
                panic!("not unique")
            };
            assert_eq!(value_result.value, length(value));
        } else {
            assert_eq!(result, ReconstructionOutcome::Unresolved);
        }
    }
    let result = SuffixReconstructor
        .reconstruct(
            &MeasurementInput::abbreviated("09".into()).unwrap(),
            &make_spec("10608.9"),
            Some(&policy("10558", "10657")),
        )
        .unwrap();
    let ReconstructionOutcome::Unique(value) = result else {
        panic!("not unique")
    };
    assert_eq!(value.value, length("10609"));
}
#[test]
fn ambiguity_is_not_a_guess_and_bounds_are_replayable() {
    let spec = make_spec("9368.9");
    let policy = policy("9300", "9499");
    let input = MeasurementInput::abbreviated("68".into()).unwrap();
    let ReconstructionOutcome::Ambiguous { candidates, .. } = SuffixReconstructor
        .reconstruct(&input, &spec, Some(&policy))
        .unwrap()
    else {
        panic!("ambiguity lost")
    };
    assert_eq!(candidates, vec![length("9368"), length("9468")]);
    let mut selected = InterpretedMeasurement {
        value: length("9368"),
        method: InterpretationMethod::IntegerMillimetreSuffix {
            policy: policy.clone(),
            operator_selected: false,
        },
    };
    assert!(validate_capture_interpretation(&input, &selected, &spec).is_err());
    selected.method = InterpretationMethod::IntegerMillimetreSuffix {
        policy,
        operator_selected: true,
    };
    assert!(validate_capture_interpretation(&input, &selected, &spec).is_ok());
    let full = MeasurementInput::full_length("=9999.1".into()).unwrap();
    let ReconstructionOutcome::Unique(value) =
        SuffixReconstructor.reconstruct(&full, &spec, None).unwrap()
    else {
        panic!("full override rejected")
    };
    assert_eq!(value.value, length("9999.1"));
    assert!(matches!(
        SuffixReconstructor.reconstruct(&input, &spec, None),
        Err(ReconstructionError::PolicyRequired)
    ));
}
#[test]
fn reconstruction_handles_fractional_edges_zero_resolution_mismatch_and_extreme_windows() {
    let spec = make_spec("9368.9");
    let input = MeasurementInput::abbreviated("00".into()).unwrap();
    assert!(matches!(
        SuffixReconstructor
            .reconstruct(&input, &spec, Some(&policy("0", "0")))
            .unwrap(),
        ReconstructionOutcome::Unique(_)
    ));
    assert_eq!(
        SuffixReconstructor
            .reconstruct(
                &MeasurementInput::abbreviated("68".into()).unwrap(),
                &spec,
                Some(&policy("9368.000001", "9368.999999"))
            )
            .unwrap(),
        ReconstructionOutcome::Unresolved
    );
    assert!(matches!(
        SuffixReconstructor.reconstruct(&input, &spec, Some(&policy("0", "9223372036854.775807"))),
        Err(ReconstructionError::CandidateLimitExceeded)
    ));
    let input = MeasurementInput::full_length("=9368.91".into()).unwrap();
    assert!(matches!(
        SuffixReconstructor.reconstruct(&input, &spec, None),
        Err(ReconstructionError::Validation(
            DomainError::ResolutionMismatch
        ))
    ));
    let unknown = ReconstructionPolicy::new(
        "unknown".into(),
        PolicyVersion::new(1).unwrap(),
        length("0"),
        length("99"),
    )
    .unwrap();
    assert!(matches!(
        SuffixReconstructor.reconstruct(
            &MeasurementInput::abbreviated("01".into()).unwrap(),
            &spec,
            Some(&unknown)
        ),
        Err(ReconstructionError::UnsupportedPolicy)
    ));
}
#[test]
fn capacity_rules_cover_closure_undo_correction_and_restoration() {
    let full = BundleCapacity::new(42).unwrap();
    assert_eq!(full.ensure_can_append(), Err(DomainError::BundleFull));
    assert!(full.ensure_can_close().is_ok());
    assert!(BundleCapacity::new(43).is_err());
    assert_eq!(
        BundleCapacity::new(0).unwrap().ensure_can_close(),
        Err(DomainError::EmptyBundle)
    );
    let empty = full
        .after_correction(true, &CorrectionAction::Invalidate, BundleStatus::Open)
        .unwrap();
    assert_eq!(empty.active(), 41);
    assert!(empty.ensure_can_append().is_ok());
    let replace = CorrectionAction::Replace {
        input: MeasurementInput::full_length("=1".into()).unwrap(),
        interpreted: InterpretedMeasurement {
            value: length("1"),
            method: InterpretationMethod::FullLength,
        },
    };
    assert_eq!(
        full.after_correction(false, &replace, BundleStatus::Open),
        Err(DomainError::BundleFull)
    );
    assert_eq!(
        full.after_correction(true, &replace, BundleStatus::Open)
            .unwrap(),
        full
    );
    assert_eq!(
        BundleCapacity::new(1).unwrap().after_correction(
            true,
            &CorrectionAction::Invalidate,
            BundleStatus::Closed
        ),
        Err(DomainError::EmptyBundle)
    );
}
