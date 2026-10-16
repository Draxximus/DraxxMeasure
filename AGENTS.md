# DraxxMeasure project guidance

## Scope and skill discovery

DraxxMeasure is a lightweight, local-first dimensional inspection application planned
around Rust, SQLite, a keyboard-first CLI, and a future Tauri 2 desktop adapter.
The Phase 2 workspace implements interactive CLI capture, transactional SQLite,
descriptive statistics, and PNG/CSV reports. Do not implement later phases unless asked.
Do not treat these skills as authorization to implement unrequested features.

Project skills live in `.agents/skills/`. Launch Codex from `DraxxMeasure/` or a
descendant so it discovers them. Invoke explicitly with `$skill-name`, or let Codex
select by description. Read the relevant skill before specialized work; combine
only the skills needed for the task.

## Engineering invariants

1. Put data integrity first; never silently discard or modify inspection data.
2. Keep capture fast and keyboard-first; acknowledge a record only after commit.
3. Identify each bundle independently: a bundle groups measurements, not a row.
4. Trace each measurement to its original input, specification revision, bundle,
   session, interpretation rule, and any later correction.
5. Make calculations reproducible with explicit datasets, units, algorithms,
   precision, exclusions, and versions. Round for display, not classification.
6. Keep domain rules and calculations independent of CLI, Tauri, and JavaScript.
7. Remain lightweight and local-first; justify dependencies and network access.
8. Allow multiple Part Numbers per session without building unnecessary frameworks.
9. Distinguish observed data, calculated statistics, and interpretation in reports.
10. Preserve history and surface failed writes, missing data, and recovery status.
11. Portable defaults belong beside the executable, not in its working directory.
    Preserve explicit data path overrides. Packages contain executable/docs only;
    never include or replace inspection databases, data directories or reports.

## Operational constraints and unresolved decisions

- Abbreviated entry uses versioned `integer-mm-window` policies with explicit
  operator-approved bounds; never derive bounds from specification limits or
  silently guess a nearest value. Resolution is supplied explicitly during setup.
- Each bundle has at most 42 active measurements and must have 1..42 when closed.
  Enforce capacity in core and inside the write transaction, including restoration
  after invalidation. Reaching capacity never closes a bundle automatically.
- Instrument resolution, uncertainty decision rules, retention policy, and user
  identity requirements must be agreed when relevant, not invented.
- The previously established length-variation report artifact is not present.
  Request it before claiming visual fidelity; a proposed layout is not that artifact.
