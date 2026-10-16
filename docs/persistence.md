# SQLite persistence — original Phase 1 proposal

**Historical design, not the shipped migration.** Phase 2 now implements
`draxx_storage::SqliteStore` with file-backed tests. The executable source of truth
is [`schema.sql`](../crates/draxx-storage/src/schema.sql); operational differences,
transaction/recovery guarantees and limits are in [Phase 2](phase2.md). The original
proposal below is retained for rationale and must not be executed as a current
migration. The Rust ports remain `InspectionStore` and `SchemaMigrator`.

## Schema target

IDs map to positive signed INTEGER primary keys; Part Numbers and request tokens
are exact TEXT with binary comparison. Dimensions are integer nanometres; UTC
times are integer epoch milliseconds. Use SQLite STRICT tables to avoid accidental
TEXT/REAL coercion in dimensional columns (requires SQLite >= 3.37). Driver/version
selection is deferred. Every connection must enable and verify foreign keys.

```sql
PRAGMA foreign_keys = ON;

CREATE TABLE commits (
    revision INTEGER PRIMARY KEY AUTOINCREMENT CHECK (revision > 0),
    request_id TEXT NOT NULL UNIQUE CHECK (length(trim(request_id)) > 0),
    operation TEXT NOT NULL,
    canonical_payload BLOB NOT NULL,
    response_payload BLOB NOT NULL,
    result_entity_type TEXT NOT NULL,
    result_entity_id TEXT NOT NULL,
    recorded_at INTEGER NOT NULL
) STRICT;

CREATE TABLE part_numbers (
    part_number TEXT PRIMARY KEY NOT NULL CHECK (length(trim(part_number)) > 0),
    created_revision INTEGER NOT NULL REFERENCES commits(revision)
) STRICT;

CREATE TABLE specifications (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    part_number TEXT NOT NULL REFERENCES part_numbers(part_number),
    version INTEGER NOT NULL CHECK (version > 0),
    nominal_nm INTEGER NOT NULL CHECK (nominal_nm >= 0),
    lsl_nm INTEGER NOT NULL CHECK (lsl_nm >= 0),
    usl_nm INTEGER NOT NULL CHECK (usl_nm >= 0),
    resolution_nm INTEGER NOT NULL CHECK (resolution_nm > 0),
    unit TEXT NOT NULL CHECK (unit = 'mm'),
    created_revision INTEGER NOT NULL REFERENCES commits(revision),
    CHECK (lsl_nm < usl_nm AND lsl_nm <= nominal_nm AND nominal_nm <= usl_nm),
    UNIQUE (part_number, version),
    UNIQUE (id, part_number)
) STRICT;

CREATE TABLE sessions (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    status TEXT NOT NULL CHECK (status IN ('active', 'suspended', 'closed')),
    created_at INTEGER NOT NULL,
    closed_at INTEGER,
    created_revision INTEGER NOT NULL REFERENCES commits(revision),
    updated_revision INTEGER NOT NULL REFERENCES commits(revision),
    CHECK ((status = 'closed' AND closed_at IS NOT NULL)
        OR (status <> 'closed' AND closed_at IS NULL))
) STRICT;

CREATE TABLE session_items (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    session_id INTEGER NOT NULL REFERENCES sessions(id),
    specification_id INTEGER NOT NULL,
    part_number TEXT NOT NULL,
    created_revision INTEGER NOT NULL REFERENCES commits(revision),
    FOREIGN KEY (specification_id, part_number) REFERENCES specifications(id, part_number),
    UNIQUE (session_id, specification_id),
    UNIQUE (id, session_id, specification_id)
) STRICT;

CREATE TABLE bundles (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    item_id INTEGER NOT NULL,
    session_id INTEGER NOT NULL,
    specification_id INTEGER NOT NULL,
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    label TEXT NOT NULL CHECK (length(trim(label)) > 0),
    status TEXT NOT NULL CHECK (status IN ('open', 'closed')),
    started_at INTEGER NOT NULL,
    closed_at INTEGER,
    created_revision INTEGER NOT NULL REFERENCES commits(revision),
    updated_revision INTEGER NOT NULL REFERENCES commits(revision),
    FOREIGN KEY (item_id, session_id, specification_id)
        REFERENCES session_items(id, session_id, specification_id),
    CHECK ((status = 'closed' AND closed_at IS NOT NULL)
        OR (status = 'open' AND closed_at IS NULL)),
    UNIQUE (item_id, sequence),
    UNIQUE (id, item_id, session_id, specification_id)
) STRICT;
CREATE UNIQUE INDEX one_open_bundle_per_session ON bundles(session_id) WHERE status = 'open';

CREATE TABLE measurements (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    session_id INTEGER NOT NULL,
    item_id INTEGER NOT NULL,
    bundle_id INTEGER NOT NULL,
    specification_id INTEGER NOT NULL,
    sequence INTEGER NOT NULL CHECK (sequence > 0),
    raw_input TEXT NOT NULL,
    input_mode TEXT NOT NULL CHECK (input_mode IN ('abbreviated', 'full')),
    value_nm INTEGER NOT NULL CHECK (value_nm >= 0),
    unit TEXT NOT NULL CHECK (unit = 'mm'),
    policy_name TEXT,
    policy_version INTEGER,
    capture_lower_nm INTEGER,
    capture_upper_nm INTEGER,
    operator_selected INTEGER NOT NULL CHECK (operator_selected IN (0, 1)),
    captured_at INTEGER NOT NULL,
    instrument_id TEXT,
    calibration_reference TEXT,
    operator TEXT,
    created_revision INTEGER NOT NULL REFERENCES commits(revision),
    FOREIGN KEY (bundle_id, item_id, session_id, specification_id)
        REFERENCES bundles(id, item_id, session_id, specification_id),
    UNIQUE (bundle_id, sequence),
    CHECK (
        (input_mode = 'full' AND substr(raw_input, 1, 1) = '=' AND length(raw_input) > 1
            AND policy_name IS NULL AND policy_version IS NULL
            AND capture_lower_nm IS NULL AND capture_upper_nm IS NULL AND operator_selected = 0)
        OR
        (input_mode = 'abbreviated' AND raw_input GLOB '[0-9][0-9]'
            AND policy_name IS NOT NULL AND length(trim(policy_name)) > 0
            AND policy_version IS NOT NULL AND policy_version > 0
            AND capture_lower_nm IS NOT NULL AND capture_lower_nm >= 0
            AND capture_upper_nm IS NOT NULL AND capture_upper_nm >= capture_lower_nm
            AND value_nm BETWEEN capture_lower_nm AND capture_upper_nm)
    )
) STRICT;

CREATE TABLE corrections (
    id INTEGER PRIMARY KEY CHECK (id > 0),
    measurement_id INTEGER NOT NULL REFERENCES measurements(id),
    previous_id INTEGER,
    action TEXT NOT NULL CHECK (action IN ('replace', 'invalidate')),
    replacement_raw_input TEXT,
    replacement_input_mode TEXT CHECK (replacement_input_mode IN ('abbreviated', 'full')),
    replacement_value_nm INTEGER CHECK (replacement_value_nm >= 0),
    replacement_policy_name TEXT,
    replacement_policy_version INTEGER,
    replacement_capture_lower_nm INTEGER,
    replacement_capture_upper_nm INTEGER,
    replacement_operator_selected INTEGER CHECK (replacement_operator_selected IN (0, 1)),
    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),
    actor TEXT,
    recorded_at INTEGER NOT NULL,
    created_revision INTEGER NOT NULL REFERENCES commits(revision),
    FOREIGN KEY (previous_id, measurement_id) REFERENCES corrections(id, measurement_id),
    CHECK (previous_id IS NULL OR previous_id <> id),
    CHECK (
        (action = 'replace' AND replacement_raw_input IS NOT NULL
            AND replacement_input_mode IS NOT NULL AND replacement_value_nm IS NOT NULL
            AND replacement_operator_selected IS NOT NULL)
        OR
        (action = 'invalidate' AND replacement_raw_input IS NULL
            AND replacement_input_mode IS NULL AND replacement_value_nm IS NULL
            AND replacement_policy_name IS NULL AND replacement_policy_version IS NULL
            AND replacement_capture_lower_nm IS NULL AND replacement_capture_upper_nm IS NULL
            AND replacement_operator_selected IS NULL)
    ),
    UNIQUE (id, measurement_id)
) STRICT;
CREATE UNIQUE INDEX one_first_correction ON corrections(measurement_id) WHERE previous_id IS NULL;
CREATE UNIQUE INDEX one_correction_successor ON corrections(previous_id) WHERE previous_id IS NOT NULL;
CREATE INDEX corrections_by_measurement ON corrections(measurement_id, created_revision);
CREATE INDEX items_by_specification ON session_items(specification_id);
CREATE INDEX measurements_by_item_revision ON measurements(item_id, created_revision);
CREATE INDEX measurements_by_revision ON measurements(created_revision);
CREATE INDEX corrections_by_revision ON corrections(created_revision);
CREATE INDEX active_sessions ON sessions(status, id);
```

