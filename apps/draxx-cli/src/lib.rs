//! Portable line and Ratatui adapters. Domain, transactions, and calculations stay in libraries.
pub mod bundle_view;
mod screen;
use draxx_application::*;
use draxx_core::*;
use draxx_storage::*;
pub use screen::run_screen;
use std::{
    error::Error,
    fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
};

pub struct CliOptions {
    pub database: PathBuf,
    pub reports_directory: PathBuf,
    pub report_notes: Vec<String>,
}

impl CliOptions {
    /// No path discovery, database migration, or writes occur here. Defaults are
    /// anchored to the executable, never the shell/shortcut's working directory.
    pub fn portable(executable: &Path, data_directory: Option<&Path>) -> io::Result<Self> {
        if !executable.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "executable path must be absolute",
            ));
        }
        let directory = executable.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "executable has no parent directory",
            )
        })?;
        let (database, reports_directory) = match data_directory {
            Some(data) if data.as_os_str().is_empty() => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "data directory must not be empty",
                ));
            }
            Some(data) => (data.join("draxxmeasure.sqlite3"), data.join("reports")),
            None => (
                directory.join("data").join("draxxmeasure.sqlite3"),
                directory.join("reports"),
            ),
        };
        Ok(Self {
            database,
            reports_directory,
            report_notes: Vec::new(),
        })
    }
}
type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Presentation hooks shared by the line interface and terminal screen.
trait CliObserver {
    fn inline_bundle(&self) -> bool {
        true
    }
    fn state(&mut self, _service: &InspectionService<SqliteStore>) -> Result<()> {
        Ok(())
    }
    fn rejected(&mut self) {}
}
impl CliObserver for () {}
const COMMANDS: &str = "Commands:\n  /session                   Start an inspection session (root only)\n  /sessions                  List session IDs and states\n  /resume ID                 Recover a saved active/suspended session\n  /partnumber [CODE]         Create or reselect a Part Number (session level)\n  /bundle                    Start a bundle (Part Number level)\n  /close                     Close only the current bundle / Part Number / session\n  /status                    Show current specification and capacity\n  /measurements              List current bundle IDs, sequences and effective values\n  /undo                      Confirm invalidation of last active measurement\n  /correct SEQUENCE VALUE    Replace/restore a specific entry, with a reason\n  /policy LOWER UPPER        Explicitly approve a new abbreviation interval (mm)\n  /policy off                Require full-length input\n  /retry                     Retry a pending storage write with the SAME request ID\n  /report                    Retry the last closed Part Number report\n  /report part ITEM_ID        Export/retry one Part Number with closed bundles\n  /report SESSION_ID          Export all parts of a closed session\n  /exit                      Leave committed context recoverable; do not close it\nInside a bundle: two digits + Enter, or =full_length + Enter. No per-measurement command.\nDuring setup prompts: /cancel cancels setup. No rows are saved for invalid/ambiguous input.";

