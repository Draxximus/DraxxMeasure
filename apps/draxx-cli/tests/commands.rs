use std::process::Command;

#[test]
fn binary_reports_help_version_and_invalid_arguments_without_opening_storage() {
    for argument in ["--help", "--version"] {
        let output = Command::new(env!("CARGO_BIN_EXE_draxxmeasure"))
            .arg(argument)
            .output()
            .unwrap();
        assert!(output.status.success());
        let text = String::from_utf8(output.stdout).unwrap();
        if argument == "--version" {
            assert_eq!(
                text.trim(),
                format!("draxxmeasure {}", env!("CARGO_PKG_VERSION"))
            );
        } else {
            assert!(text.contains("Usage: draxxmeasure"));
            assert!(text.contains("42 active measurements"));
        }
    }
    let output = Command::new(env!("CARGO_BIN_EXE_draxxmeasure"))
        .arg("capture")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Unsupported arguments")
    );
}

#[test]
fn forced_screen_rejects_redirected_io_before_creating_data_and_line_mode_stays_plain() {
    use std::{fs, io::Write, process::Stdio};
    let directory = tempfile::tempdir().unwrap();
    let data = directory.path().join("data");
    let output = Command::new(env!("CARGO_BIN_EXE_draxxmeasure"))
        .arg("--tui")
        .arg("--data-dir")
        .arg(&data)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("requires interactive stdin and stdout")
    );
    assert!(!data.exists());
    let mut child = Command::new(env!("CARGO_BIN_EXE_draxxmeasure"))
        .arg("--line")
        .arg("--data-dir")
        .arg(&data)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"/session\n/exit\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(
        !output.stdout.contains(&0x1b),
        "line mode emitted terminal escape sequences"
    );
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Inspection session started")
    );
    assert!(
        fs::metadata(data.join("draxxmeasure.sqlite3"))
            .unwrap()
            .is_file()
    );
}