The immutable policy algorithm for the listed fields is integer-mm suffix;
`policy_name`/version identify its approved configuration. Additional algorithms
will need a migration and typed policy variant, not arbitrary executable names.
Full decimal grammar, suffix arithmetic, resolution alignment, replacement policy
field consistency, and current lifecycle checks are validated by Rust within the
write transaction. SQL CHECKs are defense in depth, not a duplicate domain engine.
In particular, correction replacement policy fields must follow the same full/
abbreviated consistency rules as original input, enforced by the adapter tests.

All FKs use restrictive default deletion behavior; no cascade erases observations.
The first migration must also install immutable UPDATE/DELETE guards on
`commits`, `part_numbers`, `specifications`, `session_items`, `measurements`, and
`corrections`, plus DELETE guards for `sessions`/`bundles`. Bundle ownership/order/
creation fields and session creation fields cannot be updated. Lifecycle fields
are updated only through the checked transaction API. These guards are specified
here, not yet executable migrations. Immutability plus inserting a successor only
after its predecessor exists prevents cycles; composite FK and unique indexes
prevent cross-measurement links and forks. Adapter validation rejects any attempt
to extend a nonlatest event. No correction represents another sample.

## Index and ownership rationale

`UNIQUE(item_id, sequence)` orders bundles; `UNIQUE(bundle_id, sequence)` orders
measurements. Both support next-sequence lookup and ordered reads. Invalidation
never deletes the original, so indices cannot be reused. Sequence/ID exhaustion
returns an explicit error instead of wrapping. SQLite allocation/rowid behavior
must be pinned in adapter tests; external IDs cannot be assumed globally unique.

