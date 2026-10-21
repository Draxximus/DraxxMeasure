use draxx_cli::{CliOptions, run};
use draxx_core::*;
use draxx_storage::*;
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::Path,
    process::{Command, Stdio},
    sync::mpsc,
    time::Duration,
};

const SETUP: &str = "/session\n/partnumber\n405363\n9368.9\n5\n0.1\nyes\n/bundle\n";
fn execute(database: &Path, reports: &Path, input: &str) -> String {
    let mut output = Vec::new();
    run(
        CliOptions {
            database: database.into(),
            reports_directory: reports.into(),
            report_notes: vec!["SYNTHETIC TEST - NOT PRODUCTION DATA".into()],
        },
        &mut input.as_bytes(),
        &mut output,
    )
    .unwrap();
    String::from_utf8(output).unwrap()
}
fn assert_png(path: &Path) {
    let image = image::open(path).unwrap().to_rgb8();
    assert_eq!(image.dimensions(), (1800, 2000));
    for (from, to) in [
        (0, 280),
        (280, 700),
        (700, 1500),
        (1530, 1870),
        (1870, 2000),
    ] {
        let nonwhite = (from..to)
            .flat_map(|y| (0..1800).map(move |x| (x, y)))
            .filter(|&(x, y)| image.get_pixel(x, y).0 != [255, 255, 255])
            .count();
        assert!(nonwhite > 200, "empty expected report panel {from}..{to}");
    }
}

#[test]
fn complete_multi_part_workflow_rejects_43_and_exports_independently_verifiable_data() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("data.sqlite3");
    let reports = directory.path().join("reports");
    let input = format!(
        "{SETUP}{}71\n/close\n/close\n/partnumber\n413348\n10608.9\n5\n0.1\nyes\n/bundle\n09\n12\n/close\n/close\n/close\n/exit\n",
        "68\n".repeat(42)
    );
    let output = execute(&database, &reports, &input);
    assert!(output.contains("saved [42/42]"));
    assert!(output.contains("bundle is full"));
    assert!(!output.contains("saved [43/42]"));
    assert!(output.contains("✓ 10609 mm saved [1/42]"));
    assert!(output.contains("✓ Session closed"));
    assert_eq!(
        output.matches("✓ PNG report generated (1 page(s))").count(),
        2
    );
    assert_png(&reports.join("session-001-part-405363-item-001.png"));
    assert_png(&reports.join("session-001-part-413348-item-002.png"));
    let store = SqliteStore::open(&database).unwrap();
    let snapshots = store.session_snapshots(SessionId::new(1).unwrap()).unwrap();
    assert_eq!(snapshots.len(), 2);
    assert_eq!(snapshots[0].bundles[0].measurements.len(), 42);
    assert_eq!(snapshots[1].bundles[0].measurements.len(), 2);
    assert!(
        snapshots
            .iter()
            .all(|s| s.session.status() == SessionStatus::Closed)
    );
    assert!(output.find("PNG report generated").unwrap() < output.find("Session closed").unwrap());
    for stem in [
        "session-001-part-405363-item-001",
        "session-001-part-413348-item-002",
    ] {
        let manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(reports.join(format!("{stem}.manifest.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["complete"], true);
        assert_eq!(manifest["report_scope"], "part_number");
        assert_eq!(manifest["items"].as_array().unwrap().len(), 1);
        let item = &manifest["items"][0];
        assert_eq!(item["session_status"], "Active");
        let id = item["item_id"].as_i64().unwrap();
        let mut reader =
            csv::Reader::from_path(reports.join(item["csv_file"].as_str().unwrap())).unwrap();
        let headers = reader.headers().unwrap().clone();
        let column = |name: &str| headers.iter().position(|value| value == name).unwrap();
        let records: Vec<_> = reader.records().map(Result::unwrap).collect();
        let effective: Vec<_> = records
            .iter()
            .filter(|row| {
                &row[column("row_kind")] == "effective" && !row[column("value_mm")].is_empty()
            })
            .collect();
        let values: Vec<f64> = effective
            .iter()
            .map(|row| row[column("value_mm")].parse().unwrap())
            .collect();
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        let sd = (values
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / (values.len() - 1) as f64)
            .sqrt();
        assert_eq!(
            values.len() as u64,
            item["measurement_count"].as_u64().unwrap()
        );
        assert!((mean - item["mean_mm"].as_f64().unwrap()).abs() < 1e-9);
        assert!((sd - item["sample_standard_deviation_mm"].as_f64().unwrap()).abs() < 1e-9);
        assert_eq!(
            item["histogram_counts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_u64().unwrap())
                .sum::<u64>(),
            values.len() as u64
        );
        if id == 2 {
            assert!(
                effective
                    .iter()
                    .any(|row| &row[column("raw_input")] == "09")
            );
        }
    }
}

#[test]
fn malformed_input_ambiguity_undo_and_correction_preserve_originals_and_context() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("data.sqlite3");
    let reports = directory.path().join("reports");
    let input = format!(
        "{SETUP}abc\n 68\n000\n68\n71\n/policy 9300 9499\n68\n/correct 1 =9369.0\nTranscription, with \"quotes\"\n/undo\nyes\nOperator undo\n/measurements\n/close\n/close\n/close\n/exit\n"
    );
    let output = execute(&database, &reports, &input);
    assert!(output.contains("ambiguous input"));
    assert!(output.contains("original retained"));
    assert!(output.contains("405363/bundle-1>"));
    let store = SqliteStore::open(database).unwrap();
    let snapshots = store.session_snapshots(SessionId::new(1).unwrap()).unwrap();
    let history = &snapshots[0].bundles[0].measurements;
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].original().input().raw(), "68");
    assert_eq!(history[0].effective().unwrap().0.raw(), "=9369.0");
    assert_eq!(
        history[0].corrections()[0].reason(),
        "Transcription, with \"quotes\""
    );
    assert_eq!(history[1].status(), MeasurementStatus::Invalidated);
    let mut reader =
        csv::Reader::from_path(reports.join("session-001-part-405363-item-001.csv")).unwrap();
    let headers = reader.headers().unwrap().clone();
    let reason = headers.iter().position(|h| h == "reason").unwrap();
    let raw = headers.iter().position(|h| h == "raw_input").unwrap();
    let rows: Vec<_> = reader.records().map(Result::unwrap).collect();
    assert!(
        rows.iter()
            .any(|r| &r[reason] == "Transcription, with \"quotes\"")
    );
    assert!(rows.iter().any(|r| &r[raw] == "=9369.0"));
}