fn line(reader: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut value = String::new();
    match reader.read_line(&mut value) {
        Ok(0) => Ok(None),
        Ok(_) => {
            if value.ends_with('\n') {
                value.pop();
                if value.ends_with('\r') {
                    value.pop();
                }
            }
            Ok(Some(value))
        }
        Err(error) if error.kind() == io::ErrorKind::Interrupted => Ok(None),
        Err(error) => Err(error),
    }
}
fn ask<T>(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    observer: &mut impl CliObserver,
    label: &str,
    parse: impl Fn(String) -> Result<T>,
) -> Result<Option<T>> {
    loop {
        write!(writer, "{label}")?;
        writer.flush()?;
        let Some(raw) = line(reader)? else {
            return Ok(None);
        };
        if raw == "/cancel" {
            writeln!(
                writer,
                "Setup cancelled; committed inspection data unchanged."
            )?;
            return Ok(None);
        }
        match parse(raw) {
            Ok(value) => return Ok(Some(value)),
            Err(error) => {
                observer.rejected();
                writeln!(
                    writer,
                    "Invalid input: {error}. Correct this field or use /cancel."
                )?;
            }
        }
    }
}
fn escaped(value: &str) -> String {
    value.chars().flat_map(char::escape_default).collect()
}
fn prompt(service: &InspectionService<SqliteStore>) -> Result<String> {
    let Some(state) = service.state()? else {
        return Ok("> ".into());
    };
    if let Some(item) = state.item {
        let part = escaped(item.part_number().as_str());
        if let Some(bundle) = state.bundle {
            return Ok(format!("{part}/bundle-{}> ", bundle.sequence()));
        }
        return Ok(format!("{part}> "));
    }
    Ok("session> ".into())
}
fn policy_from_bounds(lower: Length, upper: Length) -> Result<ReconstructionPolicy> {
    Ok(ReconstructionPolicy::new(
        SUFFIX_POLICY_NAME.into(),
        PolicyVersion::new(1)?,
        lower,
        upper,
    )?)
}
fn suggested_interval(nominal: Length) -> Option<(Length, Length)> {
    let integer = nominal.nanometres() / NANOMETRES_PER_MILLIMETRE;
    let lower = integer
        .saturating_sub(50)
        .max(0)
        .checked_mul(NANOMETRES_PER_MILLIMETRE)?;
    let upper = lower.checked_add(99 * NANOMETRES_PER_MILLIMETRE)?;
    Some((
        Length::from_nanometres(lower).ok()?,
        Length::from_nanometres(upper).ok()?,
    ))
}
fn show_status(service: &InspectionService<SqliteStore>, writer: &mut impl Write) -> Result<()> {
    let Some(state) = service.state()? else {
        writeln!(
            writer,
            "No active session context. Use /session or /resume ID."
        )?;
        return Ok(());
    };
    writeln!(
        writer,
        "Session {} ({:?})",
        state.session.id(),
        state.session.status()
    )?;
    if let Some(item) = state.item {
        let spec = service.store().specification(item.specification_id())?;
        writeln!(
            writer,
            "Part Number {} | nominal {} mm | LSL {} | USL {} | resolution {} mm | spec v{}",
            escaped(item.part_number().as_str()),
            spec.nominal(),
            spec.lsl(),
            spec.usl(),
            spec.resolution().length(),
            spec.version()
        )?;
        if let Some(policy) = service.store().policy(item.id())? {
            writeln!(
                writer,
                "Approved integer-mm suffix interval: [{}, {}] mm ({} v{}); independent of tolerances",
                policy.lower(),
                policy.upper(),
                policy.name(),
                policy.version()
            )?;
        } else {
            writeln!(
                writer,
                "Abbreviation disabled. Enter explicit full lengths, e.g. =9368."
            )?;
        }
    }
    if let Some(bundle) = state.bundle {
        writeln!(
            writer,
            "Bundle #{} (ID {}) [{}/42]{}",
            bundle.sequence(),
            bundle.id(),
            state.active_count,
            if state.active_count == 42 {
                " - FULL; use /close"
            } else {
                ""
            }
        )?;
    }
    Ok(())
}
fn setup_part(
    service: &mut InspectionService<SqliteStore>,
    code: Option<&str>,
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    observer: &mut impl CliObserver,
) -> Result<()> {
    let state = service.state()?.ok_or("start /session first")?;
    if state.item.is_some() || state.bundle.is_some() {
        return Err("close the current Part Number/bundle context first".into());
    }
    let part = if let Some(code) = code {
        PartNumber::new(code.into())?
    } else {
        let Some(part) = ask(reader, writer, observer, "Part Number: ", |raw| {
            Ok(PartNumber::new(raw)?)
        })?
        else {
            return Ok(());
        };
        part
    };
    if let Some(item) = service.existing_item(&part)? {
        service.select_part(item.id())?;
        writeln!(writer, "✓ Part Number {} selected", escaped(part.as_str()))?;
        show_status(service, writer)?;
        return Ok(());
    }
    let Some(nominal) = ask(reader, writer, observer, "Nominal length (mm): ", |raw| {
        Ok(Length::parse_mm(&raw)?)
    })?
    else {
        return Ok(());
    };
    let Some(tolerance) = ask(reader, writer, observer, "Tolerance (±mm): ", |raw| {
        let value = Length::parse_mm(&raw)?;
        PartSpecification::symmetric_limits(nominal, value)?;
        Ok(value)
    })?
    else {
        return Ok(());
    };
    let Some(resolution) = ask(
        reader,
        writer,
        observer,
        "Instrument resolution (mm; explicit, not inferred from nominal): ",
        |raw| Ok(Resolution::new(Length::parse_mm(&raw)?)?),
    )?
    else {
        return Ok(());
    };
    let suggestion = suggested_interval(nominal);
    let label = if let Some((low, high)) = suggestion {
        format!(
            "Abbreviation uses last two INTEGER-mm digits. Approve interval [{low}, {high}] mm?\nEnter yes, off (full entry only), or explicit LOWER UPPER bounds: "
        )
    } else {
        "Enter approved abbreviation LOWER UPPER bounds in mm, or off: ".into()
    };
    let Some(policy) = ask(reader, writer, observer, &label, |raw| {
        if raw == "off" {
            return Ok(None);
        }
        if raw == "yes" {
            let (lower, upper) = suggestion
                .ok_or("no safe default interval at this magnitude; enter bounds or off")?;
            return Ok(Some(policy_from_bounds(lower, upper)?));
        }
        let bounds: Vec<_> = raw.split_whitespace().collect();
        if bounds.len() != 2 {
            return Err("enter yes, off, or two explicit decimal bounds".into());
        }
        Ok(Some(policy_from_bounds(
            Length::parse_mm(bounds[0])?,
            Length::parse_mm(bounds[1])?,
        )?))
    })?
    else {
        return Ok(());
    };
    service.create_part(PartSetup {
        part_number: part.clone(),
        nominal,
        tolerance,
        resolution,
        policy,
    })?;
    writeln!(writer, "✓ Part Number {} created", escaped(part.as_str()))?;
    show_status(service, writer)
}
fn saved(
    service: &InspectionService<SqliteStore>,
    receipt: CommitReceipt<Measurement>,
    writer: &mut impl Write,
) -> Result<()> {
    let count = service
        .store()
        .bundle_capacity(receipt.value.bundle_id())?
        .active();
    let spec = service
        .store()
        .specification(receipt.value.specification_id())?;
    let conformance = spec.classify(receipt.value.interpreted().value);
    writeln!(
        writer,
        "✓ {} mm saved [{count}/42] (ID {}, sequence {}){}{}",
        receipt.value.interpreted().value,
        receipt.value.id(),
        receipt.value.sequence(),
        if conformance == Conformance::WithinLimits {
            ""
        } else {
            " - OUT OF SPECIFICATION"
        },
        if receipt.replayed {
            " - original commit confirmed (not duplicated)"
        } else {
            ""
        }
    )?;
    if count == 42 {
        writeln!(
            writer,
            "Bundle is full. Enter /close explicitly; a 43rd active measurement will not be saved."
        )?;
    }
    Ok(())
}
fn report(
    service: &InspectionService<SqliteStore>,
    id: SessionId,
    config: &CliOptions,
    writer: &mut impl Write,
) -> Result<()> {
    if service.store().recover_session(id)?.session.status() != SessionStatus::Closed {
        return Err("close the inspection session before generating its report".into());
    }
    let snapshots = service.store().session_snapshots(id)?;
    let exported = draxx_reporting::export_session(
        id,
        &snapshots,
        &config.reports_directory,
        now()?,
        &config.report_notes,
    )?;
    show_export(exported, writer)
}

