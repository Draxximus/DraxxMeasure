use draxx_core::*;

fn length(text: &str) -> Length {
    Length::parse_mm(text).unwrap()
}

fn specification() -> PartSpecification {
    PartSpecification::new(
        SpecificationId::new(1).unwrap(),
        PartNumber::new("405363".into()).unwrap(),
        SpecificationVersion::new(1).unwrap(),
        length("9368.9"),
        length("9363.9"),
        length("9373.9"),
        Resolution::new(length("0.1")).unwrap(),
    )
    .unwrap()
}

fn session() -> InspectionSession {
    InspectionSession::new(SessionId::new(1).unwrap(), UnixMillis(0))
}

fn fixture() -> (InspectionSession, PartSpecification, InspectionItem, Bundle) {
    let session = session();
    let specification = specification();
    let item =
        InspectionItem::new(InspectionItemId::new(1).unwrap(), &session, &specification).unwrap();
    let bundle = Bundle::start(
        BundleId::new(1).unwrap(),
        &session,
        &item,
        Sequence::new(1).unwrap(),
        "Bundle A".into(),
        UnixMillis(1),
    )
    .unwrap();
    (session, specification, item, bundle)
}

fn full_draft(id: i64, sequence: i64, text: &str) -> MeasurementDraft {
    MeasurementDraft {
        id: MeasurementId::new(id).unwrap(),
        sequence: Sequence::new(sequence).unwrap(),
        input: MeasurementInput::full_length(format!("={text}")).unwrap(),
        interpreted: InterpretedMeasurement {
            value: length(text),
            method: InterpretationMethod::FullLength,
        },
        captured_at: UnixMillis(2),
        provenance: MeasurementProvenance::default(),
    }
}

fn measurement(text: &str) -> Measurement {
    let (session, specification, item, bundle) = fixture();
    Measurement::new(
        &CaptureContext {
            session: &session,
            specification: &specification,
            item: &item,
            bundle: &bundle,
        },
        full_draft(1, 1, text),
    )
    .unwrap()
}

#[test]
fn fixed_point_parsing_is_exact_and_preserves_the_full_i64_range() {
    assert_eq!(length("9368.9").nanometres(), 9_368_900_000);
    assert_eq!(length("0.000001").nanometres(), 1);
    assert_eq!(length("9223372036854.775807").nanometres(), i64::MAX);
    assert_eq!(length("0009368.900000").to_string(), "9368.9");
    assert_eq!(length("0").deviation_from(length("1")), -1_000_000);
    assert_eq!(
        Length::parse_mm("9223372036854.775808"),
        Err(DomainError::NumericOverflow)
    );
    assert_eq!(
        Length::parse_mm("9223372036855"),
        Err(DomainError::NumericOverflow)
    );
    assert_eq!(
        Length::parse_mm("1.0000000"),
        Err(DomainError::PrecisionExceeded)
    );
    assert_eq!(
        Length::from_nanometres(-1),
        Err(DomainError::NegativeLength)
    );
}

#[test]
fn malformed_dimensions_are_not_normalized_or_rounded() {
    for raw in [
        "", " ", " 1", "1 ", ".1", "1.", "-1", "+1", "1e3", "1,1", "NaN", "inf", "١", "1.2.3",
    ] {
        assert!(Length::parse_mm(raw).is_err(), "accepted {raw:?}");
    }
}

#[test]
fn exact_display_round_trips_a_bounded_reference_set() {
    for nm in [0, 1, 10, 999_999, 1_000_000, 9_368_900_000, i64::MAX] {
        let value = Length::from_nanometres(nm).unwrap();
        assert_eq!(Length::parse_mm(&value.to_string()).unwrap(), value);
    }
}

#[test]
fn identity_types_and_part_numbers_do_not_erase_information() {
    assert!(SessionId::new(0).is_err());
    assert!(SessionId::new(-1).is_err());
    assert!(Sequence::new(0).is_err());
    assert!(SpecificationVersion::new(0).is_err());
    assert_eq!(SessionId::new(i64::MAX).unwrap().get(), i64::MAX);
    assert_eq!(
        PartNumber::new("00405363".into()).unwrap().as_str(),
        "00405363"
    );
    assert!(PartNumber::new("  ".into()).is_err());
    assert!(RequestId::new("".into()).is_err());
}

#[test]
fn specification_requires_ordered_limits_and_nominal_inside_them() {
    let make = |nominal, lsl, usl| {
        PartSpecification::new(
            SpecificationId::new(2).unwrap(),
            PartNumber::new("A".into()).unwrap(),
            SpecificationVersion::new(2).unwrap(),
            length(nominal),
            length(lsl),
            length(usl),
            Resolution::new(length("0.1")).unwrap(),
        )
    };
    assert!(make("2", "1", "1").is_err());
    assert!(make("2", "3", "1").is_err());
    assert!(make("4", "1", "3").is_err());
    assert!(make("0", "1", "3").is_err());
    assert!(make("1", "1", "3").is_ok());
    assert!(Resolution::new(length("0")).is_err());
}

