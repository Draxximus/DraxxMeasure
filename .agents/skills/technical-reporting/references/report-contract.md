# Length-variation report and verification contract

Use when the historical design artifact is unavailable or when defining export
acceptance. This is a provisional functional contract, not the missing design.

## Report content

- Header: Part Number (as text), session identification, specification revision,
  nominal, LSL, USL, unit, capture period, generation date/time with timezone,
  snapshot/export ID, and report/calculation version.
- Summary: effective sample count, bundle count, mean, median, min, max, range,
  sample SD, signed mean deviation from nominal, low/high/total OOS counts and
  percentage when defined. Show `N/A` plus reason for undefined metrics.
- Histogram: measured-length axis in the stated unit and frequency axis, declared
  bin policy, and independently styled nominal/LSL/USL lines with English labels.
  If a limit is outside the plotted range, extend the axis or annotate it explicitly;
  do not silently omit it. Show out-of-spec observations rather than trimming them.
- Individual measurements: points in original per-bundle sequence, clear bundle
  boundaries/IDs, common comparable axes where practical, nominal and limits.
  Preserve observable gaps for retractions/missing records with an explanation;
  do not interpolate missing measurements or imply continuous sampled time.
- Bundle summary: bundle mean with observed min–max whiskers (not a confidence
  interval), sample count, and range. For a single value, min=max; an empty bundle
  has no observed mean/range and must not be drawn at zero.
- Footer: inclusion/exclusion/correction policy and missing metadata, software and
  algorithm versions, and separated observation/calculation/interpretation notes.
  Instrument/calibration information is factual or explicitly unknown.

Do not label a trend as a confirmed machine failure. Capability/control-chart
panels are optional and require justified methods; specification lines alone are
not a control chart. Use shapes/labels as well as color for status distinctions.

## Scaling the layout

Choose readable raster dimensions and font sizes for the intended viewing/printing
size; document the target. Many bundles may require small multiples or multiple
numbered PNG files with one shared snapshot ID. Every observation must appear
somewhere in the output set or an explicitly declared detail export, with any
aggregation disclosed. Do not conceal missing points behind overplotting without
counts/annotation or a detail view. Check that the summary remains population-wide.

## CSV verification dataset

Define a versioned, UTF-8, properly quoted dialect with decimal point, explicit
units, timestamp format/timezone, null convention, deterministic row order, and
exact decimal precision. At minimum expose:

- Export/snapshot ID, session ID, session-part ID, Part Number, specification ID
  and revision, nominal, LSL, USL, unit and exact value scale.
- Bundle ID/label/order, measurement ID and original per-bundle sequence,
  capture timestamp, raw input, input mode, interpreted value, interpretation
  policy/version/context or an explicit reference to accompanying metadata.
- Effective/corrected/retracted status, linked original/replacement/event IDs
  as applicable, and the classification decision rule used.

Choose and state the row model: one row per original observation with effective
status, or an effective dataset plus a separate linked history dataset. Never mix
audit events into the statistical population without an explicit filter. The
effective rows must reproduce the PNG population exactly. Include a manifest or
metadata CSV when a single table cannot express snapshot/policy context clearly.

Quote commas, quotes, newlines, and raw operator strings correctly. Values starting
with `=`, `+`, `-`, or `@` can trigger spreadsheet formulas; quoting alone is not
a defense. Preserve the canonical verification data, document safe text import,
and label any sanitized spreadsheet-view copy as transformed.

## Validation evidence

Recompute summary and bin counts independently from exported effective rows;
compare exact counts/specification decisions and documented float tolerances.
Verify PNG decoding, dimensions, every expected panel/legend, no clipped long
labels, readable crowded layouts, and no output success after partial failure.
Pixel snapshots need pinned fonts/backend and a reasoned tolerance; complement
them with structural/data tests rather than relying on pixels alone.
