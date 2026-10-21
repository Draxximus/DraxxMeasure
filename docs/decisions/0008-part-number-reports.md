# ADR 0008 — Report at Part Number closure

- Status: Accepted and implemented
- Date: 2026-10-21

Closing a Part Number context commits navigation out of the part and immediately
exports its consistent snapshot while the inspection session remains active.
Combine all bundles of that Part Number/specification into one 1800×2000 PNG with
a histogram, one measurement plot, and overall descriptive statistics. Original
bundle IDs and correction history remain in the verification CSV. This follows
the requested change from bundle pagination and session-end automatic export.

All source bundles must be closed. Session closure generates no duplicate report
set. `/report part ITEM_ID` retries a part export; `/report` uses the last part
closed in this process. `/report SESSION_ID` remains an explicit closed-session
export, still separating Part Numbers/specification revisions into distinct files.

Part closure remains navigation, so a reselected part can receive more bundles.
Closing it again exports a new cumulative snapshot without overwriting prior
files. Report format 2 manifests identify scope, session state, revision, exact
Part Number and per-item filenames. Files use a safe Part Number label plus
session/item IDs; collisions receive a new export UUID.

The global plot concatenates original per-bundle capture positions in bundle
order, retaining gaps at invalidated observations. All effective points are drawn;
coincident points can overlap, disclosed on the report and in the manifest. CSV
is the exact detail dataset. Statistical algorithms and SQLite writes are unchanged.

Tests cover immediate exports before session closure, multiple parts, more than
four bundles on one PNG, CSV recomputation, correction gaps, empty parts, unsafe
filename characters, retries, preservation of older exports, and repeated closure.
Generated synthetic PNGs are decoded and visually inspected.