Composite FKs carry session/item/specification ownership down to each measurement,
so a valid bundle ID cannot be paired with a different session or specification.
The one-open-bundle partial index implements the initial capture policy without
limiting the number of session items or completed bundles. A repeated Part Number
under a new specification can use a separate item; an item is unique by session
and specification. Query the item via its Part Number/specification, never infer
bundle ownership from spreadsheet positions.

## Atomic transactions and idempotency

Every mutation gets one `commits` row and one positive global revision inside
the same transaction. The row is also the audit/idempotency record for lifecycle
events. `canonical_payload` contains a versioned, stable encoding of the complete
command, including its supplied timestamp. `response_payload` contains a versioned
encoding of the original successful response, excluding the derived replay flag.
Lifecycle request replay returns that event's response, not whatever state the
mutable session has later reached. Allocate positive IDs/revision with checked
arithmetic under the write lock before inserting the ledger row and related
entities; no increment may silently promote to REAL on overflow. The encoding is
a Phase 2 decision and must be deterministic and round-trip exact dimensional data.

1. Begin a write transaction (initial candidate: `BEGIN IMMEDIATE`).
2. Look up request token. Identical stored command returns its original result;
   mismatched content is Conflict. Never deduplicate by measured value/time.
3. Reload authoritative entities. Check status/ownership/resolution and expected
   correction head; count open bundles before session closure. Allocate IDs and
   sequence under the same write lock. Reject counter exhaustion explicitly.
