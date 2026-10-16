use draxx_cli::CliOptions;
use draxx_core::SessionId;
use draxx_storage::{InspectionStore, SqliteStore};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

fn copied_cli(directory: &Path) -> PathBuf {
    fs::create_dir_all(directory).unwrap();
    let source = Path::new(env!("CARGO_BIN_EXE_draxxmeasure"));
    let executable = directory.join(source.file_name().unwrap());
    fs::copy(source, &executable).unwrap();
    executable
}

fn launch(executable: &Path, working_directory: &Path, arguments: &[&str], input: &str) -> String {
    let mut child = Command::new(executable)
        .current_dir(working_directory)
        .args(arguments)
        .env("PATH", "")
        .env_remove("CARGO_HOME")
        .env_remove("RUSTUP_HOME")
        .env_remove("CARGO_MANIFEST_DIR")
        .env_remove("PYTHONPATH")
        .env_remove("PYTHONHOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(!stdout.contains("Error:"), "{stdout}");
    assert!(!stdout.contains("Report failed:"), "{stdout}");
    stdout
}

#[test]
fn portable_defaults_are_executable_relative_and_custom_data_path_is_explicit() {
    let temporary = tempfile::tempdir().unwrap();
    let executable = temporary
        .path()
        .join("portable app")
        .join("draxxmeasure.exe");
    let options = CliOptions::portable(&executable, None).unwrap();
    assert_eq!(
        options.database,
        executable
            .parent()
            .unwrap()
            .join("data/draxxmeasure.sqlite3")
    );
    assert_eq!(
        options.reports_directory,
        executable.parent().unwrap().join("reports")
    );
    let custom = temporary.path().join("custom data");
    let options = CliOptions::portable(&executable, Some(&custom)).unwrap();
    assert_eq!(options.database, custom.join("draxxmeasure.sqlite3"));
    assert_eq!(options.reports_directory, custom.join("reports"));
    assert!(
        !custom.exists(),
        "path selection must not create or modify inspection files"
    );
}

#[test]
fn copied_executable_captures_and_reports_without_the_workspace_and_update_preserves_data() {
    let temporary = tempfile::tempdir().unwrap();
    let app = temporary.path().join("portable app - µ");
    let unrelated = temporary.path().join("unrelated working directory");
    fs::create_dir(&unrelated).unwrap();
    let executable = copied_cli(&app);
    let output = launch(
        &executable,
        &unrelated,
        &[],
        "/session\n/partnumber\n405363\n9368.9\n5\n0.1\nyes\n/bundle\n68\n71\n/close\n/close\n/close\n/exit\n",
    );
    assert!(output.contains("PNG report generated"));
    let database = app.join("data/draxxmeasure.sqlite3");
    let report = app.join("reports/session-001.png");
    assert!(database.is_file());
    assert_eq!(image::open(&report).unwrap().width(), 1800);
    assert!(!unrelated.join("data").exists());
    assert!(!unrelated.join("reports").exists());
    let before_database = fs::read(&database).unwrap();
    let before_report = fs::read(&report).unwrap();
    // Simulate the documented binary-only update after the previous process exits.
    copied_cli(&app);
    assert_eq!(fs::read(&database).unwrap(), before_database);
    assert_eq!(fs::read(&report).unwrap(), before_report);
    let output = launch(&executable, &unrelated, &[], "/sessions\n/exit\n");
    assert!(output.contains("Session 1: Closed"));
    let store = SqliteStore::open(database).unwrap();
    assert_eq!(
        store.session_snapshots(SessionId::new(1).unwrap()).unwrap()[0].bundles[0]
            .measurements
            .len(),
        2
    );
}

#[test]
fn configurable_data_directory_works_from_an_unrelated_directory() {
    let temporary = tempfile::tempdir().unwrap();
    let executable = copied_cli(&temporary.path().join("app"));
    let custom = temporary.path().join("chosen data - µ");
    launch(
        &executable,
        temporary.path(),
        &["--data-dir", custom.to_str().unwrap()],
        "/session\n/exit\n",
    );
    assert!(custom.join("draxxmeasure.sqlite3").is_file());
    assert!(!executable.parent().unwrap().join("data").exists());
}
