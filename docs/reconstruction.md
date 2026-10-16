# Measurement reconstruction contract

## Implemented boundary

Core implements exact dimensional parsing, raw input mode validation, policy
configuration validation, and checking that a **claimed** interpretation matches
its raw input/policy. It defines `MeasurementReconstructor`, `ReconstructionOutcome`,
and typed errors. Phase 2 implements `SuffixReconstructor` candidate enumeration
and the capture use case. Automatic selection occurs only for one candidate in an
explicitly approved interval; ambiguity/unresolved input saves nothing. See
[Phase 2](phase2.md) for setup approval, persisted policies, and tested behavior.

The interface is deterministic: input + immutable specification + optional
explicit policy → Unique, Ambiguous, Unresolved, or a typed failure. It must not
read previous measurements, wall-clock time, mutable UI state, or a current
specification that differs from the item's snapshot. Ambiguous is not Unique
with a warning; no observation may be persisted until explicitly resolved.

## Grammar and units

- Abbreviation: exactly two ASCII digits, e.g. `68`, `01`, `00`. Proposed suffix
  meaning is the last two **integer millimetre** digits, not hundredths or ticks.
- Full override: `=` followed by an unsigned exact decimal in mm, e.g. `=9368.0`.
  It bypasses abbreviation bounds, not precision/resolution checks. `=68` means
  exactly 68 mm; plain `68` is abbreviation mode. Other plain lengths are invalid
  in this grammar rather than guessed as full or abbreviated entries.
- Raw input is retained verbatim. Whitespace, non-ASCII digits, exponents, signs,
  locale decimal commas, and more than six fractional places fail explicitly.
  The terminal adapter must not silently trim or replace the captured text.
- Units other than mm and signed lengths are not supported in Phase 1. An explicit
  instrument resolution must align with any accepted value. Nominal decimal
  precision alone is not evidence of resolution.

## Strategy enabled only by explicit operator approval

For suffix `d` and an independently justified finite inclusive interval `[A,B]`:

```text
candidates = 100*k + d millimetres
k_min = ceil((A-d)/100)
k_max = floor((B-d)/100)
```

Use checked exact arithmetic, including bounds/ceil/floor semantics and candidate
cardinality checks before allocation. Bounds are capture context, **not** LSL/USL
or statistical control limits. One candidate → Unique. Zero → Unresolved, request
full mode or an explicit context change. Multiple → Ambiguous, request explicit
selection/full mode; record an operator selection in provenance. An oversized set
fails with CandidateLimitExceeded and requests full mode, not arbitrary truncation.

This strategy requires operational approval of the interval and suffix meaning.
The CLI asks the operator to approve its suggested bounds or enter custom bounds/
`off`; no empty response enables a default. A provided example alone does not. The
policy name/version/bounds and selection mode are persisted with each observation.
Unknown policy versions fail; changed bounds create distinct recorded context.

## Required examples and edge cases

For Part Number `405363`, nominal 9368.9 mm, LSL 9363.9 mm, USL 9373.9 mm,
an approved `[9300,9399]` mm interval would interpret `68` as **9368 mm**, never
9368.9 mm. In that same interval `80` → 9380 mm is valid but out of spec.

With `[9300,9499]`, `68` has two candidates (9368 and 9468). With `[9370,9399]`,
it has none. Near rollover, `[9950,10049]` and `00` → 10000, while `[9900,10100]`
contains three candidates for `00`. Tolerance membership, nearest nominal, or
the previous value must not silently break ties or suppress out-of-spec values.

Test one/two/three digits, leading zeroes, fractional bounds, interval boundaries,
zero candidates, rollover, multiple candidates, checked overflow, oversized sets,
full overrides outside capture bounds, resolution mismatch, and exact LSL/USL.
The original [project reference](../.agents/skills/dimensional-metrology/references/reconstruction.md)
contains additional synthetic fixtures. Automated tests now cover these candidate
scenarios, resolution, bounds/rollover/ambiguity, and claimed provenance. The engine
uses i128 intermediate arithmetic and refuses sets larger than 1024 candidates.

## Failure and operator decisions

`DomainError` distinguishes invalid decimal/precision/overflow, invalid mode,
resolution mismatch, invalid interval, and inconsistent interpretation.
`ReconstructionError` wraps validation causes and adds PolicyRequired,
UnsupportedPolicy, and CandidateLimitExceeded. Unresolved and Ambiguous are valid
interpretation outcomes requiring operator action, not storage failures.

A full override remains traceable and can be out of spec. Corrections produce
new events; editing the current policy never reinterprets historical records.
Calibration status, measurement-system variation, and uncertainty are unknown
unless supplied; simple dimensional classification does not validate those systems.