4. Insert the commit row and all entity/audit/state changes. A single measurement
   request inserts one measurement, not a buffered batch waiting for bundle close.
5. COMMIT. Only successful COMMIT produces `CommitReceipt`. Rollback on failure;
   never advance the CLI's accepted count before success.
6. If connection/I/O failure makes the outcome uncertain, return CommitUnknown.
   Reopen safely, lookup/retry **the same** request token and payload. Do not
   automatically retry an uncertain operation with a newly generated identity.

State transitions and correction creation must not be implemented as unprotected
read-then-write calls. `InspectionStore` deliberately exposes atomic use-case
operations rather than public `begin/save/commit` pieces that UI code can misuse.

## Durability and recovery policy for the first adapter

- Verify `foreign_keys=ON` for every connection; candidate settings are WAL plus
  `synchronous=FULL`, with a documented bounded busy timeout. Never choose OFF to
  make capture feel faster. Check actual negotiated settings and SQLite version.
- Measure the candidate settings on supported filesystems, including Windows.
  WAL and FULL improve crash/power-loss durability but cannot guarantee against
  faulty hardware, filesystem behavior, or externally edited files.
- On startup, open without destructive repair, validate schema version and run
  appropriate quick/integrity/foreign-key checks. Preserve damaged files and report
  errors; never reset a database to make startup succeed.
- Recover the requested session, its items, committed context, and optional open
  bundle in a consistent read. Do not infer that a crash changed Active to
  Suspended. Uncommitted input is unknown/lost draft text, not acknowledged data.
- The initial database design is for a local disk, not a network-share WAL database.
  Use SQLite online backup (or a properly coordinated supported equivalent);
  copying only the main WAL database file is unsafe. Restore requires verification
  in an isolated path before replacing any live data, with user authorization.

## Migrations and snapshots

Maintain immutable numbered migrations and schema version via `PRAGMA user_version`
plus a future migration checksum ledger. Current code ships no executable schema
version. The first adapter supplies migration 1 and the ledger; refuse newer
schemas and modified old migration checksums rather than guessing compatibility.
Back up before risky upgrades. Apply supported upgrades transactionally where
SQLite allows, validate foreign keys/integrity, then advance the version. Test each
supported prior populated schema; never drop/recreate a user's database on error.

Read all parts of an `InspectionSnapshot` in one transaction, taking its revision
from the same read view. Include original observations and every correction up
to that cutoff, sorted by immutable sequence/event linkage; effective values are
derived by core history. Empty bundles and invalidated originals remain present.
Validate the snapshot before returning it, then reuse the owned object for analysis,
CSV, and PNG. Historical time-travel reads are not promised; persist exported
verification data/metadata when an exact historical report must be reproduced.

## Required Phase 2 verification

File-backed reopen/WAL tests; malformed/cross-owner inserts; foreign-key checks on
each connection; competing writers and sequence allocation; same-request replay
and changed-payload conflicts; equal values with different requests; disk-full/
permission/lock failures; interruption immediately before/after commit; correction
forks and immutable guards; safe close; populated migrations and newer-schema
refusal; consistent snapshots, online backup and verified restoration. DDL parsing
alone is not evidence for any of these runtime guarantees.
