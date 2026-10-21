# DraxxMeasure

Lightweight, local-first industrial dimensional inspection, built in Rust.
The interface is a keyboard-first CLI; a future Tauri 2 desktop
adapter will reuse the same headless domain, storage, analysis, and reporting.

## Phase 2 status

**Implemented:** hierarchical interactive capture, multiple Part Numbers and bundles,
exact input reconstruction with explicit operator-approved intervals, immediate
SQLite commits, recovery, traceable undo/correction, descriptive statistics, and
automatic English PNG reports per Part Number with exact verification CSVs and a manifest.
Interactive terminals open a Ratatui capture screen with the active specification,
bundle capacity, setup prompts, and keyboard help. Press F10 to view measurement history.

**Mandatory limit:** at most **42 active measurements per bundle**, enforced by
core rules, write transactions, and SQL triggers. A 43rd active observation is
rejected without persistence. Full bundles remain open until `/close`. Closed
bundles require 1..42 active observations. Overall collections are dynamic.

No desktop UI, SPC/capability claims, measurement-system validation, closed-session
amendments, automated backup/restore, or regulatory certification is provided.

## Portable Windows download (no Rust/Python/Tauri installation)

See [Windows deployment and release instructions](docs/windows-deployment.md).
Download the versioned **windows-x64.zip** from GitHub Releases Assets, extract it
to a writable local folder, and launch `draxxmeasure.exe`. The package includes
the static-CRT release executable, usage instructions, build identity and font
license, **not** databases or reports. Updates replace only executable/documentation
files, never `data/` or `reports/`. Native verification is performed by the new
Windows Actions workflow; it has not been executed on this Linux host.

## Build and run from source (developers only)

Install stable Rust **1.97 or newer**, including rustfmt and Clippy. Development
was verified with stable Rust/Cargo 1.97.1 on Linux. The workspace uses edition 2024
with minimal purpose-specific dependencies: bundled SQLite, exact JSON audit
payloads, UUID request IDs, Rust-native Plotters/PNG encoding, UTC formatting, and
safe temporary-file publication. Core still uses only the standard library. No
Python runtime, Tauri, or async runtime is required. Ratatui 0.30.2 and Crossterm
0.29 provide the terminal screen and cross-platform keyboard handling.
Building bundled SQLite requires a C compiler (on Windows, an appropriate MSVC
Build Tools installation). PNG text uses a licensed bundled DejaVu Sans font,
not platform font discovery or a browser.
Include `crates/draxx-reporting/assets/FONT-LICENSE.txt` when distributing binaries.

From this directory (commands also work in PowerShell):

```text
cargo build --workspace --locked
cargo run -p draxx-cli -- --help
cargo run -p draxx-cli -- --version
cargo run -p draxx-cli
cargo run -p draxx-cli -- --line
cargo run -p draxx-cli -- --tui --data-dir data
cargo run -p draxx-cli -- --data-dir data
cargo run -p draxx-cli -- --database data/inspection.sqlite3 --reports-dir reports
```

The binary is named `draxxmeasure`; no arguments start interactive capture. Portable
defaults are `data/draxxmeasure.sqlite3` and `reports/` **beside the executable**, not
relative to the terminal or workspace. For `cargo run`, this is normally under
`target/debug/`; pass explicit paths to keep development data elsewhere.
`--data-dir DIR` selects `DIR/draxxmeasure.sqlite3` and `DIR/reports`; individual
`--database` and `--reports-dir` overrides take precedence in any argument order.
Explicit relative flags are resolved against the current directory. Unwritable
locations fail visibly without silently selecting a different database. Existing
working-directory-relative installations should explicitly select their old absolute
database path before updating; no automatic data move or replacement occurs.
Unsupported startup arguments exit with code 2 before opening storage. Use `/help`
for interactive commands.

## Terminal screen

