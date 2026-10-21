//! Ratatui presentation of the same line-oriented capture controller.
//!
//! One worker owns SQLite and the controller. Channels carry complete input lines,
//! committed read views, and feedback; terminal events never mutate inspection data.
use super::*;
use crossterm::{
    event::{
        self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyCode, KeyEventKind,
        KeyModifiers,
    },
    execute,
};
use ratatui::{
    Frame,
    layout::{Constraint, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Gauge, Paragraph, Row, Table, Wrap},
};
use std::{
    collections::VecDeque,
    io::{Cursor, IsTerminal, Read},
    sync::mpsc::{self, Receiver, Sender},
    thread,
    time::Duration,
};

const ACCENT: Color = Color::Cyan;
const MUTED: Color = Color::DarkGray;
const WORKFLOW_HELP: &str = "WORKFLOW\nF2  /session\nF3  /partnumber\nF4  /bundle\nF5  /close (one level)\nF6  /undo (confirm)\nF7  /retry\nF8  /report\nF9  /status (refresh)\n\nRECOVERY\n/resume ID\n/sessions\n/report ID\n/report part ITEM_ID\n\nCORRECTION\n/correct SEQ VALUE\n/policy LOWER UPPER\n/policy off";
const ENTRY_HELP: &str = "MEASUREMENTS\n68 or =9368.9, then Enter\nSaved only after commit\n42 active per bundle\n/close is always explicit\n\nEDITING\nArrows / Home / End\nBackspace / Delete\nCtrl+U clears current line\nPgUp / PgDn scroll rows\n\nPASTE\nOne line per Enter\nInvalid lines stay editable\nRemaining lines stay queued\n\nLEAVE / CANCEL\n/cancel cancels a prompt\nCtrl+C / Ctrl+D leave\n/exit leaves context open";

#[derive(Default)]
struct View {
    context: String,
    specification: String,
    policy: String,
    active: usize,
    bundle_open: bool,
    bundle_diagram: String,
    pending: Option<String>,
    rows: Vec<MeasurementRow>,
    sessions: Vec<String>,
}

struct MeasurementRow {
    sequence: String,
    id: String,
    raw: String,
    value: String,
    status: String,
    color: Color,
}

impl View {
    fn capture(service: &InspectionService<SqliteStore>) -> Result<Self> {
        let mut view = Self {
            pending: service.pending_input().map(display_text),
            ..Self::default()
        };
        let Some(state) = service.state()? else {
            view.context = "No active session — F2 starts an inspection".into();
            view.sessions = service
                .store()
                .sessions()?
                .into_iter()
                .rev()
                .map(|session| {
                    format!(
                        "Session {}  {:?}{}",
                        session.id(),
                        session.status(),
                        if session.status() == SessionStatus::Closed {
                            "  /report"
                        } else {
                            "  /resume"
                        }
                    )
                })
                .collect();
            return Ok(view);
        };
        view.context = format!(
            "Session {} · {:?}",
            state.session.id(),
            state.session.status()
        );
        if let Some(item) = state.item {
            let spec = service.store().specification(item.specification_id())?;
            view.context.push_str(&format!(
                "  /  Part {} · spec v{}",
                display_text(item.part_number().as_str()),
                spec.version()
            ));
            view.specification = format!(
                "Nominal {} mm   LSL {}   USL {}   Resolution {} mm",
                spec.nominal(),
                spec.lsl(),
                spec.usl(),
                spec.resolution().length()
            );
            view.policy = match service.store().policy(item.id())? {
                Some(policy) => format!(
                    "Approved suffix window: {} .. {} mm",
                    policy.lower(),
                    policy.upper()
                ),
                None => "Full lengths only · abbreviation disabled".into(),
            };
            if let Some(bundle) = state.bundle {
                view.context.push_str(&format!(
                    "  /  Bundle {} (ID {})",
                    bundle.sequence(),
                    bundle.id()
                ));
                view.active = state.active_count;
                view.bundle_open = true;
                view.bundle_diagram = bundle_view::render_bundle(
                    bundle.sequence(),
                    BundleCapacity::new(state.active_count)?,
                );
                for data in service.current_snapshot()?.bundles {
                    if data.bundle.id() != bundle.id() {
                        continue;
                    }
                    for history in data.measurements.into_iter().rev() {
                        let (value, status, color) = match history.effective() {
                            Some((_, value)) => {
                                let classification = spec.classify(value.value);
                                let label = match classification {
                                    Conformance::WithinLimits => "IN SPEC",
                                    Conformance::BelowLimit => "BELOW LSL",
                                    Conformance::AboveLimit => "ABOVE USL",
                                };
                                (
                                    format!("{} mm", value.value),
                                    format!(
                                        "{label}{}",
                                        if history.status() == MeasurementStatus::Corrected {
                                            " · corrected"
                                        } else {
                                            ""
                                        }
                                    ),
                                    if classification == Conformance::WithinLimits {
                                        Color::Green
                                    } else {
                                        Color::Red
                                    },
                                )
                            }
                            None => ("—".into(), "INVALIDATED".into(), MUTED),
                        };
                        view.rows.push(MeasurementRow {
                            sequence: history.original().sequence().to_string(),
                            id: history.original().id().to_string(),
                            raw: display_text(history.original().input().raw()),
                            value,
                            status,
                            color,
                        });
                    }
                }
            }
        } else {
            view.specification = "F3 selects or creates a Part Number".into();
        }
        Ok(view)
    }
}

