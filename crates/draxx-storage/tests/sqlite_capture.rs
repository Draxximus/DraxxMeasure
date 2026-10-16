use draxx_core::*;
use draxx_storage::*;
use rusqlite::Connection;
use std::sync::{Arc, Barrier};

fn length(text: &str) -> Length {
    Length::parse_mm(text).unwrap()
}
fn request(text: impl Into<String>) -> RequestId {
    RequestId::new(text.into()).unwrap()
}
struct Fixture {
    directory: tempfile::TempDir,
    store: SqliteStore,
    session: InspectionSession,
    item: InspectionItem,
    bundle: Bundle,
    spec: PartSpecification,
    policy: ReconstructionPolicy,
}
impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let mut store = SqliteStore::open(directory.path().join("inspection.sqlite3")).unwrap();
        let part = PartNumber::new("00405363".into()).unwrap();
        store.add_part(request("part"), part.clone()).unwrap();
        let spec = store
            .add_specification(
                request("spec"),
                CreateSpecification {
                    part_number: part,
                    version: SpecificationVersion::new(1).unwrap(),
                    nominal: length("9368.9"),
                    lsl: length("9363.9"),
                    usl: length("9373.9"),
                    resolution: Resolution::new(length("0.1")).unwrap(),
                },
            )
            .unwrap()
            .value;
        let session = store
            .create_session(request("session"), UnixMillis(0))
            .unwrap()
            .value;
        let item = store
            .add_item(request("item"), session.id(), spec.id())
            .unwrap()
            .value;
        let policy = ReconstructionPolicy::new(
            SUFFIX_POLICY_NAME.into(),
            PolicyVersion::new(1).unwrap(),
            length("9300"),
            length("9399"),
        )
        .unwrap();
        store
            .configure_policy(
                request("policy"),
                item.id(),
                Some(policy.clone()),
                UnixMillis(1),
            )
            .unwrap();
        let bundle = store
            .start_bundle(request("bundle"), item.id(), "A".into(), UnixMillis(2))
            .unwrap()
            .value;
        Self {
            directory,
            store,
            session,
            item,
            bundle,
            spec,
            policy,
        }
    }
    fn command(&self, token: &str, raw: &str) -> RecordMeasurement {
        let input = if raw.starts_with('=') {
            MeasurementInput::full_length(raw.into()).unwrap()
        } else {
            MeasurementInput::abbreviated(raw.into()).unwrap()
        };
        let ReconstructionOutcome::Unique(interpreted) = SuffixReconstructor
            .reconstruct(&input, &self.spec, Some(&self.policy))
            .unwrap()
        else {
            panic!("fixture input not unique")
        };
        RecordMeasurement {
            request_id: request(token),
            session_id: self.session.id(),
            item_id: self.item.id(),
            bundle_id: self.bundle.id(),
            input,
            interpreted,
            captured_at: UnixMillis(3),
            provenance: MeasurementProvenance::default(),
        }
    }
    fn path(&self) -> std::path::PathBuf {
        self.directory.path().join("inspection.sqlite3")
    }
}

#[test]
fn forty_third_active_measurement_is_never_inserted_even_when_bypassing_the_cli() {
    let mut f = Fixture::new();
    for index in 1..=42 {
        let command = f.command(&format!("m-{index}"), "68");
        f.store.record_measurement(command).unwrap();
    }
    let before = f.store.snapshot(f.item.id()).unwrap();
    let rejected = f.command("m-43", "71");
    assert!(matches!(
        f.store.record_measurement(rejected),
        Err(StorageError::Domain(DomainError::BundleFull))
    ));
    let after = f.store.snapshot(f.item.id()).unwrap();
    assert_eq!(before.revision, after.revision);
    assert_eq!(after.bundles[0].measurements.len(), 42);
    assert_eq!(after.bundles[0].bundle.status(), BundleStatus::Open);
    assert!(f.store.lookup_request(&request("m-43")).unwrap().is_none());
    // Same acknowledged command is a retry, not an extra physical tube.
    let replay = f.command("m-42", "68");
    assert!(f.store.record_measurement(replay).unwrap().replayed);
    let connection = Connection::open(f.path()).unwrap();
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .unwrap();
    assert!(connection.execute("INSERT INTO measurements SELECT id+100,session_id,item_id,bundle_id,specification_id,43,raw_input,value_nm,payload,captured_at,instrument_id,calibration_reference,operator,created_revision FROM measurements WHERE id=1",[]).is_err());
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM measurements", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        42
    );
    f.store
        .close_bundle(request("close-full"), f.bundle.id(), UnixMillis(4))
        .unwrap();
}

