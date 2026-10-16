# DraxxMeasure

Lightweight, local-first industrial dimensional inspection, built in Rust.
The interface is a keyboard-first CLI; a future Tauri 2 desktop
adapter will reuse the same headless domain, storage, analysis, and reporting.

Repository: https://github.com/Draxximus/DraxxMeasure

```text
git clone https://github.com/Draxximus/DraxxMeasure.git
cd DraxxMeasure
```

## Phase 2 status

**Implemented:** hierarchical interactive capture, multiple Part Numbers and bundles,
exact input reconstruction with explicit operator-approved intervals, immediate
SQLite commits, recovery, traceable undo/correction, descriptive statistics, and
automatic paginated English PNG reports with exact verification CSVs and a manifest.

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
files, never `data/` or `reports/`. Every published Windows archive is gated by
native verification in the Windows Actions workflow.

## Build and run from source (developers only)

Install stable Rust **1.97 or newer**, including rustfmt and Clippy. Development
was verified with stable Rust/Cargo 1.97.1 on Linux. The workspace uses edition 2024
with minimal purpose-specific dependencies: bundled SQLite, exact JSON audit
payloads, UUID request IDs, Rust-native Plotters/PNG encoding, UTC formatting, and
safe temporary-file publication. Core still uses only the standard library. No
Python runtime, Tauri, terminal dashboard framework, or async runtime is required.
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
session> /close
✓ Session closed
✓ PNG report generated
Report: reports/session-001.png
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
- Session closure automatically exports all parts. On export failure the session
  remains closed and data stays intact. Use `/report ID`, or restart with a writable
  `--reports-dir`. Retries never overwrite prior outputs; collision-safe names are used.

Reports use up to four bundles per page, with no fixed overall page/bundle/part
limit. CSV contains original, correction, and effective rows. For verification,
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
inspection and release-ZIP isolation; a green run is required before a Windows
archive can be published. Host-only checks do not substitute for that native run.

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

See [the synthetic sample report](docs/examples/session-001.png) and its
[manifest](docs/examples/session-001.manifest.json). These are explicitly labeled
synthetic examples, not production inspection records. The replayable input and
verification notes are in [docs/examples](docs/examples/README.md).

## Remaining work, subject to approval

Add an operator-approved backup/restore workflow,
instrument/calibration identity integration, richer terminal editing, and measured
large-session export optimizations. Tauri/SPC/capability analysis remain separate
future work; no additional phase starts automatically.
