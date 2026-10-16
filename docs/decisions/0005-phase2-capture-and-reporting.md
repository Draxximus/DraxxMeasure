# ADR 0005 — Transactional interactive capture and automatic headless reporting

- Status: Accepted and implemented in Phase 2
- Date: 2026-10-16

## Decision

Add the concrete shared `draxx-application` package now that orchestration is real.
Keep a synchronous line-oriented CLI instead of adding a dashboard, async runtime,
or command framework solely for a small slash-command grammar. Standard line input
also supports redirected scripts and avoids Unix-only raw-mode assumptions.

Enforce 42 **active**, not historical, observations per bundle in core, the SQLite
write transaction and defensive triggers. Bundle closure requires 1..42 and is
explicit. This supersedes Phase 1's allowance for closing empty bundles. Part-level
`/close` exits a persisted selection context; it does not globally seal a catalog part.

Implement `integer-mm-window` v1 with explicit operator approval and resolution,
not a nearest-nominal or tolerance-window default. Use an explicit `=` full-entry
marker. Ambiguity/no candidate never silently selects a value. Original inputs,
immutable specifications, and append-only corrections preserve traceability.

Choose bundled rusqlite for consistent SQLite availability; use WAL/FULL, foreign
keys, immediate write transactions, exact migration SQL verification, UUID request
identity, and immutable canonical JSON responses. No SQLite REAL dimension storage.
The actual schema refines the Phase 1 proposal with JSON value provenance and
persisted context/policy tables, retaining normalized ownership/order constraints.

Generate Rust-native Plotters PNG pages, exact verification CSVs, and a last-published
manifest after session commit. Use the pure-Rust font backend with an unchanged,
licensed embedded DejaVu font. Publish through no-clobber temporary files; retries
produce fresh names instead of assuming multi-file atomic replacement.

## Tradeoffs and validation

Two explicit setup decisions add keystrokes before capture but prevent inferred
instrument precision and undocumented reconstruction assumptions. Normal capture
still requires only two digits and Enter. Default OS interruption can terminate
the process; committed state is recovered explicitly instead of depending on a
best-effort shutdown hook. Pending drafts are not promised durable.

The dependency-free core is retained; SQLite, serialization, rendering, fonts,
temporary publication, timestamps, and IDs are justified adapter/service costs.
Rendering currently collects the output set in memory, favoring a simple tested
implementation over premature streaming infrastructure. No future GUI or SPC work
is bundled into this phase. See [Phase 2](../phase2.md) for behavior, tests, and limits.