#[test]
fn undo_frees_one_slot_but_restoring_it_cannot_create_forty_three_active_values() {
    let mut f = Fixture::new();
    assert!(matches!(
        f.store
            .close_bundle(request("empty-close"), f.bundle.id(), UnixMillis(3)),
        Err(StorageError::Domain(DomainError::EmptyBundle))
    ));
    let first = f.command("m-1", "68");
    let first = f.store.record_measurement(first).unwrap().value;
    for i in 2..=42 {
        let c = f.command(&format!("m-{i}"), "68");
        f.store.record_measurement(c).unwrap();
    }
    let undo = f
        .store
        .correct_measurement(CorrectMeasurement {
            request_id: request("undo"),
            measurement_id: first.id(),
            expected_previous: None,
            action: CorrectionAction::Invalidate,
            reason: "Operator undo".into(),
            actor: None,
            recorded_at: UnixMillis(4),
        })
        .unwrap()
        .value;
    assert_eq!(f.store.bundle_capacity(f.bundle.id()).unwrap().active(), 41);
    let extra = f.command("after-undo", "71");
    let extra = f.store.record_measurement(extra).unwrap().value;
    assert_eq!(extra.sequence().get(), 43);
    let command = f.command("unused", "67");
    assert!(matches!(
        f.store.correct_measurement(CorrectMeasurement {
            request_id: request("restore-full"),
            measurement_id: first.id(),
            expected_previous: Some(undo.id()),
            action: CorrectionAction::Replace {
                input: command.input,
                interpreted: command.interpreted
            },
            reason: "Restoring transcription".into(),
            actor: None,
            recorded_at: UnixMillis(5)
        }),
        Err(StorageError::Domain(DomainError::BundleFull))
    ));
    let snapshot = f.store.snapshot(f.item.id()).unwrap();
    assert_eq!(snapshot.bundles[0].measurements.len(), 43);
    assert_eq!(
        snapshot.bundles[0].measurements[0].original().input().raw(),
        "68"
    );
    assert_eq!(
        snapshot.bundles[0].measurements[0].status(),
        MeasurementStatus::Invalidated
    );
    assert_eq!(f.store.bundle_capacity(f.bundle.id()).unwrap().active(), 42);
}

#[test]
fn reopened_database_retains_context_order_raw_input_and_corrections() {
    let mut f = Fixture::new();
    let command = f.command("a", "=9363.9");
    let measurement = f.store.record_measurement(command.clone()).unwrap().value;
    let value = f.command("b", "=9380");
    f.store
        .correct_measurement(CorrectMeasurement {
            request_id: request("correct"),
            measurement_id: measurement.id(),
            expected_previous: None,
            action: CorrectionAction::Replace {
                input: value.input,
                interpreted: value.interpreted,
            },
            reason: "Correct transcription".into(),
            actor: Some("Inspector A".into()),
            recorded_at: UnixMillis(4),
        })
        .unwrap();
    let path = f.path();
    drop(f.store);
    let mut store = SqliteStore::open(&path).unwrap();
    let state = store.recover_session(f.session.id()).unwrap();
    assert_eq!(state.active_item, Some(f.item.id()));
    assert_eq!(state.open_bundle.unwrap().id(), f.bundle.id());
    let snapshot = store.snapshot(f.item.id()).unwrap();
    let history = &snapshot.bundles[0].measurements[0];
    assert_eq!(history.original().input().raw(), "=9363.9");
    assert_eq!(history.effective().unwrap().1.value, length("9380"));
    assert_eq!(history.corrections()[0].actor(), Some("Inspector A"));
    assert!(store.record_measurement(command.clone()).unwrap().replayed);
    let mut conflicting = command;
    conflicting.captured_at = UnixMillis(99);
    assert!(matches!(
        store.record_measurement(conflicting),
        Err(StorageError::Conflict)
    ));
    let connection = Connection::open(path).unwrap();
    assert_eq!(
        connection
            .query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "wal"
    );
    assert!(connection.execute("DELETE FROM measurements", []).is_err());
    assert!(
        connection
            .execute("UPDATE corrections SET reason='changed'", [])
            .is_err()
    );
}

#[test]
fn failed_write_rolls_back_measurement_sequence_and_commit_record() {
    let mut f = Fixture::new();
    let connection = Connection::open(f.path()).unwrap();
    connection.execute_batch("CREATE TRIGGER injected_failure BEFORE INSERT ON measurements BEGIN SELECT RAISE(ABORT,'injected disk write failure'); END;").unwrap();
    let before = f.store.snapshot(f.item.id()).unwrap().revision;
    let command = f.command("retry-me", "68");
    assert!(f.store.record_measurement(command.clone()).is_err());
    assert!(
        f.store
            .lookup_request(&command.request_id)
            .unwrap()
            .is_none()
    );
    assert_eq!(f.store.snapshot(f.item.id()).unwrap().revision, before);
    connection
        .execute_batch("DROP TRIGGER injected_failure;")
        .unwrap();
    let saved = f.store.record_measurement(command).unwrap();
    assert_eq!(saved.value.sequence().get(), 1);
}