#[test]
fn inclusive_limits_do_not_use_display_rounding() {
    let spec = specification();
    assert_eq!(spec.classify(spec.lsl()), Conformance::WithinLimits);
    assert_eq!(spec.classify(spec.usl()), Conformance::WithinLimits);
    assert_eq!(spec.classify(length("9363.8")), Conformance::BelowLimit);
    assert_eq!(spec.classify(length("9374.0")), Conformance::AboveLimit);
    assert_eq!(
        spec.classify(length("9363.899999")),
        Conformance::BelowLimit
    );
}

#[test]
fn input_modes_preserve_original_spelling_and_reject_ambiguous_grammar() {
    assert_eq!(
        MeasurementInput::abbreviated("01".into()).unwrap().raw(),
        "01"
    );
    let full = MeasurementInput::full_length("=0009368.900000".into()).unwrap();
    assert_eq!(full.raw(), "=0009368.900000");
    assert_eq!(full.mode(), InputMode::FullLength);
    for raw in ["", "1", "123", " 68", "６８", "-1", "6."] {
        assert!(MeasurementInput::abbreviated(raw.into()).is_err());
    }
    assert!(MeasurementInput::full_length("9368".into()).is_err());
    assert!(MeasurementInput::full_length("=1.0000001".into()).is_err());
}

#[test]
fn claimed_abbreviation_result_requires_matching_suffix_and_policy() {
    let policy = ReconstructionPolicy::new(
        "integer-mm-window".into(),
        PolicyVersion::new(1).unwrap(),
        length("9300"),
        length("9399"),
    )
    .unwrap();
    let input = MeasurementInput::abbreviated("68".into()).unwrap();
    let mut interpreted = InterpretedMeasurement {
        value: length("9368"),
        method: InterpretationMethod::IntegerMillimetreSuffix {
            policy,
            operator_selected: false,
        },
    };
    assert!(interpreted.validate(&input).is_ok());
    interpreted.value = length("9368.9");
    assert_eq!(
        interpreted.validate(&input),
        Err(DomainError::InterpretationMismatch)
    );
    interpreted.value = length("9468");
    assert!(interpreted.validate(&input).is_err());
    assert!(
        ReconstructionPolicy::new(
            "p".into(),
            PolicyVersion::new(1).unwrap(),
            length("10"),
            length("1")
        )
        .is_err()
    );
    // No candidate-selection algorithm is exercised or claimed here.
}

#[test]
fn precision_mismatch_is_an_error_but_out_of_spec_is_recordable() {
    let (session, spec, item, bundle) = fixture();
    let context = CaptureContext {
        session: &session,
        specification: &spec,
        item: &item,
        bundle: &bundle,
    };
    assert_eq!(
        Measurement::new(&context, full_draft(1, 1, "9368.91")),
        Err(DomainError::ResolutionMismatch)
    );
    let outside = Measurement::new(&context, full_draft(1, 1, "9380")).unwrap();
    assert_eq!(
        spec.classify(outside.interpreted().value),
        Conformance::AboveLimit
    );
    assert_eq!(outside.input().raw(), "=9380");
    assert_eq!(outside.provenance(), &MeasurementProvenance::default());
    let mut wrong = full_draft(2, 2, "9368");
    wrong.interpreted.value = length("9369");
    assert_eq!(
        Measurement::new(&context, wrong),
        Err(DomainError::InterpretationMismatch)
    );
}

#[test]
fn session_close_requires_active_state_and_no_open_bundles() {
    let mut session = session();
    let before = session.clone();
    assert_eq!(
        session.transition(SessionTransition::Close, 1, UnixMillis(3)),
        Err(DomainError::OpenBundlesRemain)
    );
    assert_eq!(session, before);
    assert!(
        session
            .transition(SessionTransition::Resume, 0, UnixMillis(3))
            .is_err()
    );
    session
        .transition(SessionTransition::Suspend, 1, UnixMillis(3))
        .unwrap();
    assert_eq!(session.status(), SessionStatus::Suspended);
    assert!(
        session
            .transition(SessionTransition::Close, 0, UnixMillis(4))
            .is_err()
    );
    session
        .transition(SessionTransition::Resume, 1, UnixMillis(4))
        .unwrap();
    session
        .transition(SessionTransition::Close, 0, UnixMillis(5))
        .unwrap();
    assert_eq!(session.closed_at(), Some(UnixMillis(5)));
    assert!(
        session
            .transition(SessionTransition::Resume, 0, UnixMillis(6))
            .is_err()
    );
}