Run `cargo run -p draxx-cli` in a terminal, or launch `draxxmeasure.exe` on Windows.
The screen opens automatically when both input and output are terminals. Use
`--tui` to require it, or `--line` for the plain interface. Redirected streams and
`TERM=dumb` select line mode automatically. The full layout needs 72 columns and
24 rows; smaller terminals show a compact prompt and preserve the draft on resize.

Press **F2** to start a session, **F3** to set up a Part Number, and **F4** to start
a bundle. Enter `68` or `=9368.9`, then press **Enter**. **F5** closes one context
level; **F6** starts confirmed undo; **F7** retries a pending write; **F8** exports
the last closed Part Number. **F9** refreshes status. **F1** opens keyboard help.
All existing slash commands, including `/resume ID`, work in the entry field.

Use arrows, Home/End, Backspace/Delete, and Ctrl+U to edit. Invalid input stays
editable. Multiline paste queues the remaining lines: press Enter once per line,
including setup fields. A failed line does not consume the next queued line.
Page Up/Down scroll measurement history; the newest entries appear first.
Ctrl+C, Ctrl+D, and `/exit` leave committed context recoverable; they do not close
the inspection. Drafts and queued paste are held in memory. Terminal modes are
restored when the screen exits. See [screen design and behavior](docs/terminal-screen.md).

Every open bundle shows 42 tube positions in five staggered layers, **8–9–8–9–8**.
`○` means unmeasured; `●` means measured and committed. Positions fill left to
right, top to bottom, using the current effective SQLite measurement count.
Confirmed `/undo` frees a position, and `/resume ID` rebuilds progress from saved
data. At 42/42 the bundle remains open until `/close`. Both indicators are monochrome.
The plain `--line` interface prints a fresh diagram only when the bundle or count
changes; the Ratatui screen updates the arrangement in place. Measurement history
is hidden by default; F10 toggles the table when needed.

## Example inspection

```text
> /session
session> /partnumber
Part Number: 405363
Nominal length (mm): 9368.9
Tolerance (±mm): 5
Instrument resolution (mm): 0.1
Approve interval [9318, 9417] mm? yes
405363> /bundle
405363/bundle-1> 68
✓ 9368 mm saved [1/42]
405363/bundle-1> 71
✓ 9371 mm saved [2/42]
405363/bundle-1> =9367.2
✓ 9367.2 mm saved [3/42]
405363/bundle-1> /close
405363> /close
✓ PNG report generated
Report: reports/session-001-part-405363-item-001.png
session> /close
✓ Session closed
```

Setup explicitly asks for instrument resolution and approval of abbreviation
bounds; neither is silently inferred from nominal precision or tolerance limits.
Instead of `yes`, enter `off` or explicit `LOWER UPPER` bounds. The suggested
100-integer-mm interval is independent of LSL/USL. An ambiguous abbreviation is
never guessed: use `=full_length` or change `/policy LOWER UPPER` explicitly.
Valid out-of-spec values are saved and clearly marked. No rounding is performed.

Repeat `/partnumber` at session level to add another Part Number. `/close` at a
Part Number exits that context; it does not delete or globally retire the part.
Reselect with `/partnumber CODE` in the same session. Bundle sequences are per
session item and measurement sequences are per bundle.

### Corrections, recovery, and report retries

- `/undo` shows the last active entry in the current bundle and asks for confirmation.
- `/correct SEQUENCE VALUE` requires a reason; originals and all events remain stored.
- Undo frees a slot; restoration cannot increase occupancy above 42. Capture
  sequence gaps are never renumbered, so historical row count may exceed 42 after
  explicit invalidation and replacement capture while active count remains <=42.
- `/exit`, EOF, or unexpected termination leaves committed context recoverable.
  Restart and use `/sessions`, then `/resume ID`. Draft/unacknowledged input is
  not promised durable; resolve recovered state before re-entering uncertain input.
- On a storage submission error, pending input stays in memory; `/retry` uses the
  exact original request ID. New measurements and context mutation are blocked
  while that pending write is unresolved.
