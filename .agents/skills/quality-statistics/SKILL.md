---
name: quality-statistics
description: Calculate or review DraxxMeasure descriptive statistics, bundle variation, distributions, SPC, and capability assumptions. Use for numerical methods and interpretation, not chart styling or diagnosis of machine failures.
---

# Quality statistics

## Responsibilities and activation

Own reproducible numerical definitions and defensible interpretation. Follow
project `AGENTS.md`; do not infer process health solely from collected dimensions.

## Procedure

1. Define the analysis snapshot: session, Part Number/specification revision, units,
   bundles, order, correction cutoff, exclusions, and missing/invalid data policy.
   Do not pool unlike parts/specifications or count retracted/superseded values as
   current measurements. Preserve an auditable path to those historical records.
2. Read [numerical definitions and fixtures](references/numerical-contract.md)
   before changing calculations. Return sample count, mean, median, min, max,
   range, sample standard deviation, signed deviation from nominal, and separate
   low/high/total out-of-spec counts. Mark undefined statistics unavailable with
   reasons, never as fabricated zeroes.
3. Retain exact values for specification decisions and use stable methods for
   accumulation/variance. Document numeric precision, overflow behavior, floating
   comparison tolerances for tests, and display rounding separately.
4. Compute per-bundle results first, retaining identity and chronological order.
   Explain weighting for pooled results. Distinguish within-bundle scatter,
   variation of bundle means, and total variation; they are not interchangeable
   estimates of the same sigma. Account for unequal bundle sizes explicitly.
5. Define histogram edges, edge inclusion, units, and bin-selection policy. Include
   all valid observations, including outliers and out-of-spec data. Keep missing
   data/exclusions visible. Constant or tiny datasets need explicit fallbacks.
6. Use SPC only with a justified chart, sampling order, rational subgroup design,
   subgroup sizes, baseline, and rule set. A bundle is not automatically a rational
   subgroup. Distinguish estimated control limits from engineering specification
   limits; do not use nominal ± tolerance as control limits.
7. Before Cp/Cpk/Pp/Ppk, evaluate process stability, distribution, independence,
   representativeness, sample adequacy, and measurement-system suitability. Define
   the sigma estimator and within/overall distinction. If unjustified, withhold
   capability claims or label a requested exploratory calculation with caveats.
8. Report patterns as observations or hypotheses. A trend, bundle difference, or
   association does not confirm a machine fault or its cause.

## Validation and acceptance

- Compare against the reference fixtures and an independently calculated dataset;
  test empty/singleton/constant data, unequal bundles, large offsets, and boundaries.
- Require matching count, inclusion set, units, and algorithm version across CLI,
  CSV, and PNG. Record snapshot/analysis metadata so results can be reproduced.
- Demonstrate distinctions between sample standard deviation, within-bundle
  estimates, overall variation, specification limits, and control limits.
- Use [rust-testing](../rust-testing/SKILL.md) for regression/property checks and
  [technical-reporting](../technical-reporting/SKILL.md) for presentation.
- For SPC/capability decisions consult the
  [NIST/SEMATECH Engineering Statistics Handbook](https://www.itl.nist.gov/div898/handbook/).
  Record methods actually used rather than implying certified compliance.
