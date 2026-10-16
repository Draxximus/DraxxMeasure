use draxx_application::*;
use draxx_core::*;
use draxx_storage::*;

#[test]
fn failed_submission_retains_raw_input_and_retries_the_original_request_once() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("data.sqlite3");
    let mut service = InspectionService::new(SqliteStore::open(&path).unwrap());
    service.start_session().unwrap();
    let length = |value| Length::parse_mm(value).unwrap();
    service
        .create_part(PartSetup {
            part_number: PartNumber::new("SYNTHETIC".into()).unwrap(),
            nominal: length("9368.9"),
            tolerance: length("5"),
            resolution: Resolution::new(length("0.1")).unwrap(),
            policy: Some(
                ReconstructionPolicy::new(
                    SUFFIX_POLICY_NAME.into(),
                    PolicyVersion::new(1).unwrap(),
                    length("9300"),
                    length("9399"),
                )
                .unwrap(),
            ),
        })
        .unwrap();
    service.start_bundle().unwrap();
    let connection = rusqlite::Connection::open(path).unwrap();
    connection.execute_batch("CREATE TRIGGER injected_failure BEFORE INSERT ON measurements BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
    assert!(service.record("68".into()).is_err());
    assert_eq!(service.pending_input(), Some("68"));
    assert!(matches!(
        service.record("71".into()),
        Err(ApplicationError::PendingWrite)
    ));
    assert!(matches!(
        service.close(),
        Err(ApplicationError::PendingWrite)
    ));
    assert!(service.retry().is_err());
    assert_eq!(service.state().unwrap().unwrap().active_count, 0);
    connection
        .execute_batch("DROP TRIGGER injected_failure;")
        .unwrap();
    let receipt = service.retry().unwrap();
    assert_eq!(receipt.value.input().raw(), "68");
    assert_eq!(receipt.value.sequence().get(), 1);
    assert!(service.pending_input().is_none());
    assert_eq!(service.state().unwrap().unwrap().active_count, 1);
    assert!(service.retry().is_err());
}
