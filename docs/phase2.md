# Phase 2 — Implemented capture and reporting

## Boundaries

```text
draxx-cli → draxx-application → draxx-storage → draxx-core
draxx-cli → draxx-reporting → draxx-statistics → draxx-core
draxx-reporting → draxx-core
draxx-application → draxx-core
```

Application use cases are now concrete, so `draxx-application` is the small shared
orchestration package anticipated by ADR 0001. Neither core nor storage depends
on it. No terminal callbacks own reconstruction/calculation/transaction policy.
The CLI directly invokes headless export after closure; a future GUI can use the
same service and snapshot/export APIs without reproducing formulas in JavaScript.

## Context and lifecycle

Root `/session` creates an Active inspection. Session `/partnumber` selects an
immutable specification item; Part Number `/bundle` opens the next independent
bundle. Numeric lines in a bundle capture immediately, without a command prefix.

`/close` changes only one level: bundle → Part Number → session → root. Part-level
closure is **navigation**, persisted in `session_context`, not catalog deletion or
a new global Part Number status. A part can be reselected later in the same session.
Session closure is sealed, requires no open bundle and no selected part context,
and is committed before attempting exports. Empty sessions/items can be reported;
empty bundles cannot close and must receive valid observations before closure.

At most one bundle is open per session. Session items/bundles/measurements use
dynamic collections and positive 64-bit IDs/order keys, without a fixed global
count limit. Timestamps are UTC epoch milliseconds, not ordering keys.

## The 42-tube invariant

`BundleCapacity` in core validates occupancy, append, closure, invalidation, and
restoration. A Closed bundle requires 1..42 effective observations. Capacity is
rechecked under `BEGIN IMMEDIATE` in storage; defensive SQL triggers also guard
raw inserts/restoration. No state/sequence/commit record for a rejected #43 is
persisted. Reaching 42 never closes the bundle automatically.

An original row is immutable. Invalidation removes an effective observation, not
its history, and can free a slot. Capturing another tube then gets the next capture
sequence (possibly >42) while the active count stays <=42. Restoring a prior
invalidated observation fails if it would make 43. Replacing an active value does
not change occupancy. A Closed bundle cannot be invalidated down to zero.

## Input policy

Setup requests nominal, positive symmetric tolerance, explicit positive resolution,
and explicit approval of an abbreviation interval. LSL/USL are calculated exactly;
negative LSL or overflow is rejected. Engineering parameters need not align with
instrument ticks; accepted observations must align exactly with supplied resolution.

`integer-mm-window` version 1 interprets two ASCII digits as the last two integer-mm
digits. Candidates are `100*k+d` in the approved inclusive interval. The CLI offers
an interval from `max(0,floor(nominal_mm)-50)` to that lower integer plus 99 mm,
but **does not enable it until the operator answers `yes`**. The operator may enter
custom bounds or `off`. These bounds are not tolerance limits or evidence that
actual tubes must fall there. Full overrides are necessary outside those assumptions.

Zero candidates requests a full override; multiple candidates produce an ambiguity
message and save nothing. More than 1024 candidates is an explicit error, not
truncation/guessing. `=9368.0` bypasses abbreviation bounds, but not representation
or resolution checks. Unsupported policy names/versions fail. Storage revalidates
claimed interpretations, so a client cannot falsely label ambiguity as automatic
uniqueness. Valid out-of-spec values remain accepted and visible.

No whitespace trimming, locale conversion, inferred nominal fraction, or silent
rounding changes a measurement. Only transport CRLF/LF line endings are removed.
Raw text, exact integer-nm values, policy name/version/bounds and selection mode
are preserved per observation/event.

## Concrete SQLite schema and migration

The executable source of truth is
[`schema.sql`](../crates/draxx-storage/src/schema.sql), not the historical proposal
in `persistence.md`. It contains normalized parts, specification revisions,
sessions, session items, bundles, measurements, corrections, a commit ledger,
an exact migration-SQL ledger, item policy settings, and persisted selected context.
JSON payloads hold exact integer values/provenance in addition to constrained
relational identity/order/value columns. Domain constructors validate rehydrated
values; original/correction payloads are checked against their value columns.

Foreign keys are enabled and verified on each opened connection. Composite keys
enforce bundle/item/session/specification ownership. Unique indexes enforce per-
bundle capture sequences, per-item bundle sequences, a single correction root/
successor, request identity, and one open bundle per session. Original observations,
corrections, specifications, identities, and commit/migration records cannot be
updated/deleted through the schema's immutable-history guards.

