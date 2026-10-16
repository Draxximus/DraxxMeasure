# ADR 0004 — Explicit reconstruction policy, no nearest-value default

- Status: Interface/grammar accepted; candidate selection policy pending operational approval
- Date: 2026-10-16

## Context

The example 9368.9 mm nominal with `68` → 9368 mm does not uniquely define a
general reconstruction rule, especially near hundreds rollover or with wide limits.
Filtering by tolerance could hide genuine out-of-spec measurements.

## Decision

Separate exact input validation from a deterministic reconstruction engine port.
Plain two ASCII digits are abbreviation mode; `=decimal` is an explicit full-mm
override. Preserve both raw text and policy provenance. Distinguish Unique,
Ambiguous, Unresolved, and typed failures. No production engine in Phase 1.

Propose integer-mm suffix candidates in explicit independently approved inclusive
capture bounds, with checked cardinality/arithmetic and explicit operator
resolution/full override. Persist policy/version/bounds and selection mode.
Do not choose by nearest nominal, previous sample, or tolerance membership.

## Alternatives and consequences

Nearest-nominal heuristics are fast but can turn an outlier into a conforming
fabricated measurement; specification-window filtering has the same danger.
Always-full entry is unambiguous but adds keystrokes. The proposed approach keeps
the fast path while making uncertainty visible, at the cost of explicit interval
configuration and full entry at some boundaries. Production approval, instrument
resolution, operator feedback, and exhaustive reconstruction tests remain Phase 2
prerequisites. The documented examples are synthetic fixtures, not inspection data.
