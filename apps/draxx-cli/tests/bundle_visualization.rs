use draxx_cli::{CliOptions, run};
use draxx_core::{BundleStatus, SessionId};
use draxx_storage::{InspectionStore, SqliteStore};
use std::path::Path;

const SETUP: &str = "/session\n/partnumber\nSYNTHETIC\n9368.9\n5\n0.1\nyes\n/bundle\n";

fn execute(directory: &Path, input: &str) -> String {
    let mut output = Vec::new();
    run(
        CliOptions {
            database: directory.join("inspection.sqlite3"),
            reports_directory: directory.join("reports"),
            report_notes: Vec::new(),
        },
        &mut input.as_bytes(),
        &mut output,
    )
    .unwrap();
    String::from_utf8(output).unwrap()
}

fn diagrams(output: &str) -> Vec<(String, usize)> {
    let lines: Vec<_> = output.lines().collect();
    let mut diagrams = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.starts_with("Bundle #") && line.contains("/42 measured") {
            assert_eq!(lines[index + 1], "");
            let circles: String = lines[index + 2..index + 7].join("\n");
            let measured = circles.matches('●').count();
            assert_eq!(measured + circles.matches('○').count(), 42);
            assert!(line.ends_with(&format!("— {measured}/42 measured")));
            diagrams.push((line.to_string(), measured));
        }
    }
    diagrams
}

#[test]
fn line_view_updates_after_commit_and_undo_and_rebuilds_on_resume() {
    let directory = tempfile::tempdir().unwrap();
    let output = execute(
        directory.path(),
        &format!("{SETUP}68\nabc\n/status\n71\n/undo\nyes\nSynthetic undo\n/exit\n"),
    );
    assert_eq!(
        diagrams(&output)
            .iter()
            .map(|(_, count)| *count)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 1]
    );
    assert!(
        output.find("saved [1/42]").unwrap() < output.find("Bundle #1 — 1/42 measured").unwrap()
    );
    assert!(
        !output.contains('\x1b'),
        "plain interface must not clear the terminal"
    );
    let output = execute(
        directory.path(),
        "/resume 1\n/correct 2 =9369\nSynthetic restoration\n/close\n/bundle\n/exit\n",
    );
    assert_eq!(
        diagrams(&output),
        vec![
            ("Bundle #1 — 1/42 measured".into(), 1),
            ("Bundle #1 — 2/42 measured".into(), 2),
            ("Bundle #2 — 0/42 measured".into(), 0),
        ]
    );
}

#[test]
fn line_view_fills_all_positions_without_automatic_closure_or_a_forty_third_position() {
    let directory = tempfile::tempdir().unwrap();
    let output = execute(
        directory.path(),
        &format!("{SETUP}{}71\n/exit\n", "68\n".repeat(42)),
    );
    let counts: Vec<_> = diagrams(&output)
        .into_iter()
        .map(|(_, count)| count)
        .collect();
    assert_eq!(counts, (0..=42).collect::<Vec<_>>());
    assert!(output.contains("bundle is full"));
    let store = SqliteStore::open(directory.path().join("inspection.sqlite3")).unwrap();
    let recovered = store.recover_session(SessionId::new(1).unwrap()).unwrap();
    assert_eq!(recovered.open_bundle.unwrap().status(), BundleStatus::Open);
}
