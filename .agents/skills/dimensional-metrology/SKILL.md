---
name: dimensional-metrology
description: Define or review DraxxMeasure dimensions, units, tolerance decisions, instrument precision, and abbreviated measurement reconstruction. Use for two-digit input interpretation, full overrides, ambiguity, or measurement traceability.
---

# Dimensional metrology

## Responsibilities and activation

Own measurement meaning and interpretation, not terminal interaction or SQL.
Follow project `AGENTS.md`. Preserve observed values separately from assumptions.

## Procedure

1. Establish the unit, nominal, specification revision, LSL/USL, and boundary
   decision rule. Derive symmetric limits as nominal ± tolerance only when that
   is the actual specification; allow asymmetric limits. Require LSL < USL and
   validate nominal placement under the agreed specification policy.
2. Specify exact storage scale, instrument resolution, accepted decimal syntax,
   conversion rules, and rounding policy. Prefer checked fixed-point integers or
   a justified exact decimal representation for dimensional comparisons. Never
   classify using display-rounded values or silently round excessive precision.
3. Preserve the exact operator input, interpreted value/unit, input mode,
   reconstruction policy/version and context, specification revision, and relevant
   instrument metadata. Mark calibration status/uncertainty as unknown when absent;
   do not infer calibration, resolution, or extra digits from nominal precision.
4. For abbreviated entry, read
   [the reconstruction contract](references/reconstruction.md). Document and obtain
   agreement on its suffix meaning, candidate bounds, uniqueness rules, and failure
   handling before enabling it. The provided example alone is not an algorithm.
5. Distinguish malformed, unambiguous, ambiguous, and unresolved entries. Ambiguity
   requires explicit operator selection or a full-length override. Do not resolve
   by the previous measurement, nearest nominal, or tolerance membership unless a
   separately approved policy explicitly requires and records that behavior.
6. Keep syntactic validity separate from conformance. Accept representable valid
   out-of-spec measurements, mark them, and retain traceability. A full-length
   override bypasses abbreviation, not unit/precision validation or audit history.
7. Address instrument resolution, calibration evidence, measurement-system variation
   and any uncertainty/guard-band rule when relevant to decisions. Label a simple
   LSL/USL comparison as such; it is not evidence of a validated measurement system.

## Validation and acceptance

- Include the Part Number `405363`, nominal `9368.9 mm`, tolerance `±5 mm`, input
  `68` → `9368 mm` as a conditional approved-policy fixture, never a hidden default.
- Test rollover, multiple/no candidates, leading zeros, decimals, unit mismatch,
  overflow, full override, exact LSL/USL, and one instrument increment outside.
- Reproduce interpretation from stored raw input and recorded policy/context.
- State missing instrument information and separate measured results from inferred
  context. Use [rust-testing](../rust-testing/SKILL.md) for automated evidence and
  [inspection-data-model](../inspection-data-model/SKILL.md) for persisted provenance.
- For terminology consult [JCGM metrology publications](https://www.bipm.org/en/committees/jc/jcgm/publications).
  Do not claim standards compliance without the applicable requirements/evidence.
