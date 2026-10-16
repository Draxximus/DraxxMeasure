---
name: rust-architecture
description: Design or review DraxxMeasure Rust crate boundaries, domain interfaces, ownership, and errors. Use for architectural changes and CLI/SQLite/Tauri integration boundaries, not UI layout or statistical formulas alone.
---

# Rust architecture

## Responsibilities and activation

Use for new modules, dependency direction, public APIs, or cross-layer changes.
Own the separation of domain policy, application use cases, and adapters. Follow
the project `AGENTS.md` invariants; do not scaffold the application unless asked.

## Procedure

1. Inspect existing manifests, modules, tests, and decisions before proposing a
   structure. Identify the requested use case and its failure/recovery boundaries.
2. Model Part Number as a string (preserve leading zeros), explicit dimensional
   units/precision, specification revisions, session-part associations, bundles,
   and traceable measurements. Delegate their exact invariants to
   [inspection-data-model](../inspection-data-model/SKILL.md) and
   [dimensional-metrology](../dimensional-metrology/SKILL.md).
3. Start with modules; split crates only for a concrete boundary or build benefit.
   A possible dependency direction is CLI/Tauri adapters → application → domain,
   with SQLite implementing persistence needed by application use cases. Domain
   must not import Tauri, terminal, plotting, or SQLite APIs.
4. Define small operations such as start/resume session, select session-part,
   start bundle, record measurement, correct measurement, close session, and read
   a report snapshot. Specify inputs, outputs, state transitions, and commit
   semantics before choosing traits. Use traits only where substitution helps.
5. Keep parsing, reconstruction, tolerance decisions, and statistics deterministic.
   Inject time, identifiers, and storage at the boundary when tests require it.
   UI adapters translate events and display results; they do not own calculations.
6. Prefer owned domain values at persistence/IPC boundaries and borrows for local
   reads; avoid global mutable state, unnecessary clones, and exposed lifetimes
   that complicate ordinary callers. Introduce concurrency or async only for a
   demonstrated blocking or coordination need.
7. Distinguish validation, ambiguity, invalid state, persistence, and export errors.
   Return typed recoverable errors; preserve causes at adapter boundaries. Do not
   panic on operator input, file errors, or corrupt storage, or swallow failures.
8. Record a short decision with alternatives, dependency cost, compatibility,
   and test strategy when choosing a consequential interface or dependency.

## Project conventions

- Keep specification snapshots immutable for recorded observations; changing a
  current Part Number specification must not reinterpret history.
- Put commit acknowledgement and correction atomicity in application contracts,
  not terminal rendering callbacks or frontend state.
- Prefer stable explicit value objects to a generic plugin/repository framework.
- Keep reports usable from the CLI without initializing a webview.

## Validation and acceptance

- Show the dependency direction and public interfaces with error/state contracts.
- Demonstrate that a headless consumer can record and analyze measurements.
- Explain how session-part ownership permits multiple Part Numbers per session.
- For implemented changes, run relevant tests, formatting, and lint checks using
  [rust-testing](../rust-testing/SKILL.md); do not claim builds for docs-only plans.
- Reject dependency cycles, GUI-owned business rules, and abstractions without a
  concrete use case. Consult the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
  for public-interface questions, not as a requirement to maximize API surface.
