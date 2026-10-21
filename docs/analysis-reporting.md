# Statistical and reporting contracts

## Implemented versus deferred

Phase 2 implements the descriptive engine, exact histogram membership, verification
CSV serialization, Rust-native Plotters PNG pages, and no-clobber export publication
with a complete manifest last. Domain/snapshot/identity validation remains shared.
SPC/capability analysis and machine-fault diagnosis are not implemented. See
[Phase 2](phase2.md) for concrete behavior and limits; never return synthetic data
as production inspection results.

## Analysis input and population

`StatisticsEngine` consumes one validated `InspectionSnapshot`: one item, one
immutable specification/unit, ordered bundles, original measurements, and all
corrections at a consistent revision. It must call snapshot validation and report
corruption rather than silently omit malformed data. Use effective values only;
invalidated observations count separately, not as zero-valued samples. Original
records and correction history remain available for verification exports.

`InspectionStatistics` records revision, item/specification IDs, unit, algorithm
name/version, total bundle count (including empty bundles), invalidated count,
overall summary, and one summary per bundle. Never pool distinct Part Numbers or
specification revisions. Overall mean must be weighted by sample counts, not an
unweighted average of bundle means.

## Numerical definitions

| Output | Definition |
| --- | --- |
| Measurement count | Effective valid observations, including out-of-spec values |
| Mean | Sum/n using numerically stable accumulation |
| Median | Middle sorted value or exact middle-pair midpoint before descriptive conversion |
| Minimum/maximum/range | Exact dimensional values; range = max-min |
| Sample SD | sqrt(sum((x-mean)^2)/(n-1)); stable Welford/compensated method |
| Mean deviation | Signed mean minus specification nominal, in mm |
| OOS counts | Exact comparisons: below LSL, above USL, and their sum |
| OOS percentage | 100 * OOS/n, unavailable when n=0 |
| Bundle results | Same definitions per independently identified bundle |

`Statistic<T>` represents Available or Unavailable with a reason. Empty data:
counts are zero, location/spread/percentage unavailable. One value: location,
extrema, zero range exist; sample SD unavailable. Constant n>=2 has zero SD.
Do not fabricate zeros for missing values, return NaN/infinity, or round observations
before classification. Exact extrema/range use `Length`; derived descriptive
values use `f64` in mm. Document conversion and calculation tolerances; binary
floats never feed conformance decisions. Six-decimal exact inputs may lose
precision at extreme ranges during descriptive conversion; validate numerical
behavior or reject unsupported analysis ranges explicitly.

Histogram edges are explicit exact increasing lengths; bins are left-closed/
right-open except the final included right edge. All valid values must be covered,
counts must sum to effective n, and OOS/outliers must not be hidden. The engine
returns HistogramDoesNotCoverData rather than dropping observations. Define a
future bin-selection policy separately; constant/empty samples need fallbacks.

Within-bundle scatter, between-bundle mean differences, and total variation are
not interchangeable. Bundles are not automatically rational SPC subgroups. SPC,
Cp/Cpk/Pp/Ppk are deferred until sampling/stability/distribution and measurement-
system assumptions plus sigma estimators are explicitly justified. Engineering
specification limits are never statistical control limits. Patterns do not prove
machine faults. Validate future implementations using the
[independent statistical fixtures](../.agents/skills/quality-statistics/references/numerical-contract.md).

## Report input and chart model

`ReportInput` references the same owned snapshot, statistics, and histogram plus
English-only configuration and export metadata. `validate_identity` checks
snapshot structure and revision/item/specification/unit agreement; it does not
prove numeric results. Exporters must additionally validate counts, edges,
completeness, finite outputs, and display configuration before outputting success.

- Header: exact Part Number, session/item IDs, spec version, nominal/LSL/USL and
  mm unit, capture period, UTC report date, export ID, software/report version.
- Summary: required statistics, bundle/sample counts, invalidation/missing-data
  notes, OOS low/high/total and percentage, and explicit N/A reasons.
- Histogram: exact declared bins/frequencies; labeled nominal/LSL/USL lines.
- Individuals: all effective observations for one Part Number/specification in
  one plot, concatenating bundle capture sequences and retaining retracted gaps.
  Bundle IDs/order remain in the verification CSV; no separate bundle panels.
- Part Number summary: overall mean/median/sample SD, min/max/range, signed mean
  deviation, and low/high/total OOS counts. Empty results show N/A with reasons.
- Footer: snapshot and algorithm identity, factual instrument/calibration metadata
  or unknown, inclusion/correction policy, and separate interpretation notes.

The earlier established length-variation design artifact is absent. This is a
provisional functional contract, not a claimed visual reproduction. Obtain that
artifact before committing to visual fidelity. Format 2 produces one PNG per Part
Number/specification, with all bundles combined. Every effective point is drawn;
coincident points may overlap, as declared on the PNG and manifest. CSV supplies
exact detail at large populations. Axes cover observations and specification limits.

## Export contracts

`PngExporter::render` returns one PNG byte page per Part Number; `CsvExporter::write` emits
UTF-8 verification data to a writer. They are headless Rust services, with no
Tauri/browser dependencies and no frontend recalculation.

Part Number closure triggers `export_part` after committing navigation out of the
part context. Its session can remain Active. All bundles in the exported snapshot
must be closed. `/report part ITEM_ID` retries this export after failure/restart;
`/report` retries the latest part closed in the current process. Closing the session
does not trigger another automatic export. `/report SESSION_ID` explicitly exports
a closed session with one PNG/CSV per Part Number and a shared session manifest.
Report format 2 manifests record scope, cutoff, session state and per-item filenames.

CSV format will use a versioned schema and deterministic row order. Include exact
raw/effective values, units, original IDs/sequence/time, session/item/bundle/spec
context, interpretation provenance, status, and linked audit events. One possible
layout is effective/original rows plus a linked audit dataset; label row kinds
and population filters so audit events are not mistaken for extra samples.
Include a manifest for shared policy/snapshot/algorithm metadata. Recalculation
from effective rows must reproduce PNG summaries and bin counts.

CSV quoting handles delimiters, newlines, and quotes but does not prevent formula
injection. Preserve canonical raw data; document text-only spreadsheet import and
optionally produce a separately labeled transformed viewing copy. No silent prefix
insertion/normalization in the verification dataset. Part Numbers stay text even
when a spreadsheet guesses a numeric type.

The caller writes temporary files, checks complete rendering/flush, confirms
overwrites, and publishes the export set with a manifest **last**. Multiple files
are not assumed atomically replaceable across platforms; the manifest identifies
complete sets and failed/partial publication is reported. No export modifies
inspection data. See the [report reference](../.agents/skills/technical-reporting/references/report-contract.md)
for visual/CSV acceptance. Test actual raster readability, not file existence alone.
