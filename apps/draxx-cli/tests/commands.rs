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