fn report_part(
    service: &InspectionService<SqliteStore>,
    item: InspectionItemId,
    config: &CliOptions,
    writer: &mut impl Write,
) -> Result<()> {
    let snapshot = service.store().snapshot(item)?;
    let exported = draxx_reporting::export_part(
        &snapshot,
        &config.reports_directory,
        now()?,
        &config.report_notes,
    )?;
    show_export(exported, writer)
}

fn show_export(exported: draxx_reporting::ExportedReport, writer: &mut impl Write) -> Result<()> {
    writeln!(
        writer,
        "✓ PNG report generated ({} page(s)); verification CSV exported",
        exported.png_files.len()
    )?;
    for path in exported.png_files {
        writeln!(writer, "Report: {}", path.display())?;
    }
    for path in exported.csv_files {
        writeln!(writer, "CSV: {}", path.display())?;
    }
    writeln!(writer, "Manifest: {}", exported.manifest.display())?;
    Ok(())
}

pub fn run(config: CliOptions, reader: &mut impl BufRead, writer: &mut impl Write) -> Result<()> {
    run_observed(config, reader, writer, &mut ())
}

fn run_observed(
    config: CliOptions,
    reader: &mut impl BufRead,
    writer: &mut impl Write,
    observer: &mut impl CliObserver,
) -> Result<()> {
    if let Some(parent) = config
        .database
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut service = InspectionService::new(SqliteStore::open(&config.database)?);
    writeln!(
        writer,
        "DraxxMeasure - keyboard-first dimensional inspection. Type /help.\nDatabase: {}",
        config.database.display()
    )?;
    let recoverable: Vec<_> = service
        .store()
        .sessions()?
        .into_iter()
        .filter(|s| s.status() != SessionStatus::Closed)
        .collect();
    for session in recoverable {
        writeln!(
            writer,
            "Recoverable session {} ({:?}); use /resume {}",
            session.id(),
            session.status(),
            session.id()
        )?;
    }
    let mut displayed_bundle = None;
    let mut last_report_item = None;
    loop {
        observer.state(&service)?;
        if observer.inline_bundle() {
            let state = service.state()?;
            if let Some(state) = state
                && let Some(bundle) = state.bundle
            {
                let identity = (bundle.id(), state.active_count);
                if displayed_bundle != Some(identity) {
                    writeln!(
                        writer,
                        "\n{}",
                        bundle_view::render_bundle(
                            bundle.sequence(),
                            BundleCapacity::new(state.active_count)?
                        )
                    )?;
                    displayed_bundle = Some(identity);
                }
            } else {
                displayed_bundle = None;
            }
        }
        write!(writer, "{}", prompt(&service)?)?;
        writer.flush()?;
        let Some(raw) = line(reader)? else {
            writeln!(
                writer,
                "\nInput ended. Committed data and open context remain recoverable with /resume ID."
            )?;
            if let Some(input) = service.pending_input() {
                writeln!(
                    writer,
                    "Unacknowledged input {}: resolve committed state before re-entering it.",
                    escaped(input)
                )?;
            }
            break;
        };
        if raw.is_empty() {
            continue;
        }
        if raw == "/exit" {
            writeln!(
                writer,
                "Leaving context open; committed data is recoverable with /resume ID."
            )?;
            if service.pending_input().is_some() {
                writeln!(
                    writer,
                    "An unacknowledged write exists: inspect recovery state before re-entering it."
                )?;
            }
            break;
        }
        let result: Result<()> = (|| {
            if !raw.starts_with('/') {
                let receipt = service.record(raw.clone())?;
                return saved(&service, receipt, writer);
            }
            let words: Vec<_> = raw.split_whitespace().collect();
            match words.as_slice() {
                ["/help"] => writeln!(writer, "{COMMANDS}")?,
                ["/session"] => {
                    let session = service.start_session()?;
                    writeln!(writer, "✓ Inspection session started (ID {})", session.id())?;
                }
                ["/sessions"] => {
                    for session in service.store().sessions()? {
                        writeln!(writer, "Session {}: {:?}", session.id(), session.status())?;
                    }
                }
                ["/resume", id] => {
                    service.resume(SessionId::new(id.parse()?)?)?;
                    writeln!(writer, "✓ Session recovered from committed data")?;
                    show_status(&service, writer)?;
                }
                ["/partnumber"] => setup_part(&mut service, None, reader, writer, observer)?,
                ["/partnumber", code] => {
                    setup_part(&mut service, Some(code), reader, writer, observer)?
                }
                ["/bundle"] => {
                    let bundle = service.start_bundle()?;
                    writeln!(
                        writer,
                        "✓ Bundle #{} started\nMaximum capacity: 42 measurements",
                        bundle.sequence()
                    )?;
                }
                ["/status"] => show_status(&service, writer)?,
                ["/retry"] => {
                    let receipt = service.retry()?;
                    saved(&service, receipt, writer)?;
                }
                ["/close"] => match service.close()? {
                    ClosedContext::Bundle {
                        bundle,
                        measurements,
                    } => writeln!(
                        writer,
                        "✓ Bundle #{} closed\nMeasurements: {measurements}",
                        bundle.sequence()
                    )?,
                    ClosedContext::Part {
                        item,
                        bundles,
                        measurements,
                    } => {
                        writeln!(
                            writer,
                            "✓ Part Number {} closed\nBundles: {bundles}\nMeasurements: {measurements}",
                            escaped(item.part_number().as_str())
                        )?;
                        last_report_item = Some(item.id());
                        if let Err(error) = report_part(&service, item.id(), &config, writer) {
                            writeln!(
                                writer,
                                "Report failed: {error}. Part Number context remains closed and all inspection data is preserved.\nRetry with /report part {} (or restart with a writable --reports-dir).",
                                item.id()
                            )?;
                        }
                    }
                    ClosedContext::Session(id) => writeln!(writer, "✓ Session closed (ID {id})")?,
                },
                ["/report"] if last_report_item.is_some() => report_part(
                    &service,
                    last_report_item.ok_or("no closed Part Number")?,
                    &config,
                    writer,
                )?,
                ["/report", "part", id] => report_part(
                    &service,
                    InspectionItemId::new(id.parse()?)?,
                    &config,
                    writer,
                )?,
                ["/report"] => report(
                    &service,
                    service.last_closed().ok_or(
                        "specify /report part ITEM_ID or /report SESSION_ID after restart",
                    )?,
                    &config,
                    writer,
                )?,
                ["/report", id] => report(&service, SessionId::new(id.parse()?)?, &config, writer)?,
                ["/policy", "off"] => {
                    service.configure_policy(None)?;
                    writeln!(
                        writer,
                        "✓ Abbreviation disabled; full-length entry remains available"
                    )?;
                }
                ["/policy", lower, upper] => {
                    let policy =
                        policy_from_bounds(Length::parse_mm(lower)?, Length::parse_mm(upper)?)?;
                    service.configure_policy(Some(policy))?;
                    writeln!(
                        writer,
                        "✓ Explicit capture interval recorded; historical interpretations unchanged"
                    )?;
                    show_status(&service, writer)?;
                }
                ["/measurements"] => {
                    let state = service.state()?.ok_or("no session")?;
                    let bundle = state.bundle.ok_or("start /bundle first")?;
                    let snapshot = service.current_snapshot()?;
                    for data in snapshot
                        .bundles
                        .into_iter()
                        .filter(|b| b.bundle.id() == bundle.id())
                    {
                        for history in data.measurements {
                            writeln!(
                                writer,
                                "Sequence {} | ID {} | raw {} | effective {} | {:?}",
                                history.original().sequence(),
                                history.original().id(),
                                escaped(history.original().input().raw()),
                                history
                                    .effective()
                                    .map_or("invalidated".into(), |(_, v)| format!(
                                        "{} mm",
                                        v.value
                                    )),
                                history.status()
                            )?;
                        }
                    }
                }
                ["/undo"] => {
                    let target = service.target(None)?;
                    let label = format!(
                        "Undo sequence {} (ID {}, {} mm) in THIS bundle? Enter yes/no: ",
                        target.original().sequence(),
                        target.original().id(),
                        target
                            .effective()
                            .ok_or("entry already invalidated")?
                            .1
                            .value
                    );
                    if ask(reader, writer, observer, &label, |raw| match raw.as_str() {
                        "yes" => Ok(true),
                        "no" => Ok(false),
                        _ => Err("enter yes or no".into()),
                    })? == Some(true)
                    {
                        let Some(reason) = ask(
                            reader,
                            writer,
                            observer,
                            "Reason (Enter = Operator undo): ",
                            |raw| {
                                Ok(if raw.is_empty() {
                                    "Operator undo confirmed in CLI".into()
                                } else {
                                    raw
                                })
                            },
                        )?
                        else {
                            return Ok(());
                        };
                        let event = service.correct(target, None, reason)?;
                        writeln!(
                            writer,
                            "✓ Measurement invalidated (audit event {}); original retained",
                            event.id()
                        )?;
                        show_status(&service, writer)?;
                    }
                }
                ["/correct", sequence, value] => {
                    let target = service.target(Some(sequence.parse()?))?;
                    writeln!(
                        writer,
                        "Correcting sequence {} (ID {}) in the current bundle; original {} is retained.",
                        target.original().sequence(),
                        target.original().id(),
                        escaped(target.original().input().raw())
                    )?;
                    let Some(reason) =
                        ask(reader, writer, observer, "Correction reason: ", |raw| {
                            if raw.trim().is_empty() {
                                Err("reason is required".into())
                            } else {
                                Ok(raw)
                            }
                        })?
                    else {
                        return Ok(());
                    };
                    let event = service.correct(target, Some((*value).into()), reason)?;
                    writeln!(
                        writer,
                        "✓ Correction saved (audit event {}); original retained",
                        event.id()
                    )?;
                    show_status(&service, writer)?;
                }
                _ => return Err("unknown command or wrong context/arguments; use /help".into()),
            }
            Ok(())
        })();
        if let Err(error) = result {
            observer.rejected();
            writeln!(
                writer,
                "Error: {error}\nNot acknowledged as saved: {}",
                escaped(&raw)
            )?;
            if let Some(input) = service.pending_input() {
                writeln!(
                    writer,
                    "Pending input retained: {}. Use /retry, not a new measurement submission.",
                    escaped(input)
                )?;
            }
        }
    }
    Ok(())
}
