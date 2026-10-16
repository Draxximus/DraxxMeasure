# Numerical contract and independent fixtures

## Definitions

For the effective valid observation set `x[1..n]` in one unit/specification:

- Mean: `sum(x)/n`; median: middle sorted value, or midpoint of the two middle
  values. Sorting for analysis must not change persisted capture order.
- Minimum/maximum and range `max-min` require at least one observation.
- Sample variance: `sum((x-mean)^2)/(n-1)` for `n >= 2`; sample SD is its square root.
  Use Welford, a compensated two-pass algorithm, or a comparably stable method,
  not subtraction of large nearly equal raw sums of squares.
- Signed deviation for each observation: `x-nominal` in the same unit. Distinguish
  signed from absolute or percentage deviation when additional outputs are requested.
- For the simple inclusive-limit policy, low means `x < LSL`, high means `x > USL`;
  exact LSL/USL values conform. An uncertainty/guard-band rule is a separate,
  explicitly named decision rule, not a silent change to these comparisons.
- `n=0`: count/OOS counts are zero, descriptive location/spread and percentages
  are unavailable. `n=1`: mean/median/min/max exist, range is zero, sample SD is
  unavailable. Constant `n>=2`: SD is zero; capability division by zero is unavailable.
- Reject NaN/infinite/malformed values at the domain boundary. Never coerce them
  to zero or quietly omit them during analysis. Report corrupted/missing records.

## Bundle decomposition

Keep each bundle's `n_j`, mean, range, and sample SD. Overall mean is
`sum(n_j*mean_j)/sum(n_j)`, not an unweighted mean of bundle means.

For nonempty bundles, `SS_within = sum_j sum_i (x_ij-mean_j)^2` and
`SS_between = sum_j n_j*(mean_j-grand_mean)^2`;
`SS_total = SS_within + SS_between`. These descriptive quantities do not by
themselves prove random-effects structure or rational subgroup validity.

If a pooled within SD is justified, document
`sqrt(SS_within / sum_j(n_j-1))`, requiring positive denominator. A range-based
sigma estimator requires its own subgroup-size constants and assumptions.
Do not label SD of bundle means as a pure between-process sigma without a model
that accounts for sampling variation and unequal subgroup sizes.

## Capability definitions, only after checking assumptions

For two-sided limits and positive justified sigma:
`Cp=(USL-LSL)/(6*sigma_within)`;
`Cpk=min(USL-mean, mean-LSL)/(3*sigma_within)`.
`Pp` and `Ppk` use overall sample SD instead. Retain negative Cpk/Ppk when the
mean lies outside the limits. Do not clip it or substitute an undocumented
estimator. One-sided specifications require an explicitly appropriate metric.
Do not invent a universal minimum sample count or assert normality from a small
sample. Non-normal capability requires a separately justified method.

## Fixtures (synthetic tests only)

1. Values `[1,2,3,4,5]` mm, nominal `3`, inclusive limits `[2,4]`:
   n=5, mean=median=3, min=1, max=5, range=4,
   sample variance=2.5, sample SD=1.5811388300841898,
   deviations `[-2,-1,0,1,2]`, low=1, high=1, total OOS=2.
2. Bundle A `[1,2]`, B `[3,4,5]`: means 1.5 and 4, ranges 1 and 2,
   grand mean=3, SS_within=2.5, SS_between=7.5, SS_total=10.
   Pooled within SD (if appropriate)=sqrt(2.5/3)=0.9128709291752769.
   The unweighted mean of bundle means is 2.75 and is not the grand mean.
3. Add `1_000_000_000` to fixture 1: sample variance remains 2.5 and mean becomes
   1_000_000_003. This catches unstable sum-of-squares implementations.
4. Values exactly `[LSL, USL]` are both in specification under the simple policy;
   values one representable instrument increment beyond each boundary are out.

Calculate expected values independently (exact fractions or a separate trusted
implementation), not by calling the production function under test. For floats,
choose documented absolute/relative error bounds appropriate to scale; exact
counts and dimensional boundary decisions must not use epsilon-based fuzzing.

## Histogram reproducibility

Persist or emit the actual bin edges and counts. A useful convention is
left-closed/right-open bins with a closed final right edge; specify it explicitly.
Ensure every valid observation belongs to exactly one bin and counts sum to n.
Do not trim the histogram to LSL/USL or hide extreme values to improve its appearance.