enum ScreenEvent {
    View(Box<View>),
    Output(String),
    Rejected,
    Ready,
    Finished(std::result::Result<(), String>),
}

struct Observer(Sender<ScreenEvent>);
impl CliObserver for Observer {
    fn inline_bundle(&self) -> bool {
        false
    }
    fn state(&mut self, service: &InspectionService<SqliteStore>) -> Result<()> {
        self.0
            .send(ScreenEvent::View(Box::new(View::capture(service)?)))?;
        Ok(())
    }
    fn rejected(&mut self) {
        let _ = self.0.send(ScreenEvent::Rejected);
    }
}

struct ScreenWriter {
    sender: Sender<ScreenEvent>,
    buffer: Vec<u8>,
}
impl Write for ScreenWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        if !self.buffer.is_empty() {
            let text = String::from_utf8(std::mem::take(&mut self.buffer))
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
            self.sender
                .send(ScreenEvent::Output(text))
                .map_err(io::Error::other)?;
        }
        Ok(())
    }
}

struct ScreenReader {
    sender: Sender<ScreenEvent>,
    receiver: Receiver<String>,
    buffer: Cursor<Vec<u8>>,
}
impl BufRead for ScreenReader {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.buffer.position() as usize == self.buffer.get_ref().len() {
            self.sender
                .send(ScreenEvent::Ready)
                .map_err(io::Error::other)?;
            self.buffer = Cursor::new(match self.receiver.recv() {
                Ok(line) => format!("{line}\n").into_bytes(),
                Err(_) => Vec::new(), // Closing the channel is the controller's normal EOF path.
            });
        }
        self.buffer.fill_buf()
    }
    fn consume(&mut self, amount: usize) {
        self.buffer.consume(amount);
    }
}
impl Read for ScreenReader {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let bytes = self.fill_buf()?;
        let count = bytes.len().min(output.len());
        output[..count].copy_from_slice(&bytes[..count]);
        self.consume(count);
        Ok(count)
    }
}

#[derive(Default)]
struct Editor {
    text: String,
    cursor: usize, // UTF-8 byte boundary, independent of terminal cell width.
    queued: VecDeque<String>,
}
impl Editor {
    fn insert(&mut self, text: &str) {
        self.text.insert_str(self.cursor, text);
        self.cursor += text.len();
    }
    fn paste(&mut self, text: &str) {
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        self.insert(&text);
        if self.text.contains('\n') {
            let mut lines: VecDeque<_> = self.text.split('\n').map(String::from).collect();
            if self.text.ends_with('\n') {
                lines.pop_back();
            }
            self.text = lines.pop_front().unwrap_or_default();
            lines.append(&mut self.queued);
            self.queued = lines;
            self.cursor = self.text.len();
        }
    }
    fn accepted(&mut self) {
        self.text = self.queued.pop_front().unwrap_or_default();
        self.cursor = self.text.len();
    }
    fn left(&mut self) {
        self.cursor = self.text[..self.cursor]
            .char_indices()
            .next_back()
            .map_or(0, |(index, _)| index);
    }
    fn right(&mut self) {
        if let Some(ch) = self.text[self.cursor..].chars().next() {
            self.cursor += ch.len_utf8();
        }
    }
    fn backspace(&mut self) {
        let end = self.cursor;
        self.left();
        self.text.drain(self.cursor..end);
    }
    fn delete(&mut self) {
        let start = self.cursor;
        self.right();
        self.text.drain(start..self.cursor);
        self.cursor = start;
    }
}

