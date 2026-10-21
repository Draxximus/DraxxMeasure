# Ratatui inspection screen

Interactive stdin/stdout open the terminal screen automatically. `--tui` requires
both streams to be terminals and fails before opening a database if either is
redirected. `--line` selects the plain interface. Automatic mode also selects
plain input for `TERM=dumb`. Existing data path flags retain their precedence and
portable defaults beside the executable.

```text
┌ Capture workspace ───────────────────────────────────────────────────────────┐
│ DRAXXMEASURE     Dimensional inspection · Local SQLite                        │
├ Active inspection ────────────────────────────────┬ Bundle capacity ─────────┤
│ Session 1 / Part 00405363 / Bundle 1 (ID 1)        │          2/42 saved      │
│ Nominal 9368.9 mm · LSL 9363.9 · USL 9373.9        │                          │
│ Approved suffix window: 9318 .. 9417 mm            │                          │
├ Tube arrangement · 8–9–8–9–8 ──────────┬ Committed measurements ──────────────┤
│ Bundle #1 — 2/42 measured             │ Seq  Value       Status             │
│                                      │ 2    9367.2 mm   IN SPEC            │
│    ●   ●   ○   ○   ○   ○   ○   ○     │ 1    9368 mm     IN SPEC            │
│  ○   ○   ○   ○   ○   ○   ○   ○   ○   │                                    │
│    ○   ○   ○   ○   ○   ○   ○   ○     │                                    │
│  ○   ○   ○   ○   ○   ○   ○   ○   ○   │                                    │
│    ○   ○   ○   ○   ○   ○   ○   ○     │                                    │
├ Activity · commit receipts and errors ───────────────────────────────────────┤
│ ✓ 9367.2 mm saved [2/42] (ID 2, sequence 2)                                   │
├ Current prompt ─────────────────────────────────────────────────────────────┤
│ 00405363/bundle-1>                                                           │
├ Entry · 0 pasted lines queued · Enter submits one line ───────────────────────┤
│ 71                                                                          │
└─────────────────────────────────────────────────────────────────────────────┘
```

This is an illustrative text layout using synthetic values. The full screen
needs at least 72 × 24 cells; smaller terminals show a compact prompt, draft and
arrangement when space permits. Wider layouts add a capacity gauge and full history
columns when history is enabled with F10; the history table is hidden by default
so the bundle graphic remains the primary progress display. Outside a bundle,
wider layouts show a shortcut sidebar. F1 opens the
complete keyboard guide; F1, Esc, or Enter dismisses it. The help modal never
submits a command. Resize preserves current input and queued paste.

| Context | Input | Effect |
| --- | --- | --- |
| Command prompt | F2 / `/session` | Start a session at root |
| Session prompt | F3 / `/partnumber` | Existing setup/reselection prompts |
| Part prompt | F4 / `/bundle` | Explicitly start a bundle |
| Bundle prompt | Two digits or `=full_length`, Enter | Record through the shared application service |
| Command prompt | F5 / `/close` | Close one level; Part Number closure exports PNG/CSV |
| Bundle prompt | F6 / `/undo` | Display target, request yes/no and reason |
| Bundle prompt | `/correct SEQUENCE VALUE` | Request a reason and append a correction |
| Pending write | F7 / `/retry` | Replay the original in-memory command and request ID |
| Command prompt | F8 / `/report` | Retry the last Part Number closed in this process |
| Command prompt | `/report part ITEM_ID` | Export/retry a part with closed bundles, even in an active session |
| Command prompt | `/report ID`, `/resume ID`, `/sessions` | Existing export/recovery commands |
| Command prompt | F9 / `/status` | Refresh the committed view |
| Setup prompt | `/cancel` | Cancel the current setup/confirmation |
| Any prompt | Ctrl+C / Ctrl+D | EOF to the controller, preserving open committed context |

Function keys that submit commands apply only at a command prompt with an empty
entry and no queued lines. A draft is retained if a shortcut would replace it;
submit or clear it with Ctrl+U first. All slash commands are also available as text.

The editor supports Unicode character insertion/deletion, arrows, Home/End, and
Ctrl+U. Validation failures keep the submitted text editable. Bracketed multiline
paste normalizes transport line endings and queues every remaining line; each
Enter submits exactly one. A failed line stays in place and does not advance the
queue. Rapid Enter on an empty field does not duplicate the previous measurement.

## Bundle arrangement

The reusable `bundle_view::render_bundle(Sequence, BundleCapacity)` function in
`draxx-cli` produces plain Unicode text with exactly 42 circles in staggered
8–9–8–9–8 layers. It uses only existing core value types and the standard library.
`○` is unmeasured and `●` is measured and saved; indicators have no tolerance colors.

Both terminal adapters supply committed effective occupancy. Every new capture
fills the next position in row order after its SQLite write is acknowledged.
Validation and failed writes do not advance progress. Confirmed invalidation
reduces the filled count; correction/restoration follows the effective count.
`/resume ID` reconstructs it from SQLite, and reaching 42 never closes a bundle.

This is a progress arrangement: filled positions are packed in capture order by
effective count. Historical rows and sequence gaps do not reserve physical slot
IDs, so undo followed by another capture still produces exactly 42 positions.
Original identities and correction history remain visible in the history table.

In the full screen the layout reserves enough height for all five tube layers,
including at 80 × 24. Narrow history tables show sequence, value and status; wider
ones also show ID and original input. Smaller terminal windows preserve the diagram
text and draft in the compact view, but need enough cells to display them completely.
Ratatui diffs the frame; no extra terminal clears are introduced for progress.
The plain line interface appends a complete diagram only when bundle identity or
effective count changes. Rejected input and `/status` do not repeat an unchanged
diagram, and no cursor-control escapes are emitted by the plain interface.

F10 toggles the measurement table, which shows committed history in the current bundle, newest first,
including invalidated entries and effective corrected values. Page Up/Down scrolls
history; the table title identifies which historical rows are visible. Active
occupancy is separate from historical row count. Classification is supplied by
the exact domain comparison; out-of-spec entries are labeled as well as colored.
The activity pane retains the latest 200 output lines in memory; SQLite remains
the authoritative complete inspection history.

One worker owns the existing synchronous command controller and storage. The UI
waits for a new input request after each submission and leaves keyboard events
queued while a commit/export is in progress. The controller emits rejection
feedback explicitly; the screen does not infer success by parsing receipt text.
Only committed views update counts and table rows. Failed captures display an
unacknowledged banner and retain the existing `/retry` contract. Pending writes,
drafts, and pasted lines are not durable across process death.

Normal exits and returned errors restore terminal modes through a guard. Ratatui's
initialization installs its panic restoration hook. Ctrl+C during a write waits
for that controller operation to finish before the queued key is handled. Forced
termination cannot run cleanup; committed data is recovered through `/resume ID`.

Automated tests cover the actual channel/controller/SQLite path, invalid input,
paste ordering, Unicode editing, capacity, confirmed undo, correction history,
failed writes and retry, EOF recovery, and rendering at several terminal sizes.
Process tests cover forced-screen rejection of redirected IO and escape-free line
mode. Native Windows terminal behavior still requires a Windows console check;
redirected Windows package checks continue to exercise line mode.
