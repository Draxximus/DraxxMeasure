---
name: inspection-data-model
description: Design or review DraxxMeasure entities, SQLite schemas, transactional capture, correction history, migrations, and crash recovery. Use for persistence and session/Part Number/bundle ownership, not numerical formulas or terminal UI.
---

# Inspection data model

## Responsibilities and activation

Own persisted identity, lifecycle, and data integrity. Follow project `AGENTS.md`.
SQLite is authoritative; UI memory is not evidence that an observation was saved.

## Procedure

1. Inspect existing schema/migrations and model these relationships explicitly:
   Part Number → immutable specification revisions; Session → session-part
   associations → Bundles → Measurements. Each association selects a Part Number
   and specification revision; each bundle belongs to one association. Support
   several associations per session without requiring a multi-part UI initially.
2. Give all entities stable IDs. Store Part Number as text. Give bundles their own
   IDs and explicit sequence/label; a bundle groups observations and is not a
   measurement or spreadsheet row. Define scope/uniqueness of ordering keys.
3. Give each measurement a stable ID and monotonically allocated per-bundle
   capture sequence. Preserve sequence gaps after retractions; never renumber
   history. Timestamps carry timezone semantics but are not the ordering authority.
   If cross-bundle chronology is needed, add a session event order, not timestamp
   guesses. Define legal session and bundle state transitions.
4. Persist exact raw input, interpreted value/scale/unit, mode and interpretation
   version/context, specification reference, capture time, and available instrument/
   operator provenance. Explicitly mark unknown metadata. Define an effective
   current view plus immutable correction/retraction records with target IDs,
   timestamps, reasons, and actor when known. Never invent operator identity.
5. Make each accepted capture a transaction, including sequence allocation and
   necessary state changes. Acknowledge only after successful commit. Use explicit
   request IDs or another documented idempotency mechanism for retry after an
   uncertain commit; do not deduplicate by value, since repeated values are valid.
6. Enforce foreign keys on every SQLite connection and appropriate NOT NULL,
   UNIQUE, CHECK, and ownership constraints. Prevent references crossing a wrong
   session-part or bundle. Choose integer-scaled or exact-decimal persistence with
   [dimensional-metrology](../dimensional-metrology/SKILL.md), not SQLite REAL for
   exact specification comparisons. Avoid cascades that erase inspection history.
7. Select transaction, journal, synchronous, busy-timeout, and concurrency settings
   deliberately. Explain WAL/checkpoint and durability tradeoffs; do not disable
   synchronization for entry speed. Handle disk full, permissions, locks, and I/O
   failures visibly. Do not claim immunity to hardware/power failures.
8. Version migrations, test upgrades from each supported prior version, and protect
   existing data before risky changes. Prefer transactional migrations where valid;
   refuse unsupported newer schemas rather than guessing or rebuilding storage.
   Never automatically delete a database that fails to open or pass integrity checks.
9. Recover active sessions and bundle context from committed records. Expose any
   incomplete/uncommitted draft separately; never present it as saved. Use a
   consistent read transaction/snapshot for statistics and PNG/CSV exports with
   a recorded revision/cutoff. Back up live databases using SQLite-supported
   mechanisms; copying only the main file in WAL mode is not a safe backup plan.

## Validation and acceptance

- Provide an entity/relationship map, constraints, transaction boundaries, migration
  policy, and explicit correction/recovery semantics for the requested change.
- Prove multi-Part-Number sessions and multiple independent bundles fit the model.
- Use file-backed SQLite tests for reopen/recovery, foreign keys, duplicate request
  IDs, legitimate repeated values, concurrent ordering, failed writes, and upgrades.
- Verify corrections retain originals and reports select an explicit effective view.
- Coordinate with [rust-testing](../rust-testing/SKILL.md). Consult SQLite's
  [transactions](https://www.sqlite.org/lang_transaction.html),
  [WAL](https://www.sqlite.org/wal.html), and
  [online backup](https://www.sqlite.org/backup.html) docs when choosing durability
  and recovery behavior; record actual settings, not assumed driver defaults.