struct Screen {
    view: View,
    editor: Editor,
    log: VecDeque<String>,
    prompt: String,
    ready: bool,
    submitted: bool,
    rejected: bool,
    help: bool,
    history: bool,
    row_offset: usize,
}
impl Default for Screen {
    fn default() -> Self {
        Self {
            view: View::default(),
            editor: Editor::default(),
            log: VecDeque::new(),
            prompt: "Opening inspection database…".into(),
            ready: false,
            submitted: false,
            rejected: false,
            help: false,
            history: false,
            row_offset: 0,
        }
    }
}
impl Screen {
    fn log(&mut self, line: String) {
        self.log.push_back(line);
        while self.log.len() > 200 {
            self.log.pop_front();
        }
    }
    fn apply(&mut self, event: ScreenEvent) -> Option<std::result::Result<(), String>> {
        match event {
            ScreenEvent::View(view) => {
                self.view = *view;
                self.row_offset = self.row_offset.min(self.view.rows.len().saturating_sub(1));
            }
            ScreenEvent::Output(text) => {
                let mut lines = text.split('\n').peekable();
                while let Some(line) = lines.next() {
                    if lines.peek().is_some() {
                        if !line.is_empty() {
                            self.log(display_text(line));
                        }
                    } else {
                        self.prompt = display_text(line);
                    }
                }
            }
            ScreenEvent::Rejected => self.rejected = true,
            ScreenEvent::Ready => {
                if self.submitted && !self.rejected {
                    self.editor.accepted();
                }
                self.submitted = false;
                self.rejected = false;
                self.ready = true;
            }
            ScreenEvent::Finished(result) => {
                if result.is_ok() && self.submitted && !self.rejected {
                    self.editor.accepted();
                }
                return Some(result);
            }
        }
        None
    }
    fn submit(&mut self, sender: &Sender<String>) -> Result<()> {
        sender.send(self.editor.text.clone())?;
        self.log(format!("> {}", display_text(&self.editor.text)));
        self.ready = false;
        self.submitted = true;
        self.row_offset = 0;
        Ok(())
    }
    fn key(
        &mut self,
        code: KeyCode,
        modifiers: KeyModifiers,
        sender: &Sender<String>,
    ) -> Result<bool> {
        if modifiers.contains(KeyModifiers::CONTROL) && matches!(code, KeyCode::Char('c' | 'd')) {
            return Ok(true);
        }
        if self.help {
            if matches!(code, KeyCode::F(1) | KeyCode::Esc | KeyCode::Enter) {
                self.help = false;
            }
            return Ok(false);
        }
        if modifiers.contains(KeyModifiers::CONTROL) {
            if code == KeyCode::Char('u') {
                self.editor.text.clear();
                self.editor.cursor = 0;
            }
            return Ok(false);
        }
        match code {
            KeyCode::Enter => self.submit(sender)?,
            KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::ALT) => {
                self.editor.insert(&ch.to_string())
            }
            KeyCode::Backspace => self.editor.backspace(),
            KeyCode::Delete => self.editor.delete(),
            KeyCode::Left => self.editor.left(),
            KeyCode::Right => self.editor.right(),
            KeyCode::Home => self.editor.cursor = 0,
            KeyCode::End => self.editor.cursor = self.editor.text.len(),
            KeyCode::PageUp => {
                self.row_offset = (self.row_offset + 8).min(self.view.rows.len().saturating_sub(1))
            }
            KeyCode::PageDown => self.row_offset = self.row_offset.saturating_sub(8),
            KeyCode::F(1) => self.help = !self.help,
            KeyCode::F(10) => self.history = !self.history,
            KeyCode::F(number @ 2..=9) if self.prompt.ends_with("> ") => {
                if !self.editor.text.is_empty() || !self.editor.queued.is_empty() {
                    self.log(
                        "Current input retained; submit it or use Ctrl+U before a shortcut.".into(),
                    );
                } else {
                    self.editor.insert(match number {
                        2 => "/session",
                        3 => "/partnumber",
                        4 => "/bundle",
                        5 => "/close",
                        6 => "/undo",
                        7 => "/retry",
                        8 => "/report",
                        _ => "/status",
                    });
                    self.submit(sender)?;
                }
            }
            _ => {}
        }
        Ok(false)
    }

    fn draw(&self, frame: &mut Frame, database: &str) {
        let area = frame.area();
        if area.width < 72 || area.height < 24 {
            frame.render_widget(Paragraph::new(format!("DraxxMeasure · compact view\nFull view needs 72 × 24 (currently {} × {}).\n{}\n{}\nPrompt: {}\nEntry: {}\n{} pasted lines queued · Enter submits · Ctrl+C leaves", area.width, area.height, self.view.context, self.view.bundle_diagram, self.prompt, display_text(&self.editor.text), self.editor.queued.len()))
                .block(panel(" Terminal size ")).wrap(Wrap { trim: false }), area);
            return;
        }
        let compact_bundle = self.view.bundle_open && area.height < 29;
        let areas = Layout::vertical([
            Constraint::Length(if compact_bundle { 1 } else { 3 }),
            Constraint::Length(5),
            Constraint::Min(if self.view.bundle_open { 9 } else { 4 }),
            Constraint::Length(if compact_bundle { 5 } else { 8 }),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(
                    " DRAXXMEASURE ",
                    Style::default()
                        .fg(Color::Black)
                        .bg(ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw("   Dimensional inspection   ·   Local SQLite"),
            ]))
            .block(if compact_bundle {
                Block::default()
            } else {
                panel(" Capture workspace ")
            }),
            areas[0],
        );
        let context = Layout::horizontal([
            Constraint::Min(40),
            Constraint::Length(if area.width >= 110 { 24 } else { 0 }),
        ])
        .split(areas[1]);
        let context_title = if self.view.bundle_open && area.width < 110 {
            format!(" Active inspection · {}/42 saved ", self.view.active)
        } else {
            " Active inspection ".into()
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::styled(&self.view.context, Style::default().fg(ACCENT)),
                Line::raw(&self.view.specification),
                Line::raw(&self.view.policy),
            ])
            .block(panel(&context_title)),
            context[0],
        );
        frame.render_widget(
            Gauge::default()
                .block(panel(" Bundle capacity "))
                .ratio(self.view.active as f64 / MAX_BUNDLE_MEASUREMENTS as f64)
                .label(if self.view.bundle_open {
                    format!(
                        "{}/42{}",
                        self.view.active,
                        if self.view.active == 42 {
                            " FULL"
                        } else {
                            " saved"
                        }
                    )
                } else {
                    "No open bundle".into()
                })
                .gauge_style(Style::default().fg(if self.view.active == 42 {
                    Color::Yellow
                } else {
                    ACCENT
                })),
            context[1],
        );
        let body = if self.view.bundle_open {
            Layout::horizontal([Constraint::Length(38), Constraint::Min(34)]).split(areas[2])
        } else {
            Layout::horizontal([
                Constraint::Min(40),
                Constraint::Length(if area.width >= 110 { 32 } else { 0 }),
            ])
            .split(areas[2])
        };
        if self.view.bundle_open {
            frame.render_widget(
                Paragraph::new(self.view.bundle_diagram.as_str())
                    .block(panel(" Tube arrangement · 8–9–8–9–8 ")),
                body[0],
            );
            if self.history {
                let measurements_area = body[1];
                let detailed = measurements_area.width >= 70;
                let count = measurements_area.height.saturating_sub(3) as usize;
                let rows = self
                    .view
                    .rows
                    .iter()
                    .skip(self.row_offset)
                    .take(count)
                    .map(|row| {
                        Row::new(if detailed {
                            vec![
                                row.sequence.clone(),
                                row.id.clone(),
                                row.raw.clone(),
                                row.value.clone(),
                                row.status.clone(),
                            ]
                        } else {
                            vec![row.sequence.clone(), row.value.clone(), row.status.clone()]
                        })
                        .style(Style::default().fg(row.color))
                    });
                let title = format!(
                    " {} · rows {}–{} of {} ",
                    if detailed {
                        "Committed measurements · latest first"
                    } else {
                        "Saved · latest first"
                    },
                    (self.row_offset + 1).min(self.view.rows.len()),
                    (self.row_offset + count).min(self.view.rows.len()),
                    self.view.rows.len()
                );
                frame.render_widget(
                    Table::new(
                        rows,
                        if detailed {
                            vec![
                                Constraint::Length(5),
                                Constraint::Length(6),
                                Constraint::Percentage(24),
                                Constraint::Percentage(25),
                                Constraint::Min(12),
                            ]
                        } else {
                            vec![
                                Constraint::Length(4),
                                Constraint::Length(14),
                                Constraint::Min(12),
                            ]
                        },
                    )
                    .header(
                        Row::new(if detailed {
                            vec!["Seq", "ID", "Original input", "Effective value", "Status"]
                        } else {
                            vec!["Seq", "Value", "Status"]
                        })
                        .style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
                    )
                    .block(panel(&title)),
                    measurements_area,
                );
            } else {
                frame.render_widget(Paragraph::new("Enter a measurement, then press Enter.\n\nEmpty positions: not yet measured\nFilled positions: measured and saved\n\nF5 closes this bundle explicitly.\nF6 starts confirmed undo.\nF10 shows measurement history.")
                    .block(panel(" Bundle progress ")).wrap(Wrap { trim: false }), body[1]);
            }
        } else {
            let message = if self.view.sessions.is_empty() {
                "Ready for capture\n\nF2  Start a session\nF3  Select a Part Number\nF4  Start a bundle\n\nEnter two digits or =full_length, then press Enter.\nOnly committed records appear in the measurement table.".into()
            } else {
                format!(
                    "Saved sessions\n\n{}\n\nUse /resume ID to continue or /report ID for a closed session.",
                    self.view.sessions.join("\n")
                )
            };
            frame.render_widget(
                Paragraph::new(message)
                    .block(panel(" Inspection overview "))
                    .wrap(Wrap { trim: false }),
                body[0],
            );
        }
        if !self.view.bundle_open && body[1].width > 0 {
            frame.render_widget(
                Paragraph::new("F2 Session\nF3 Part Number\nF4 Bundle\nF5 Close level\nF6 Undo\nF7 Retry\nF8 Report\nF1 All commands")
                    .block(panel(" Keyboard / commands "))
                    .wrap(Wrap { trim: false }),
                body[1],
            );
        }
        let prompt = if self.ready {
            self.prompt.as_str()
        } else {
            "Working… waiting for commit / report completion"
        };
        let prompt_height =
            (wrap_cells(prompt, area.width.saturating_sub(2) as usize).len() as u16 + 2)
                .clamp(3, 5);
        let activity = Layout::vertical([
            Constraint::Min(if compact_bundle { 1 } else { 3 }),
            Constraint::Length(prompt_height),
        ])
        .split(areas[3]);
        let width = activity[0]
            .width
            .saturating_sub(if compact_bundle { 0 } else { 2 }) as usize;
        let lines: Vec<String> = self
            .log
            .iter()
            .flat_map(|line| wrap_cells(line, width))
            .collect();
        let visible = activity[0]
            .height
            .saturating_sub(if compact_bundle { 0 } else { 2 }) as usize;
        let lines: Vec<Line<'_>> = lines
            .iter()
            .skip(lines.len().saturating_sub(visible))
            .map(|line| {
                Line::styled(
                    line,
                    Style::default().fg(
                        if line.starts_with("Error:") || line.starts_with("Invalid input:") {
                            Color::Red
                        } else {
                            Color::White
                        },
                    ),
                )
            })
            .collect();
        frame.render_widget(
            Paragraph::new(lines).block(if compact_bundle {
                Block::default()
            } else {
                panel(" Activity · commit receipts and errors ")
            }),
            activity[0],
        );
        frame.render_widget(
            Paragraph::new(prompt)
                .style(Style::default().fg(Color::Yellow))
                .wrap(Wrap { trim: false })
                .block(panel(" Current prompt ")),
            activity[1],
        );
        let input_width = areas[4].width.saturating_sub(3) as usize;
        let cursor = Line::raw(display_text(&self.editor.text[..self.editor.cursor])).width();
        let scroll = cursor
            .saturating_sub(input_width.saturating_sub(1))
            .min(u16::MAX as usize) as u16;
        let input_title = match &self.view.pending {
            Some(raw) => format!(" UNACKNOWLEDGED: {raw} · use /retry "),
            None => format!(
                " Entry · {} pasted lines queued · Enter submits one line ",
                self.editor.queued.len()
            ),
        };
        frame.render_widget(
            Paragraph::new(display_text(&self.editor.text))
                .scroll((0, scroll))
                .block(panel(&input_title).border_style(Style::default().fg(
                    if self.view.pending.is_some() {
                        Color::Red
                    } else {
                        ACCENT
                    },
                ))),
            areas[4],
        );
        if self.ready && !self.help {
            frame.set_cursor_position((
                areas[4].x
                    + 1
                    + (cursor - scroll as usize).min(input_width.saturating_sub(1)) as u16,
                areas[4].y + 1,
            ));
        }
        frame.render_widget(
            Paragraph::new(format!(
                " F1 help · F10 history · Ctrl+C leave   |   {database}"
            ))
            .style(Style::default().fg(MUTED)),
            areas[5],
        );
        if self.help {
            let area = area.inner(ratatui::layout::Margin {
                horizontal: 1,
                vertical: if area.height >= 26 { 1 } else { 0 },
            });
            frame.render_widget(Clear, area);
            let block = panel(" Keyboard guide · F1 / Esc / Enter returns to capture ");
            let inner = block.inner(area);
            frame.render_widget(block, area);
            let columns =
                Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                    .split(inner);
            frame.render_widget(
                Paragraph::new(format!("{WORKFLOW_HELP}\nF10 Measurement table"))
                    .wrap(Wrap { trim: false }),
                columns[0],
            );
            frame.render_widget(
                Paragraph::new(ENTRY_HELP).wrap(Wrap { trim: false }),
                columns[1],
            );
        }
    }
}