#[test]
fn suspended_or_closed_context_rejects_capture_without_changing_originals() {
    let (mut session, spec, item, mut bundle) = fixture();
    session
        .transition(SessionTransition::Suspend, 1, UnixMillis(3))
        .unwrap();
    assert!(
        Measurement::new(
            &CaptureContext {
                session: &session,
                item: &item,
                bundle: &bundle,
                specification: &spec
            },
            full_draft(1, 1, "9368")
        )
        .is_err()
    );
    assert!(
        bundle
            .close(&session, UnixMillis(4), BundleCapacity::new(1).unwrap())
            .is_err()
    );
    assert!(
        Bundle::start(
            BundleId::new(2).unwrap(),
            &session,
            &item,
            Sequence::new(2).unwrap(),
            "B".into(),
            UnixMillis(4)
        )
        .is_err()
    );
    session
        .transition(SessionTransition::Resume, 1, UnixMillis(4))
        .unwrap();
    bundle
        .close(&session, UnixMillis(5), BundleCapacity::new(1).unwrap())
        .unwrap();
    assert!(
        bundle
            .close(&session, UnixMillis(5), BundleCapacity::new(1).unwrap())
            .is_err()
    );
    assert!(
        Measurement::new(
            &CaptureContext {
                session: &session,
                item: &item,
                bundle: &bundle,
                specification: &spec
            },
            full_draft(1, 1, "9368")
        )
        .is_err()
    );
}

#[test]
fn multiple_parts_and_bundles_have_independent_ownership() {
    let (session, _spec, item, first) = fixture();
    let second = Bundle::start(
        BundleId::new(2).unwrap(),
        &session,
        &item,
        Sequence::new(2).unwrap(),
        "B".into(),
        UnixMillis(3),
    )
    .unwrap();
    assert_ne!(first.id(), second.id());
    let other_spec = PartSpecification::new(
        SpecificationId::new(2).unwrap(),
        PartNumber::new("OTHER".into()).unwrap(),
        SpecificationVersion::new(1).unwrap(),
        length("10"),
        length("9"),
        length("11"),
        Resolution::new(length("1")).unwrap(),
    )
    .unwrap();
    let other_item =
        InspectionItem::new(InspectionItemId::new(2).unwrap(), &session, &other_spec).unwrap();
    assert_eq!(item.session_id(), other_item.session_id());
    assert_ne!(item.part_number(), other_item.part_number());
    let context = CaptureContext {
        session: &session,
        item: &other_item,
        bundle: &first,
        specification: &other_spec,
    };
    assert_eq!(context.validate(), Err(DomainError::AssociationMismatch));
    let other_session = InspectionSession::new(SessionId::new(2).unwrap(), UnixMillis(0));
    assert!(
        Bundle::start(
            BundleId::new(3).unwrap(),
            &other_session,
            &item,
            Sequence::new(3).unwrap(),
            "C".into(),
            UnixMillis(4)
        )
        .is_err()
    );
}

fn correction(
    target: &Measurement,
    previous: Option<&MeasurementCorrection>,
    id: i64,
    action: CorrectionAction,
) -> MeasurementCorrection {
    MeasurementCorrection::new(
        CorrectionId::new(id).unwrap(),
        target,
        &specification(),
        previous,
        CorrectionDraft {
            action,
            reason: "Operator transcription correction".into(),
            actor: None,
            recorded_at: UnixMillis(4),
        },
    )
    .unwrap()
}

#[test]
fn corrections_preserve_original_identity_order_and_input() {
    let original = measurement("9368");
    let replacement = CorrectionAction::Replace {
        input: MeasurementInput::full_length("=9369".into()).unwrap(),
        interpreted: InterpretedMeasurement {
            value: length("9369"),
            method: InterpretationMethod::FullLength,
        },
    };
    let first = correction(&original, None, 1, replacement);
    let second = correction(&original, Some(&first), 2, CorrectionAction::Invalidate);
    let history = MeasurementHistory::new(original.clone(), vec![first.clone()]).unwrap();
    assert_eq!(history.status(), MeasurementStatus::Corrected);
    assert_eq!(history.effective().unwrap().1.value, length("9369"));
    assert_eq!(history.original(), &original);
    assert_eq!(history.original().input().raw(), "=9368");
    let invalidated =
        MeasurementHistory::new(original.clone(), vec![first.clone(), second.clone()]).unwrap();
    assert_eq!(invalidated.status(), MeasurementStatus::Invalidated);
    assert!(invalidated.effective().is_none());
    assert_eq!(invalidated.original().sequence(), original.sequence());
    assert!(MeasurementHistory::new(original.clone(), vec![second]).is_err());
    assert!(MeasurementHistory::new(original, vec![first.clone(), first]).is_err());
}

