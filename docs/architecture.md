# Architecture

## Scope and status

Phase 2 implements the headless domain, SQLite storage, shared capture use cases,
descriptive statistics, PNG/CSV reporting, and an interactive CLI. Tauri remains
absent. The original five packages isolate correctness-critical domain logic,
persistence, numerical work, and rendering; a sixth small application package now
holds actual reusable orchestration. See [current behavior and constraints](phase2.md).

## Dependencies

Actual manifest edges (arrow means **depends on**):

```text
draxx-storage ────────────────────────────> draxx-core
draxx-statistics ─────────────────────────> draxx-core
draxx-reporting ──> draxx-statistics ──────> draxx-core
draxx-reporting ──────────────────────────> draxx-core
draxx-application ──> draxx-core + draxx-storage
draxx-cli ──────────> draxx-application + draxx-core + draxx-storage + draxx-reporting
```

Application wiring:

```text
CLI (future Tauri adapter) → shared application use cases → core + storage port
CLI (future Tauri adapter) → statistics/reporting services → core snapshots
SQLite adapter implements InspectionStore in draxx-storage
```

Core does not know terminal, filesystem, SQL, GUI, or reporting APIs. Statistics
does not depend on storage; reporting consumes already-calculated results rather
than calculating them again. Storage does not depend on the CLI or reporting.
No cycles exist. Core has no external dependencies; adapter dependencies have
specific storage/rendering/identity/publication purposes. Tauri is deliberately absent.

## Boundaries and interfaces

- `draxx-core`: immutable `PartSpecification`/`Measurement`, `Length`/`Resolution`,
  typed IDs, session/bundle methods, correction history, `MeasurementReconstructor`,
  and an owned `InspectionSnapshot` read model with structural validation.
- `draxx-storage`: `InspectionStore` is one synchronous transaction-oriented port,
  not independent CRUD repositories that expose partially completed use cases.
  `CommitReceipt<T>` signifies successful commit; `CommitUnknown` demands lookup
  or retry using the same request ID. `SchemaMigrator` specifies future upgrades.
- `draxx-statistics`: `StatisticsEngine`, explicit unavailable metrics, exact
  extrema/range, descriptive floating outputs in mm, and histogram edge contracts.
- `draxx-reporting`: `ReportInput` references one snapshot and its derived results;
  `PngExporter` returns one raster page per Part Number/specification and
  `CsvExporter` writes to a stream. Part context closure invokes `export_part` from
  a committed snapshot; session closure does not duplicate automatic exports.
  The headless export service publishes a no-clobber file set and manifest; no
  inspection writes occur.
- `draxx-application`: context navigation, capture/reconstruction, explicit
  corrections and same-request retry over the storage port.
- `draxx-cli`: portable startup/slash-command parsing, line input, and a Ratatui
  screen using Crossterm. Both presentations use the same capture controller.
  A single worker owns the synchronous controller/SQLite connection while the
  screen thread handles events and draws committed read views. Channels carry
  complete input lines, rejection feedback, and output; no terminal callback owns
  transaction or measurement rules. See [ADR 0007](decisions/0007-ratatui-screen.md).

`DomainError`, `ReconstructionError`, `StorageError`, `AnalysisError`, and
`ReportError` distinguish recoverable failure classes. Wrapped backend/I/O causes
are retained. Operator input does not trigger panics or silent fallback values.

## Implemented measurement use case

1. Read the active session/item/bundle and immutable specification snapshot.
2. Validate input without trimming/reinterpreting original operator text.
3. Reconstruct through an approved policy; stop for ambiguity/no candidate or
   request a full override. Never reject a value merely for being out of spec.
4. Send `RecordMeasurement` with a stable request ID to the storage port.
5. Inside one write transaction, reload/validate authoritative state, allocate ID
   and per-bundle sequence, persist observation/provenance and commit record.
6. Only after a successful receipt, show the committed value/ID/count. If outcome
   is unknown, resolve the same request; do not blindly issue a new ID.

This orchestration lives in `draxx-application`, not terminal callbacks. Core does
not depend on storage. This preserves the same commit semantics for a future
desktop consumer. Capacity is a domain rule checked under the same write lock.

## Ownership, concurrency, and portability

Domain entities own their values; ordinary queries borrow them. Read snapshots are
owned and can outlive a database transaction without borrowed SQL rows. Time and
IDs are supplied externally, making unit tests deterministic. No global state,
unsafe Rust, async runtime, generic plugin framework, or unnecessary trait hierarchy.

Phase 1 is synchronous and single-operator oriented. Future SQLite write locking,
unique sequence indexes, and idempotency handle accidental competing submissions.
An authoritative count/previous correction must be checked within its write
transaction, never trusted from a stale GUI snapshot. At most one open bundle per
session is a deliberate initial policy, not a ban on many closed bundles/items.

Paths use Rust standard types; CLI argument handling uses `OsString`. Defaults are
executable-relative and explicit `--data-dir`/database/report overrides preserve
caller control. Tests copy the executable outside the workspace and verify
capture, PNG export and update preservation. The Windows Actions workflow builds
static-CRT MSVC releases and verifies extracted archives with restricted runtime
paths and OS-only DLL imports. It must run on GitHub before native Windows support
can be claimed; this development host is Linux. See [Windows deployment](windows-deployment.md).

## Verification and expansion

Tests include file-backed SQLite, competing writers, failed writes/idempotency,
actual process termination after acknowledgement, independent numerical fixtures,
PNG decoding/panel checks, CSV recomputation, and report retries. These do not
certify hardware power-loss behavior or untested Windows execution. See
[README](../README.md) for mandatory checks and [Phase 2](phase2.md) for limitations.

ADRs: [boundaries](decisions/0001-workspace-boundaries.md),
[exact dimensions](decisions/0002-exact-dimensions.md),
[history and transactions](decisions/0003-history-and-commit-boundaries.md),
[reconstruction](decisions/0004-explicit-reconstruction.md),
[Phase 2 implementation](decisions/0005-phase2-capture-and-reporting.md).
