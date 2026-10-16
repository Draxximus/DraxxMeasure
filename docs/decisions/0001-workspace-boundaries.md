# ADR 0001 — Small headless workspace and deferred application wiring

- Status: Accepted for Phase 1
- Date: 2026-10-16

## Context

The CLI is first, Tauri 2 later. Storage/numerical/plotting dependencies must not
enter the domain. The requested five packages represent distinct test/build
boundaries, but the capture application itself is outside this phase.

## Decision

Retain core, storage, statistics, reporting, and CLI packages. Use edition 2024,
resolver 3, stable Rust >=1.97 (verified 1.97.1), standard library only, shared
package metadata/lints, and a committed application lockfile. Core has no package
dependencies; storage/statistics depend on core; reporting depends on core and
statistics. The help/version CLI currently has no library dependency.

Define synchronous, use-case-sized storage ports in storage. Domain remains
independent. When real orchestration exists, share it outside CLI/Tauri, in a
small application module or package with core/storage dependencies. Do not add
that package, Tauri, async, or a generic repository framework speculatively.

## Alternatives and consequences

A single package would reduce manifests but blur future heavy backend boundaries.
A full hexagonal framework/application crate now would add empty abstractions.
This compromise preserves clear ownership with five small packages and no
external dependency cost. The CLI does not demonstrate a working capture use case
yet; that is intentional, not an implied implementation. The minimum Rust version
is a conservative tested baseline, not a claim older Rust could not compile it.

Validate with workspace fmt/check/test/Clippy and manifest dependency inspection;
add a reusable headless use-case integration test when capture is implemented.
