---
name: rust-testing
description: Design or run DraxxMeasure Rust unit, integration, property, persistence, recovery, and report tests. Use for correctness, data-integrity regression prevention, malformed input, or validation of changes across application layers.
---

# Rust testing

## Responsibilities and activation

Own evidence of correctness and reliability. Follow project `AGENTS.md`; prioritize
traceability and data integrity over premature benchmarks. Do not add an application
or test framework merely because this instruction-only skill was requested.

## Procedure

1. Inspect current behavior, manifests, toolchain, fixtures, and CI before adding
   tests. Turn the requested change into observable invariants and identify the
   responsible layer. Test public contracts rather than private implementation shape.
2. Add deterministic unit tests for parsers, units, exact rounding/precision,
   reconstruction, tolerance boundaries, statistical formulas, and state transitions.
   Use independent expected values from the relevant specialist references:
   [reconstruction](../dimensional-metrology/references/reconstruction.md) and
   [statistics](../quality-statistics/references/numerical-contract.md).
3. Cover malformed/empty/oversized input, leading zeros, rollover, ambiguity,
   no-candidate/full overrides, overflow, resolution mismatch, and exact LSL/USL
   plus one instrument increment beyond. Never use floating epsilon to redefine
   specification membership.
4. Add integration tests spanning session → session-part → bundles → measurements,
   cross-bundle corrections, repeated equal values, transitions, and multi-part
   ownership. Verify raw input, stable IDs, specification snapshots, order, and
   retained audit history, not only returned counts.
5. Use temporary file-backed SQLite databases for reopen, WAL/foreign-key behavior,
   migrations with populated historical fixtures, failed commits, and recovery.
   Test an uncertain-commit retry with the same request ID and legitimate equal
   measurements with distinct IDs. Inject disk/permission/lock failures safely;
   keep all failure tests away from real inspection databases.
6. Test crash points before/after commit and during transitions/corrections using
   process-level termination or a controlled fault harness where warranted. State
   what is actually simulated; dropping a connection alone is not a power-loss test.
   Verify acknowledged records survive under the documented durability contract,
   uncommitted work is not reported as saved, and recovery never deletes evidence.
7. Use property tests when they add coverage: reconstruction is unique-or-explicit,
   replay is deterministic, correction leaves originals, histogram counts sum to n,
   total SS equals within+between, and translations preserve variance within
   documented numerical tolerance. Add Proptest or similar only for concrete need;
   retain regression seeds and bounded generators with meaningful preconditions.
8. Test PNG decoding, labels/limits, empty/large layouts, snapshot-consistent CSV,
   quoting/Unicode/raw input, and export failures using
   [the report contract](../technical-reporting/references/report-contract.md).
   Use pinned fonts/backend for visual tests and assert data independently of pixels.
9. Keep terminal/IPC adapter tests separate from domain tests. Exercise Windows
   terminal/path behavior when supported, and mark platform gaps honestly. Inject
   clocks/IDs where necessary; avoid wall-clock sleeps and network-dependent tests.

## Validation and acceptance

- When Rust code exists, run repository-defined checks. Typical workspace checks:
  `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  and `cargo test --workspace`. Exercise documented headless/desktop feature sets
  separately; do not assume `--all-features` is valid or available on every host.
- If no Cargo manifest exists, state that Rust checks are not applicable; do not
  fabricate passing application tests or scaffold code just to run them.
- Report commands, results, platform, skipped cases, and remaining risks. A failed
  check is not a pass; distinguish pre-existing failures from changes under review.
- Every bug fix has a failing-then-passing regression where practical. Coverage
  percentages do not substitute for recovery and integrity assertions.
- For framework/API questions consult the [Rust testing guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
  and the chosen test library's versioned docs.