- Closing a Part Number exports all of its bundles immediately, while the session
  remains active. On export failure, the context stays closed and data stays intact.
  Use `/report` to retry the last closed part or `/report part ITEM_ID` after restart,
  optionally with another `--reports-dir`. Retries never overwrite prior outputs.
- Session closure does not generate another automatic report. `/report SESSION_ID`
  remains available to explicitly export all parts of a closed session.

Each Part Number/specification gets one 1800×2000 PNG combining all bundles into a
histogram, an ordered measurement plot, and an overall summary. Files are named
with a safe Part Number label plus session/item IDs. Reselecting and closing a part
again exports a fresh cumulative snapshot with a new collision-safe name. Bundle
identities remain in CSV; unlike parts or specification revisions are never pooled.
CSV contains original, correction, and effective rows. For verification,
filter `row_kind=effective` and nonempty `value_mm`; audit rows are not extra samples.
Import canonical CSV columns as **text**: preserved operator inputs beginning with
`=` can be interpreted as formulas by spreadsheets. The manifest is published last
and identifies a complete export set; interrupted sets without it are incomplete.

## Development checks

```text
cargo fmt --all -- --check
cargo check --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

Use `cargo fmt --all` to format changes and `cargo doc --workspace --no-deps` to
read public API contracts. Commit `Cargo.lock` for reproducible application builds.
Unsafe Rust, debug macros, and unfinished `todo!` implementations are prohibited
by workspace lints. Tests use synthetic data and isolated temporary SQLite databases,
including process termination, competing writers, failed writes, and real PNG decoding.
Copied-executable isolation and binary-only update preservation are covered by
host tests. The Actions workflow performs native Windows tests, static CRT/DLL
inspection and release-ZIP isolation; a green run is needed to verify a Windows
release. This Linux development session cannot claim that remote run occurred.

## Workspace

| Package | Responsibility |
| --- | --- |
| `draxx-core` | Exact dimensions, entities, lifecycle guards, input/provenance contracts |
| `draxx-storage` | Transactional SQLite, immutable audit receipts, recovery and snapshots |
| `draxx-statistics` | Stable descriptive statistics and exact histogram bin membership |
| `draxx-reporting` | Headless Plotters PNGs, verification CSVs, safe export publication |
| `draxx-application` | Reusable capture/navigation/correction/retry use cases |
| `draxx-cli` | Portable line-oriented interactive adapter and startup arguments |

Every bundle is a group of measurements: it corresponds to a spreadsheet column,
with measurements ordered top to bottom. Sessions contain inspection items that
select immutable Part Number/specification snapshots; each item has many bundles.

## Design documentation

- [Architecture and dependency boundaries](docs/architecture.md)
- [Portable Windows download, data paths, updates and publishing](docs/windows-deployment.md)
- [Current Phase 2 behavior, schema and limitations](docs/phase2.md)
- [Domain model and dimensional invariants](docs/domain-model.md)
- [Session/bundle lifecycle](docs/lifecycle.md)
- [Original SQLite design and current migration reference](docs/persistence.md)
- [Reconstruction and precision contract](docs/reconstruction.md)
- [Statistics and report contracts](docs/analysis-reporting.md)
- [Architectural decisions](docs/decisions/0001-workspace-boundaries.md)
- [Project agent guidance](AGENTS.md), with skills under `.agents/skills/`

## Verified example artifacts

See [the legacy format-1 synthetic sample report](docs/examples/session-001.png) and its
[manifest](docs/examples/session-001.manifest.json). These are explicitly labeled
synthetic examples, not production inspection records. The replayable input and
verification notes are in [docs/examples](docs/examples/README.md). Replay generates
the current format-2 reports, one per Part Number at Part Number closure.

## Remaining work, subject to approval

Run the new Windows release workflow on GitHub, add an operator-approved backup/restore workflow,
instrument/calibration identity integration, richer terminal editing, and measured
large-session export optimizations. Tauri/SPC/capability analysis remain separate
future work; no additional phase starts automatically.
