# Synthetic example report

**SYNTHETIC EXAMPLE — NOT PRODUCTION DATA.** Every observation in this directory
comes from the explicit fixture lines in `session-input.txt`, not an actual
inspection or the application's own fabricated measurements.

The generated pages demonstrate two Part Numbers, multiple independent bundles,
leading-zero abbreviated input, full-length overrides, exact specification
boundaries, out-of-spec observations, a traceable correction, and an invalidation
that retains original data and sequence gaps.

- [First Part Number PNG](session-001.png)
- [Second Part Number PNG](session-001-page-002.png)
- [Complete export manifest](session-001.manifest.json)
- [First Part Number verification CSV](session-001-item-001.csv)
- [Second Part Number verification CSV](session-001-item-002.csv)
- [Replayable input](session-input.txt)

## Replay

From the project root, use a **fresh database path**, not production inspection
storage. Do not replay over a database containing unrelated inspections.

Linux/bash:

```bash
cargo run -p draxx-cli -- --database data/synthetic-example.sqlite3 --reports-dir exports/synthetic-example --report-note "SYNTHETIC EXAMPLE - NOT PRODUCTION DATA" < docs/examples/session-input.txt
```

PowerShell:

```powershell
Get-Content docs/examples/session-input.txt | cargo run -p draxx-cli -- --database data/synthetic-example.sqlite3 --reports-dir exports/synthetic-example --report-note "SYNTHETIC EXAMPLE - NOT PRODUCTION DATA"
```

Images are generated entirely in Rust. Python is not required at runtime. A fresh
replay has the same measured/statistical population, but generation/capture dates,
request/export UUIDs and image bytes differ. Existing outputs are never overwritten.

## Independent verification

Both included PNGs were decoded and visually inspected at 1800x2000 pixels.
The CSVs were parsed independently, filtering `row_kind=effective` and nonempty
`value_mm`; mean, sample SD, OOS counts/percentage and every histogram bin were
recomputed and compared with the manifest. Extrema were checked against the
expected population and visible PNG summaries. Original/correction audit
rows were excluded from the statistical population, not deleted.

Expected effective populations:

| Part Number | Bundles | Effective lengths (mm) | OOS |
| --- | --- | --- | --- |
| 405363 | 2 | 9368, 9370.9, 9367, 9381; 9363.9, 9373, 9373.9 | 1 of 7 |
| 413348 | 1 | 10609, 10612, 10603.9, 10614 | 1 of 4 |

For 405363, original `71` is retained alongside its explicit `=9370.9` correction;
original `80` is retained as invalidated. The later `=9381` has a new sequence.
For 413348, raw `09` remains exactly `09`, interpreted as 10609 mm under the
approved integer-mm interval. LSL/USL equality is conforming; no guard band is
implied. The footer/manifest explicitly label the dataset synthetic.

Canonical CSV preserves raw `=` inputs. Import all columns as **text** in spreadsheet
software to avoid formula interpretation. The production test suite performs the
same type of CSV/statistical round-trip verification without a Python dependency.