fn panel(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(MUTED))
        .title(title)
}
fn display_text(text: &str) -> String {
    let mut output = String::new();
    for ch in text.chars() {
        if ch.is_control() {
            output.extend(ch.escape_default());
        } else {
            output.push(ch);
        }
    }
    output
}
fn wrap_cells(text: &str, width: usize) -> Vec<String> {
    let mut lines = vec![String::new()];
    let mut cells = 0;
    for ch in text.chars() {
        let size = Line::raw(ch.to_string()).width();
        if cells + size > width.max(1) && cells > 0 {
            lines.push(String::new());
            cells = 0;
        }
        lines.last_mut().expect("one line always exists").push(ch);
        cells += size;
    }
    lines
}

/// Restore raw/alternate-screen and paste modes on errors and normal exits.
struct RestoreTerminal;
impl Drop for RestoreTerminal {
    fn drop(&mut self) {
        let _ = execute!(io::stdout(), DisableBracketedPaste);
        ratatui::restore();
    }
}

/// Interactive terminal screen. Redirected streams must use `run` instead.
pub fn run_screen(config: CliOptions) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(
            "--tui requires interactive stdin and stdout; use --line for redirected input/output"
                .into(),
        );
    }
    let restore = RestoreTerminal;
    let mut terminal = ratatui::try_init()?;
    execute!(io::stdout(), EnableBracketedPaste)?;
    let database = display_text(&config.database.display().to_string());
    let (input, input_receiver) = mpsc::channel();
    let (events, event_receiver) = mpsc::channel();
    let worker = thread::spawn(move || capture_worker(config, input_receiver, events));
    let mut screen = Screen::default();
    let result: Result<()> = (|| {
        loop {
            while let Ok(event) = event_receiver.try_recv() {
                if let Some(result) = screen.apply(event) {
                    return result.map_err(Into::into);
                }
            }
            terminal.draw(|frame| screen.draw(frame, &database))?;
            // Leave keyboard events queued while a write is in flight. Rapid typing
            // cannot bypass the controller's commit acknowledgement or lose a line.
            if screen.ready && event::poll(Duration::from_millis(40))? {
                match event::read()? {
                    Event::Key(key)
                        if key.kind == KeyEventKind::Press
                            || (key.kind == KeyEventKind::Repeat
                                && matches!(
                                    key.code,
                                    KeyCode::Char(_)
                                        | KeyCode::Backspace
                                        | KeyCode::Delete
                                        | KeyCode::Left
                                        | KeyCode::Right
                                        | KeyCode::Home
                                        | KeyCode::End
                                )) =>
                    {
                        if screen.key(key.code, key.modifiers, &input)? {
                            return Ok(());
                        }
                    }
                    Event::Paste(text) => screen.editor.paste(&text),
                    _ => {}
                }
            } else if !screen.ready {
                match event_receiver.recv_timeout(Duration::from_millis(40)) {
                    Ok(event) => {
                        if let Some(result) = screen.apply(event) {
                            return result.map_err(Into::into);
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        return Err("capture worker disconnected".into());
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                }
            }
        }
    })();
    drop(input); // EOF wakes the controller, even when terminal rendering failed.
    let worker_result = worker.join().map_err(|_| "capture worker panicked")?;
    while let Ok(event) = event_receiver.try_recv() {
        screen.apply(event);
    }
    drop(terminal);
    drop(restore);
    for line in screen.log.iter().skip(screen.log.len().saturating_sub(3)) {
        println!("{line}");
    }
    if let Some(pending) = &screen.view.pending {
        println!(
            "Unacknowledged measurement: {pending}. Resolve committed state before re-entering it."
        );
    }
    if screen.submitted {
        println!(
            "Last submission has no acknowledgement: {}. Inspect committed state before retrying.",
            display_text(&screen.editor.text)
        );
    } else if !screen.editor.text.is_empty() || !screen.editor.queued.is_empty() {
        println!(
            "Retained draft: {} ({} additional pasted lines).",
            display_text(&screen.editor.text),
            screen.editor.queued.len()
        );
    }
    result?;
    worker_result.map_err(Into::into)
}

fn capture_worker(
    config: CliOptions,
    input: Receiver<String>,
    events: Sender<ScreenEvent>,
) -> std::result::Result<(), String> {
    let mut reader = ScreenReader {
        sender: events.clone(),
        receiver: input,
        buffer: Cursor::new(Vec::new()),
    };
    let mut writer = ScreenWriter {
        sender: events.clone(),
        buffer: Vec::new(),
    };
    let result = run_observed(
        config,
        &mut reader,
        &mut writer,
        &mut Observer(events.clone()),
    )
    .and_then(|()| writer.flush().map_err(Into::into))
    .map_err(|error| error.to_string());
    let _ = events.send(ScreenEvent::Finished(result.clone()));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};

    struct Harness {
        directory: tempfile::TempDir,
        input: Option<Sender<String>>,
        events: Receiver<ScreenEvent>,
        worker: Option<thread::JoinHandle<std::result::Result<(), String>>>,
        screen: Screen,
    }
    impl Harness {
        fn new() -> Self {
            let directory = tempfile::tempdir().unwrap();
            let config = CliOptions {
                database: directory.path().join("inspection.sqlite3"),
                reports_directory: directory.path().join("reports"),
                report_notes: vec!["SYNTHETIC SCREEN TEST".into()],
            };
            let (input, lines) = mpsc::channel();
            let (events, receiver) = mpsc::channel();
            let worker = thread::spawn(move || capture_worker(config, lines, events));
            let mut harness = Self {
                directory,
                input: Some(input),
                events: receiver,
                worker: Some(worker),
                screen: Screen::default(),
            };
            harness.ready();
            harness
        }
        fn ready(&mut self) {
            loop {
                let event = self
                    .events
                    .recv_timeout(Duration::from_secs(10))
                    .expect("controller did not request input");
                let ready = matches!(event, ScreenEvent::Ready);
                assert!(
                    self.screen.apply(event).is_none(),
                    "controller stopped unexpectedly"
                );
                if ready {
                    break;
                }
            }
        }
        fn key(&mut self, key: KeyCode) {
            assert!(
                !self
                    .screen
                    .key(key, KeyModifiers::NONE, self.input.as_ref().unwrap())
                    .unwrap()
            );
            if !self.screen.ready {
                self.ready();
            }
        }
        fn line(&mut self, text: &str) {
            self.screen.editor.text.clear();
            self.screen.editor.cursor = 0;
            self.screen.editor.insert(text);
            self.key(KeyCode::Enter);
        }
        fn setup(&mut self) {
            self.key(KeyCode::F(2));
            self.key(KeyCode::F(3));
            for value in ["00405363-µ", "9368.9", "5", "0.1", "yes"] {
                self.line(value);
            }
            self.key(KeyCode::F(4));
        }
        fn rendered(&self, width: u16, height: u16) -> String {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| self.screen.draw(frame, "synthetic.sqlite3"))
                .unwrap();
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect()
        }
        fn assert_progress(&self, measured: usize) {
            for (width, height) in [(120, 36), (80, 24), (60, 18)] {
                let text = self.rendered(width, height);
                assert!(
                    text.contains(&format!("Bundle #1 — {measured}/42 measured")),
                    "missing counter at {width}x{height}"
                );
                assert_eq!(
                    text.matches('●').count(),
                    measured,
                    "filled positions at {width}x{height}"
                );
                assert_eq!(
                    text.matches('○').count(),
                    42 - measured,
                    "empty positions at {width}x{height}"
                );
            }
        }
    }
    impl Drop for Harness {
        fn drop(&mut self) {
            self.input.take();
            if let Some(worker) = self.worker.take() {
                assert!(worker.join().unwrap().is_ok());
            }
        }
    }

    #[test]
    fn screen_preserves_paste_and_invalid_input_and_displays_committed_corrections() {
        let mut app = Harness::new();
        app.setup();
        app.assert_progress(0);
        app.screen.editor.paste("68\r\n=9367.2\r\nbad\r\n71\r\n");
        app.key(KeyCode::Enter);
        assert_eq!(app.screen.view.active, 1);
        assert_eq!(app.screen.editor.text, "=9367.2");
        assert_eq!(app.screen.editor.queued.len(), 2);
        app.key(KeyCode::Enter);
        assert_eq!(app.screen.view.active, 2);
        app.key(KeyCode::Enter);
        assert_eq!(app.screen.editor.text, "bad");
        assert_eq!(app.screen.editor.queued.front().unwrap(), "71");
        assert_eq!(app.screen.view.rows.len(), 2);
        app.assert_progress(2);
        assert!(app.screen.log.iter().any(|line| line.starts_with("Error:")));
        app.line("68"); // Repair the invalid line, leaving the subsequent pasted line queued.
        app.key(KeyCode::Enter);
        assert_eq!(app.screen.view.active, 4);
        app.assert_progress(4);
        assert!(!app.rendered(120, 36).contains("Committed measurements"));
        app.key(KeyCode::F(10));
        let rendered = app.rendered(120, 36);
        for expected in [
            "00405363-µ",
            "4/42 saved",
            "9367.2 mm",
            "IN SPEC",
            "Committed measurements",
        ] {
            assert!(rendered.contains(expected), "missing {expected}");
        }
        app.key(KeyCode::F(10));
        assert!(!app.rendered(120, 36).contains("Committed measurements"));
        app.key(KeyCode::F(6));
        assert!(app.screen.prompt.contains("Undo sequence 4"));
        assert_eq!(app.screen.view.active, 4); // Confirmation itself never changes data.
        app.line("yes");
        app.line("Synthetic undo");
        assert_eq!(app.screen.view.active, 3);
        app.assert_progress(3);
        assert_eq!(app.screen.view.rows[0].status, "INVALIDATED");
        app.line("/correct 4 =9370");
        app.line("Synthetic correction");
        assert_eq!(app.screen.view.active, 4);
        app.assert_progress(4);
        assert!(app.screen.view.rows[0].status.contains("corrected"));
        assert!(!app.rendered(120, 36).contains("Committed measurements"));
        let store = SqliteStore::open(app.directory.path().join("inspection.sqlite3")).unwrap();
        let snapshot = store.session_snapshots(SessionId::new(1).unwrap()).unwrap();
        let measurements = &snapshot[0].bundles[0].measurements;
        assert_eq!(measurements.len(), 4);
        assert_eq!(measurements[3].original().input().raw(), "71");
        assert_eq!(measurements[3].corrections().len(), 2);
        assert_eq!(measurements[3].effective().unwrap().0.raw(), "=9370");
    }

    #[test]
    fn screen_capacity_resize_help_and_eof_preserve_the_open_inspection() {
        let mut app = Harness::new();
        app.setup();
        for _ in 0..42 {
            app.line("68");
        }
        app.line("71");
        assert_eq!(app.screen.view.active, 42);
        assert_eq!(app.screen.view.rows.len(), 42);
        app.assert_progress(42);
        assert_eq!(app.screen.editor.text, "71");
        assert!(app.rendered(120, 36).contains("42/42 FULL"));
        let narrow = app.rendered(80, 24);
        assert!(narrow.contains("LSL 9363.9"));
        assert!(narrow.contains("Resolution 0.1 mm"));
        app.key(KeyCode::PageUp);
        assert_eq!(app.screen.row_offset, 8);
        for (width, height) in [(120, 36), (80, 24), (60, 18), (1, 1), (0, 0)] {
            app.rendered(width, height);
            assert_eq!(app.screen.editor.text, "71");
        }
        app.key(KeyCode::F(1));
        assert!(app.rendered(120, 36).contains("Keyboard guide"));
        assert!(app.rendered(80, 24).contains("/exit leaves context open"));
        app.key(KeyCode::F(5)); // Help modal cannot accidentally close a bundle.
        assert!(app.screen.view.bundle_open);
        app.key(KeyCode::Esc);
        app.input.take(); // EOF, not an implicit /close.
        assert!(app.worker.take().unwrap().join().unwrap().is_ok());
        let store = SqliteStore::open(app.directory.path().join("inspection.sqlite3")).unwrap();
        let recovered = store.recover_session(SessionId::new(1).unwrap()).unwrap();
        assert_eq!(recovered.session.status(), SessionStatus::Active);
        assert_eq!(
            store
                .bundle_capacity(recovered.open_bundle.unwrap().id())
                .unwrap()
                .active(),
            42
        );
    }

    #[test]
    fn unicode_editing_preserves_byte_boundaries_and_queued_lines() {
        let mut editor = Editor::default();
        editor.paste("零µ\n68\n");
        editor.left();
        editor.backspace();
        assert_eq!(editor.text, "µ");
        editor.insert("é");
        editor.delete();
        assert_eq!(editor.text, "é");
        editor.accepted();
        assert_eq!(editor.text, "68");
        assert!(editor.queued.is_empty());
    }

    #[test]
    fn failed_screen_capture_keeps_the_draft_and_retry_commits_exactly_once() {
        let mut app = Harness::new();
        app.setup();
        let connection =
            rusqlite::Connection::open(app.directory.path().join("inspection.sqlite3")).unwrap();
        connection.execute_batch("CREATE TRIGGER synthetic_failure BEFORE INSERT ON measurements BEGIN SELECT RAISE(ABORT,'synthetic write failure'); END;").unwrap();
        app.line("68");
        assert_eq!(app.screen.editor.text, "68");
        assert_eq!(app.screen.view.pending.as_deref(), Some("68"));
        assert_eq!(app.screen.view.active, 0);
        app.assert_progress(0);
        assert!(app.rendered(120, 36).contains("UNACKNOWLEDGED: 68"));
        connection
            .execute_batch("DROP TRIGGER synthetic_failure;")
            .unwrap();
        app.line("/retry");
        assert_eq!(app.screen.view.active, 1);
        app.assert_progress(1);
        assert_eq!(app.screen.view.rows.len(), 1);
        assert_eq!(app.screen.view.rows[0].raw, "68");
        assert!(app.screen.view.pending.is_none());
        app.line("/retry");
        assert_eq!(app.screen.view.active, 1);
        assert_eq!(app.screen.view.rows.len(), 1);
    }

    #[test]
    fn screen_resume_rebuilds_diagram_after_a_committed_undo() {
        let mut app = Harness::new();
        app.setup();
        app.line("68");
        app.line("=9380"); // Out-of-spec observations are still saved and fill a position.
        app.assert_progress(2);
        app.key(KeyCode::F(6));
        app.line("yes");
        app.line("Synthetic undo before restart");
        app.assert_progress(1);
        app.input.take();
        assert!(app.worker.take().unwrap().join().unwrap().is_ok());
        let config = CliOptions {
            database: app.directory.path().join("inspection.sqlite3"),
            reports_directory: app.directory.path().join("reports"),
            report_notes: Vec::new(),
        };
        let (input, lines) = mpsc::channel();
        let (events, receiver) = mpsc::channel();
        app.input = Some(input);
        app.events = receiver;
        app.worker = Some(thread::spawn(move || capture_worker(config, lines, events)));
        app.screen = Screen::default();
        app.ready();
        app.line("/resume 1");
        app.assert_progress(1);
        assert_eq!(app.screen.view.rows.len(), 2); // History is retained; only one position is filled.
    }
}
