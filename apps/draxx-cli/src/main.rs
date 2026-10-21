use draxx_cli::{CliOptions, run, run_screen};
use std::{env, ffi::OsString, io::IsTerminal, path::PathBuf, process::ExitCode};

const HELP: &str = "DraxxMeasure - local-first dimensional inspection\n\nUsage: draxxmeasure [--tui | --line] [--data-dir DIR] [--database PATH] [--reports-dir PATH] [--report-note TEXT]\n       draxxmeasure --help | --version\n\nInteractive terminals open the Ratatui screen. Redirected input/output use the line interface.\n--tui requires an interactive terminal; --line always uses plain line input.\nScreen: Enter submits; F1 toggles help; Ctrl+C exits with committed context recoverable.\nType /help for commands. Each bundle holds at most 42 active measurements; /close is explicit.\nPortable defaults beside the executable: data/draxxmeasure.sqlite3 and reports/\n--data-dir DIR places the database in DIR and reports in DIR/reports.\n--database and --reports-dir override their individual paths, regardless of option order.\nExplicit relative paths are relative to the current working directory.\nUse a writable local folder; no administrator access or workspace is required.\nEOF, Ctrl+C, or /exit leave committed sessions recoverable with /resume ID.";

#[derive(Clone, Copy, Default)]
enum Interface {
    #[default]
    Auto,
    Line,
    Screen,
}

struct LaunchOptions {
    config: CliOptions,
    interface: Interface,
}

fn options(arguments: Vec<OsString>) -> Result<Option<LaunchOptions>, String> {
    if arguments.len() == 1 && (arguments[0] == "--help" || arguments[0] == "-h") {
        println!("{HELP}");
        return Ok(None);
    }
    if arguments.len() == 1 && (arguments[0] == "--version" || arguments[0] == "-V") {
        println!("draxxmeasure {}", env!("CARGO_PKG_VERSION"));
        return Ok(None);
    }
    let mut data_directory = None;
    let mut database = None;
    let mut reports_directory = None;
    let mut report_notes = Vec::new();
    let mut interface = Interface::Auto;
    let mut arguments = arguments.into_iter();
    while let Some(flag) = arguments.next() {
        if flag == "--tui" || flag == "--line" {
            if !matches!(interface, Interface::Auto) {
                return Err("Specify only one of --tui or --line".into());
            }
            interface = if flag == "--tui" {
                Interface::Screen
            } else {
                Interface::Line
            };
            continue;
        }
        if flag != "--data-dir"
            && flag != "--database"
            && flag != "--reports-dir"
            && flag != "--report-note"
        {
            return Err(format!("Unsupported arguments: {flag:?}"));
        }
        let value = arguments
            .next()
            .ok_or_else(|| format!("Missing value for {flag:?}"))?;
        if value.is_empty() {
            return Err(format!("Empty value for {flag:?}"));
        }
        if flag == "--data-dir" {
            data_directory = Some(PathBuf::from(value));
        } else if flag == "--database" {
            database = Some(PathBuf::from(value));
        } else if flag == "--reports-dir" {
            reports_directory = Some(PathBuf::from(value));
        } else if flag == "--report-note" {
            report_notes.push(
                value
                    .into_string()
                    .map_err(|_| "Report notes must be Unicode text")?,
            );
        } else {
            return Err(format!("Unsupported arguments: {flag:?}"));
        }
    }
    let executable = env::current_exe()
        .map_err(|error| format!("Cannot determine executable location: {error}"))?;
    let mut config = CliOptions::portable(&executable, data_directory.as_deref())
        .map_err(|error| error.to_string())?;
    if let Some(path) = database {
        config.database = path;
    }
    if let Some(path) = reports_directory {
        config.reports_directory = path;
    }
    config.report_notes = report_notes;
    Ok(Some(LaunchOptions { config, interface }))
}
fn main() -> ExitCode {
    let launch = match options(env::args_os().skip(1).collect()) {
        Ok(Some(config)) => config,
        Ok(None) => return ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}\nUse --help for usage.");
            return ExitCode::from(2);
        }
    };
    let interactive = std::io::stdin().is_terminal()
        && std::io::stdout().is_terminal()
        && env::var_os("TERM").is_none_or(|term| term != "dumb");
    let result = match launch.interface {
        Interface::Screen => run_screen(launch.config),
        Interface::Auto if interactive => run_screen(launch.config),
        _ => run(
            launch.config,
            &mut std::io::stdin().lock(),
            &mut std::io::stdout().lock(),
        ),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("DraxxMeasure stopped: {error}. Existing inspection data was not reset.");
            eprintln!(
                "Use a writable local folder beside the executable, or specify --data-dir DIR / --database PATH / --reports-dir PATH. No alternate database was selected automatically."
            );
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn individual_path_overrides_win_regardless_of_argument_order() {
        for arguments in [
            vec![
                "--database",
                "existing.sqlite3",
                "--reports-dir",
                "existing-reports",
                "--data-dir",
                "portable",
            ],
            vec![
                "--data-dir",
                "portable",
                "--database",
                "existing.sqlite3",
                "--reports-dir",
                "existing-reports",
            ],
        ] {
            let config = options(arguments.into_iter().map(OsString::from).collect())
                .unwrap()
                .unwrap()
                .config;
            assert_eq!(config.database, PathBuf::from("existing.sqlite3"));
            assert_eq!(config.reports_directory, PathBuf::from("existing-reports"));
        }
    }

    #[test]
    fn empty_paths_and_missing_arguments_are_rejected_before_opening_storage() {
        for flag in ["--data-dir", "--database", "--reports-dir"] {
            assert!(options(vec![flag.into()]).is_err());
            assert!(options(vec![flag.into(), "".into()]).is_err());
        }
    }

    #[test]
    fn interface_flags_are_valueless_and_mutually_exclusive() {
        for (flag, expected_screen) in [("--tui", true), ("--line", false)] {
            let launch = options(vec![flag.into(), "--data-dir".into(), "chosen".into()])
                .unwrap()
                .unwrap();
            assert_eq!(
                matches!(launch.interface, Interface::Screen),
                expected_screen
            );
            assert_eq!(
                launch.config.database,
                PathBuf::from("chosen/draxxmeasure.sqlite3")
            );
        }
        assert!(options(vec!["--tui".into(), "--line".into()]).is_err());
        assert!(options(vec!["--line".into(), "--tui".into()]).is_err());
    }
}