#[test]
fn part_report_failure_preserves_active_session_and_retry_never_overwrites_exports() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("data.sqlite3");
    let blocked = directory.path().join("blocked");
    fs::write(&blocked, "do not overwrite").unwrap();
    let output = execute(
        &database,
        &blocked,
        &format!("{SETUP}68\n/close\n/close\n/exit\n"),
    );
    assert!(output.contains("Report failed"));
    assert!(output.contains("/report part 1"));
    assert!(!output.contains("✓ PNG report generated"));
    let store = SqliteStore::open(&database).unwrap();
    assert_eq!(store.sessions().unwrap()[0].status(), SessionStatus::Active);
    assert_eq!(
        store.session_snapshots(SessionId::new(1).unwrap()).unwrap()[0].bundles[0]
            .measurements
            .len(),
        1
    );
    drop(store);
    assert_eq!(fs::read_to_string(blocked).unwrap(), "do not overwrite");
    let reports = directory.path().join("reports");
    let output = execute(&database, &reports, "/report part 1\n/exit\n");
    assert!(output.contains("✓ PNG report generated"));
    let original = fs::read(reports.join("session-001-part-405363-item-001.png")).unwrap();
    let output = execute(&database, &reports, "/report part 1\n/exit\n");
    assert!(output.contains("✓ PNG report generated"));
    assert_eq!(
        fs::read(reports.join("session-001-part-405363-item-001.png")).unwrap(),
        original
    );
    assert_eq!(
        fs::read_dir(reports)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().ends_with(".manifest.json"))
            .count(),
        2
    );
}