New empty databases receive migration 1 transactionally and `user_version=1`.
The complete migration SQL, including triggers, is stored and compared on reopen.
Nonempty unversioned databases, modified migration text, corrupt foreign keys,
failed quick checks, and unsupported newer schemas are refused without resetting
files. No legacy data upgrade is needed from Phase 1, which created no database.
There is no speculative migration to an unknown future schema; future upgrades
must add backup, validation, and populated-version tests before changing data.

All writes use a bounded busy timeout, WAL and `synchronous=FULL`; opening fails
if required settings cannot be verified. Use a local file database, not `:memory:`
or a network-share WAL file. A C toolchain is needed to build bundled SQLite, but
the running application does not need a separately installed SQLite/Python runtime.

## Transactions, retries and recovery

Mutations allocate checked IDs/sequences and insert a versioned canonical JSON
command, immutable response, and affected entities in one write transaction. A
receipt is returned only after COMMIT. A UUID request token replays the original
result if its entire payload/operation matches; mismatched reuse conflicts.
Identical values with different request IDs are legitimate separate observations.
Mutable lifecycle receipts preserve their original state even after later closure.

COMMIT/response-loading uncertainty retains the underlying cause and request token.
The application keeps failed measurement input in memory and `/retry` reuses the
same command/token; further mutations are blocked until resolved. That draft is
not claimed durable across process death. After a restart, inspect recovered
committed observations before re-entering any input whose acknowledgement was lost.
Database corruption errors never cause automatic deletion, rebuilding or repair.

Recovery restores the selected item, open bundle, exact specification/policy and
effective occupancy from committed state. EOF, `/exit`, or process termination
does not silently close a context. `/resume ID` explicitly resumes a suspended
session or continues an Active one; it never reopens a Closed session. No raw-mode
terminal state needs restoration in this line-oriented adapter.

## Statistics and export

Statistics analyze effective observations within each item/specification, never
pool different parts/revisions. Centered compensated two-pass mean/variance avoids
large-offset cancellation. Exact integer comparisons determine OOS; sample SD
uses n-1. Median/extrema/range, low/high/total OOS, percentage and bundle summaries
are provided, with explicit unavailable reasons for empty/singleton data. Values/
nominal above 2^53 nanometres are rejected for descriptive analysis rather than
silently losing integer precision; dimensional storage itself supports i64 range.
Histograms use explicit increasing integer-nm edges covering data and specifications,
left-inclusive bins and an inclusive final right edge. Counts include all OOS data.

Session report snapshots are read in one transaction. Plotters produces English
1800x2000 PNG pages, up to four bundle panels per page, repeating population-wide
summary/histogram and showing detail bundle means/min-max ranges. Other items
always get separate pages. All effective points appear, including OOS points and
original sequence gaps. Limits are engineering limits, not statistical control
limits; observed ranges are not confidence intervals. A bundled licensed font
removes the runtime dependency on system font discovery.

Each CSV includes original rows, linked correction rows, and explicit effective
rows. Filter effective rows with nonempty values to reproduce results; preserve
Part Numbers as text and never count audit events as extra samples. Canonical raw
`=`/other strings are not silently sanitized for spreadsheets: import as text.
The manifest records exact specification context, count/statistical definitions,
histogram edges/counts, revision, generation date, export identity and output files.

Files are staged in the destination directory, flushed, and published without
clobbering existing names. The complete manifest is published last; interrupted
sets without it are not complete. An ordinary failure removes only outputs created
by that failed attempt; a later retry uses fresh collision-safe names. No report
failure changes inspection data or reopens the session. `/report ID` retries a
Closed session, including after restart with another `--reports-dir`.

## Verified behavior and limitations

Tests cover exact reconstruction/rollover/ambiguity, 42/43 capacity, restoration,
immutable corrections, transaction rollback, idempotency/conflicts, competing
writers, multi-part end-to-end sessions, dynamic bundle counts, process termination
after acknowledgement, report failure/retry/no overwrite, CSV round-trips and
independent statistical recomputation, PNG decoding/panels, and pagination.

Only Linux execution has been verified. Windows uses portable paths/line input and
Rust-native font rendering but still needs native testing. Process termination is
not a hardware power-loss certification. WAL durability depends on correct local
filesystem/storage behavior. Automated backup/restore, authenticated operators,
instrument/calibration integration, SPC/capability analysis, closed-session
amendments and a desktop UI are not implemented. Reports are a new functional
layout; the original historical visual reference was not supplied. Report memory
currently scales with the output set; streaming and extreme-size layout testing
are future measured optimizations, not claims of constant memory or infinite RAM.
