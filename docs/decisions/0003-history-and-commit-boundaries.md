# ADR 0003 — Immutable observations, append-only history, commit-before-ack

- Status: Accepted design; SQLite implementation deferred
- Date: 2026-10-16

## Context

Thousands of repeated values are legitimate. Crashes can occur after commit but
before feedback; undo cannot mean erasing inspection evidence. One session may
contain several Part Numbers/specification revisions and many bundles.

## Decision

Use session items linking immutable specifications; bundles belong to one item,
measurements have stable IDs and per-bundle sequences. Corrections append linked
events with a reason and expected head, never rewrite originals. Invalidation
excludes an effective sample without deletion; replacement can explicitly restore
it. Corrections require Active sessions, even for Closed bundles; Closed sessions
are sealed. Preserve empty bundles and sequence gaps.

Use one synchronous atomic storage port. Each command has a durable request token
and canonical payload. Assign ID/sequence and audit revision in the same write
transaction. Acknowledge only COMMIT success; resolve unknown outcomes with the
same token. Repeat values with different tokens remain separate observations.
Consistent owned snapshots feed analysis/reporting without live mutable queries.

## Alternatives and consequences

Overwriting values and buffered bundle commits are simpler but lose history or
acknowledged observations. Independent CRUD repositories expose unsafe transaction
gaps. Full event sourcing for every domain entity is unnecessary; a commit audit
ledger plus immutable measurement/correction rows is sufficient initially.

At most one Open bundle per session simplifies keyboard context and recovery;
many closed bundles and multiple items remain supported. Local methods cannot
enforce this global constraint, ID uniqueness, or stale-head checks: the adapter
must enforce them transactionally with constraints and integration tests. One read
snapshot is reproducible when exported; historical database time-travel is not
promised. Closed-session amendments require a separately designed later workflow.

Validate transaction failure/uncertain replay, repeated values, multi-item ownership,
correction chains, sealed closure, crash recovery, and populated migrations before
claiming the SQLite implementation meets these guarantees.