#[test]
fn all_bundles_share_one_part_number_png_without_losing_any_population() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("data.sqlite3");
    let reports = directory.path().join("reports");
    let mut input = format!("{SETUP}68\n/close\n");
    for _ in 1..5 {
        input.push_str("/bundle\n71\n/close\n");
    }
    input.push_str("/close\n/close\n/exit\n");
    let output = execute(&database, &reports, &input);
    assert_eq!(
        output.matches("✓ PNG report generated (1 page(s))").count(),
        1
    );
    assert_png(&reports.join("session-001-part-405363-item-001.png"));
    assert_eq!(
        fs::read_dir(&reports)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "png"))
            .count(),
        1
    );
    let manifest: serde_json::Value = serde_json::from_slice(
        &fs::read(reports.join("session-001-part-405363-item-001.manifest.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["items"][0]["bundle_count"], 5);
    assert_eq!(manifest["items"][0]["measurement_count"], 5);
}

#[test]
fn closing_a_part_exports_while_session_stays_active_and_reclosing_preserves_old_reports() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("data.sqlite3");
    let reports = directory.path().join("reports");
    let output = execute(
        &database,
        &reports,
        &format!("{SETUP}68\n/close\n/close\n/exit\n"),
    );
    assert!(output.contains("Part Number 405363 closed"));
    assert!(output.contains("PNG report generated"));
    assert!(!output.contains("Session closed"));
    let original_path = reports.join("session-001-part-405363-item-001.png");
    let original = fs::read(&original_path).unwrap();
    let store = SqliteStore::open(&database).unwrap();
    let state = store.recover_session(SessionId::new(1).unwrap()).unwrap();
    assert_eq!(state.session.status(), SessionStatus::Active);
    assert!(state.open_bundle.is_none());
    assert!(state.active_item.is_none());
    drop(store);
    let output = execute(
        &database,
        &reports,
        "/resume 1\n/partnumber 405363\n/bundle\n71\n/close\n/close\n/close\n/exit\n",
    );
    assert_eq!(output.matches("PNG report generated").count(), 1);
    assert!(output.contains("Session closed"));
    assert_eq!(fs::read(original_path).unwrap(), original);
    let mut counts = Vec::new();
    for entry in fs::read_dir(&reports).unwrap().map(Result::unwrap) {
        if entry
            .file_name()
            .to_string_lossy()
            .ends_with(".manifest.json")
        {
            let manifest: serde_json::Value =
                serde_json::from_slice(&fs::read(entry.path()).unwrap()).unwrap();
            assert_eq!(manifest["items"][0]["session_status"], "Active");
            assert!(manifest["items"][0]["session_closed_utc"].is_null());
            counts.push(manifest["items"][0]["measurement_count"].as_u64().unwrap());
        }
    }
    counts.sort_unstable();
    assert_eq!(counts, vec![1, 2]);
}

#[test]
fn part_retry_requires_closed_bundles_and_manual_session_report_remains_available() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("data.sqlite3");
    let reports = directory.path().join("reports");
    let output = execute(
        &database,
        &reports,
        &format!("{SETUP}68\n/report part 1\n/report 1\n/exit\n"),
    );
    assert!(output.contains("close all bundles for this Part Number"));
    assert!(output.contains("close the inspection session"));
    assert!(!reports.exists());
    let output = execute(
        &database,
        &reports,
        "/resume 1\n/close\n/close\n/close\n/report 1\n/exit\n",
    );
    assert_eq!(
        output.matches("PNG report generated (1 page(s))").count(),
        2
    );
    assert_eq!(
        fs::read_dir(reports)
            .unwrap()
            .map(Result::unwrap)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "png"))
            .count(),
        2
    );
}

#[test]
fn empty_part_report_keeps_exact_part_number_and_uses_safe_file_names() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("data.sqlite3");
    let reports = directory.path().join("reports");
    let part = "../0042:µ";
    let output = execute(
        &database,
        &reports,
        &format!("/session\n/partnumber\n{part}\n10\n1\n0.1\noff\n/close\n/exit\n"),
    );
    assert!(output.contains("PNG report generated (1 page(s))"));
    let entries: Vec<_> = fs::read_dir(&reports)
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(entries.len(), 3);
    let manifest_path = entries
        .iter()
        .find(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .ends_with(".manifest.json")
        })
        .unwrap()
        .path();
    let manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(manifest_path).unwrap()).unwrap();
    assert_eq!(manifest["items"][0]["part_number"], part);
    assert_eq!(manifest["items"][0]["measurement_count"], 0);
    assert!(manifest["items"][0]["capture_start_utc"].is_null());
    assert!(manifest["items"][0]["capture_end_utc"].is_null());
    assert_png(&reports.join(manifest["items"][0]["png_file"].as_str().unwrap()));
    let mut csv =
        csv::Reader::from_path(reports.join(manifest["items"][0]["csv_file"].as_str().unwrap()))
            .unwrap();
    assert_eq!(csv.records().count(), 0);
    assert!(
        entries
            .iter()
            .all(|entry| entry.path().parent() == Some(reports.as_path()))
    );
}

#[test]
fn abrupt_process_termination_recovers_every_acknowledged_measurement() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("data.sqlite3");
    let mut child = Command::new(env!("CARGO_BIN_EXE_draxxmeasure"))
        .arg("--database")
        .arg(&database)
        .arg("--reports-dir")
        .arg(directory.path().join("reports"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(format!("{SETUP}68\n").as_bytes()).unwrap();
    stdin.flush().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if line.unwrap().contains("saved [1/42]") {
                let _ = send.send(());
                break;
            }
        }
    });
    let acknowledged = receive.recv_timeout(Duration::from_secs(15));
    child.kill().unwrap();
    child.wait().unwrap();
    drop(stdin);
    reader.join().unwrap();
    acknowledged.expect("CLI did not acknowledge a committed measurement in time");
    let output = execute(
        &database,
        &directory.path().join("reports"),
        "/resume 1\n/status\n71\n/exit\n",
    );
    assert!(output.contains("✓ Session recovered"));
    assert!(output.contains("[1/42]"));
    assert!(output.contains("saved [2/42]"));
    let store = SqliteStore::open(database).unwrap();
    let snapshot = store.session_snapshots(SessionId::new(1).unwrap()).unwrap();
    let values = &snapshot[0].bundles[0].measurements;
    assert_eq!(values.len(), 2);
    assert_eq!(values[0].original().input().raw(), "68");
    assert_eq!(values[1].original().sequence().get(), 2);
}
