---
name: technical-reporting
description: Design or generate DraxxMeasure English-language PNG inspection reports and verification CSV exports. Use for length-variation charts, bundle comparisons, report layout, provenance, and export consistency, not statistical estimator selection.
---

# Technical reporting

## Responsibilities and activation

Own faithful inspection presentation and verifiable exports. Follow project
`AGENTS.md`. Never fabricate observations or hide missing data.

## Procedure

1. Locate the previously established length-variation report concept before claiming
   fidelity. It is not currently supplied in this project. Ask for it when matching
   its visual design is required; otherwise state that the proposed layout is
   provisional and follow [the report contract](references/report-contract.md).
2. Select a consistent persisted snapshot and identify session, Part Number,
   specification revision/unit, included bundles, effective corrections, cutoff,
   exclusions, and report generation time/timezone. Split unlike parts or specs
   into clearly labeled sections/files; never pool them as one distribution.
3. Obtain calculations from the shared Rust core using
   [quality-statistics](../quality-statistics/SKILL.md). Do not recalculate them in
   frontend JavaScript or a chart adapter. Mark unavailable results and missing
   metadata explicitly, with reasons.
4. Create an English-language PNG with a clear hierarchy: identity/specification,
   summary, measured-length histogram with nominal/LSL/USL, ordered individual
   measurements by bundle, and bundle means plus observed min–max ranges.
   Distinguish observed ranges from confidence intervals or control limits.
5. Adapt layout to actual bundle/sample counts. Keep all observations represented;
   use facets or additional clearly numbered PNG pages instead of unreadable labels
   or undisclosed downsampling. Distinguish a legitimate empty bundle from lost data.
6. Use Rust-based plotting/raster generation, selecting a library only after checking
   required chart types, fonts, PNG backend, licensing, and platform support. Keep
   rendering headless/local, deterministic for a fixed snapshot/configuration,
   and independent of Tauri. Bundle fonts only with an appropriate license.
7. Export a machine-readable CSV verification dataset from the same snapshot with
   identity, order, original input, exact interpreted values, spec context, and
   correction status. Preserve raw strings using proper CSV escaping; document
   spreadsheet formula-injection risk and provide a separate explicitly sanitized
   viewing export if needed, never silently alter the verification dataset.
8. Render/export into temporary output, publish safely after success, confirm
   overwrite, and surface failures without modifying inspection records. An export
   set needs a consistent identifier and detectable partial-completion status.

## Validation and acceptance

- All report text is English; units, Part Number, session ID, specification limits,
  report date, counts, and bundle IDs are legible and unambiguous.
- CSV values reproduce displayed statistics and classification at documented
  precision. Histogram counts and per-bundle counts match included observations.
- Test zero/one/many bundles, unequal counts, constant data, long labels, outliers,
  missing data, corrections, Unicode input, and failures using
  [rust-testing](../rust-testing/SKILL.md).
- Inspect actual raster outputs for clipping, font fallback, contrast, and useful
  dimensions, not just file existence. Label fixtures as synthetic, never live data.
