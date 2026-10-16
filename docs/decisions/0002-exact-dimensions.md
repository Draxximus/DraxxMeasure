# ADR 0002 — Integer nanometres for exact dimensional data

- Status: Accepted for Phase 1
- Date: 2026-10-16

## Context

Binary floats can misclassify boundary values; an external decimal library adds
dependency/serialization decisions before they are needed. Nominal precision and
instrument resolution are not the same thing.

## Decision

Represent nonnegative lengths with i64 nanometres, mm input/output, and up to six
fractional mm digits. Reject unsupported precision and overflow without rounding.
Store and compare exact integers, preserving original input separately. Require
positive explicit resolution and exact alignment for observations. Specification
parameters need not align with instrument ticks. Nominal is within inclusive,
strictly ordered LSL/USL limits. No uncertainty guard band is implied.

## Alternatives and consequences

Per-specification variable scales complicate cross-snapshot comparison and SQLite
constraints. Decimal types would enable broader precision/unit support but are
unnecessary for the initial scope. i64 gives SQLite-compatible exact values and
no dependency, but caps precision/range and initially excludes signed dimensions
and non-mm entry. Adding units/finer precision needs an explicit migration/policy.

Descriptive statistics may return f64 in mm; they cannot feed exact conformance
decisions. Stable algorithms and extreme-range conversion tests remain required.
Raw string preservation and explicit errors avoid hidden data transformations.

Validate exact round trips, i64 boundaries, invalid grammar, resolution alignment,
and inclusive specification boundaries without tolerance-based comparisons.