#[test]
fn correction_requires_reason_and_exact_resolution() {
    let original = measurement("9368");
    let mut draft = CorrectionDraft {
        action: CorrectionAction::Invalidate,
        reason: " ".into(),
        actor: None,
        recorded_at: UnixMillis(3),
    };
    assert!(
        MeasurementCorrection::new(
            CorrectionId::new(1).unwrap(),
            &original,
            &specification(),
            None,
            draft.clone()
        )
        .is_err()
    );
    draft.reason = "Correction".into();
    draft.action = CorrectionAction::Replace {
        input: MeasurementInput::full_length("=9368.91".into()).unwrap(),
        interpreted: InterpretedMeasurement {
            value: length("9368.91"),
            method: InterpretationMethod::FullLength,
        },
    };
    assert!(
        MeasurementCorrection::new(
            CorrectionId::new(1).unwrap(),
            &original,
            &specification(),
            None,
            draft
        )
        .is_err()
    );
}

#[test]
fn snapshot_validation_allows_gaps_but_not_reordering_or_duplicates() {
    let (session, specification, item, bundle) = fixture();
    let context = CaptureContext {
        session: &session,
        specification: &specification,
        item: &item,
        bundle: &bundle,
    };
    let first = MeasurementHistory::new(
        Measurement::new(&context, full_draft(1, 1, "9368")).unwrap(),
        vec![],
    )
    .unwrap();
    let third = MeasurementHistory::new(
        Measurement::new(&context, full_draft(3, 3, "9368")).unwrap(),
        vec![],
    )
    .unwrap();
    let mut snapshot = InspectionSnapshot {
        revision: SnapshotRevision::new(1).unwrap(),
        session,
        specification,
        item,
        bundles: vec![SnapshotBundle {
            bundle,
            measurements: vec![first.clone(), third],
        }],
    };
    assert!(snapshot.validate().is_ok());
    snapshot.bundles[0].measurements.reverse();
    assert!(snapshot.validate().is_err());
    snapshot.bundles[0].measurements = vec![first.clone(), first];
    assert_eq!(snapshot.validate(), Err(DomainError::DuplicateIdentity));
}

#[test]
fn historical_restore_does_not_require_an_active_capture_context() {
    let spec = specification();
    let session = InspectionSession::restore(
        SessionId::new(1).unwrap(),
        SessionStatus::Closed,
        UnixMillis(0),
        Some(UnixMillis(9)),
    )
    .unwrap();
    let item = InspectionItem::restore(InspectionItemId::new(1).unwrap(), &session, &spec);
    assert!(InspectionItem::new(InspectionItemId::new(2).unwrap(), &session, &spec).is_err());
    let record = BundleRecord {
        id: BundleId::new(1).unwrap(),
        sequence: Sequence::new(1).unwrap(),
        label: "Historical bundle".into(),
        status: BundleStatus::Closed,
        started_at: UnixMillis(1),
        closed_at: Some(UnixMillis(8)),
    };
    let bundle = Bundle::restore(record.clone(), &session, &item).unwrap();
    let context = CaptureContext {
        session: &session,
        item: &item,
        bundle: &bundle,
        specification: &spec,
    };
    let restored = Measurement::restore(&context, full_draft(1, 1, "9368")).unwrap();
    assert_eq!(restored.input().raw(), "=9368");
    assert!(Measurement::new(&context, full_draft(2, 2, "9369")).is_err());
    let mut inconsistent = record;
    inconsistent.status = BundleStatus::Open;
    assert!(Bundle::restore(inconsistent.clone(), &session, &item).is_err());
    inconsistent.closed_at = None;
    assert!(Bundle::restore(inconsistent, &session, &item).is_err());
    assert!(
        InspectionSession::restore(session.id(), SessionStatus::Closed, UnixMillis(0), None)
            .is_err()
    );
    assert!(
        InspectionSession::restore(
            session.id(),
            SessionStatus::Active,
            UnixMillis(0),
            Some(UnixMillis(9))
        )
        .is_err()
    );
}

#[test]
fn explicit_replacement_after_invalidation_restores_an_effective_value() {
    let original = measurement("9368");
    let first = correction(&original, None, 1, CorrectionAction::Invalidate);
    let action = CorrectionAction::Replace {
        input: MeasurementInput::full_length("=9369".into()).unwrap(),
        interpreted: InterpretedMeasurement {
            value: length("9369"),
            method: InterpretationMethod::FullLength,
        },
    };
    let second = correction(&original, Some(&first), 2, action);
    let history = MeasurementHistory::new(original, vec![first, second]).unwrap();
    assert_eq!(history.status(), MeasurementStatus::Corrected);
    assert_eq!(history.effective().unwrap().1.value, length("9369"));
    assert_eq!(history.corrections().len(), 2);
}