#[test]
fn competing_connections_cannot_overfill_the_last_slot() {
    let mut f = Fixture::new();
    for i in 1..=41 {
        let c = f.command(&format!("m-{i}"), "68");
        f.store.record_measurement(c).unwrap();
    }
    let commands = [
        f.command("concurrent-a", "68"),
        f.command("concurrent-b", "71"),
    ];
    let barrier = Arc::new(Barrier::new(2));
    let mut threads = Vec::new();
    for command in commands {
        let path = f.path();
        let barrier = barrier.clone();
        threads.push(std::thread::spawn(move || {
            let mut store = SqliteStore::open(path).unwrap();
            barrier.wait();
            store.record_measurement(command)
        }));
    }
    let results: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(f.store.bundle_capacity(f.bundle.id()).unwrap().active(), 42);
    assert_eq!(
        f.store.snapshot(f.item.id()).unwrap().bundles[0]
            .measurements
            .len(),
        42
    );
}

#[test]
fn closure_seals_sessions_and_last_active_closed_bundle_value_cannot_be_invalidated() {
    let mut f = Fixture::new();
    let c = f.command("m-1", "68");
    let measurement = f.store.record_measurement(c).unwrap().value;
    f.store
        .close_bundle(request("close"), f.bundle.id(), UnixMillis(5))
        .unwrap();
    assert!(matches!(
        f.store.correct_measurement(CorrectMeasurement {
            request_id: request("invalidate-closed"),
            measurement_id: measurement.id(),
            expected_previous: None,
            action: CorrectionAction::Invalidate,
            reason: "test".into(),
            actor: None,
            recorded_at: UnixMillis(6)
        }),
        Err(StorageError::Domain(DomainError::EmptyBundle))
    ));
    f.store
        .select_item(request("exit-part"), f.session.id(), None, UnixMillis(6))
        .unwrap();
    let close_request = request("close-session");
    let closed = f
        .store
        .transition_session(
            close_request.clone(),
            f.session.id(),
            SessionTransition::Close,
            UnixMillis(7),
        )
        .unwrap();
    assert_eq!(closed.value.status(), SessionStatus::Closed);
    // Lifecycle replay returns the original state at that event, not today's state.
    let replay = f
        .store
        .create_session(request("session"), UnixMillis(0))
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.value.status(), SessionStatus::Active);
    assert!(
        f.store
            .transition_session(
                request("reopen"),
                f.session.id(),
                SessionTransition::Resume,
                UnixMillis(8)
            )
            .is_err()
    );
    let c = f.command("new-after-close", "68");
    assert!(f.store.record_measurement(c).is_err());
}

#[test]
fn migrations_refuse_newer_or_unversioned_nonempty_databases_without_erasing_them() {
    let f = Fixture::new();
    let path = f.path();
    drop(f.store);
    let connection = Connection::open(&path).unwrap();
    connection.pragma_update(None, "user_version", 2).unwrap();
    drop(connection);
    assert!(matches!(
        SqliteStore::open(&path),
        Err(StorageError::UnsupportedSchema { found: 2, .. })
    ));
    let connection = Connection::open(path).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM part_numbers", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("unknown.sqlite3");
    let connection = Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE important(value TEXT); INSERT INTO important VALUES('preserve');",
        )
        .unwrap();
    drop(connection);
    assert!(SqliteStore::open(&path).is_err());
    let connection = Connection::open(path).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT value FROM important", [], |r| r.get::<_, String>(0))
            .unwrap(),
        "preserve"
    );
}

#[test]
fn bundle_count_is_dynamic_and_sequences_remain_per_item() {
    let mut f = Fixture::new();
    for index in 1..=45 {
        let command = f.command(&format!("tube-{index}"), "68");
        f.store.record_measurement(command).unwrap();
        f.store
            .close_bundle(
                request(format!("close-{index}")),
                f.bundle.id(),
                UnixMillis(index + 5),
            )
            .unwrap();
        if index < 45 {
            f.bundle = f
                .store
                .start_bundle(
                    request(format!("bundle-{index}")),
                    f.item.id(),
                    format!("Bundle {}", index + 1),
                    UnixMillis(index + 6),
                )
                .unwrap()
                .value;
        }
    }
    let snapshot = f.store.snapshot(f.item.id()).unwrap();
    assert_eq!(snapshot.bundles.len(), 45);
    assert_eq!(snapshot.bundles[44].bundle.sequence().get(), 45);
    assert!(
        snapshot
            .bundles
            .iter()
            .all(|b| b.measurements[0].original().sequence().get() == 1)
    );
}

#[test]
fn foreign_keys_and_domain_context_prevent_cross_session_capture() {
    let mut f = Fixture::new();
    let other = f
        .store
        .create_session(request("other-session"), UnixMillis(1))
        .unwrap()
        .value;
    let mut command = f.command("wrong-session", "68");
    command.session_id = other.id();
    assert!(matches!(
        f.store.record_measurement(command),
        Err(StorageError::Domain(DomainError::AssociationMismatch))
    ));
    assert_eq!(f.store.bundle_capacity(f.bundle.id()).unwrap().active(), 0);
    let connection = Connection::open(f.path()).unwrap();
    connection
        .pragma_update(None, "foreign_keys", "ON")
        .unwrap();
    assert!(
        connection
            .execute(
                "INSERT INTO session_items VALUES (99,999,1,'00405363',1)",
                []
            )
            .is_err()
    );
}
