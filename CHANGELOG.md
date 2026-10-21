# Changelog

## 0.2.0

- Add a Ratatui terminal screen with keyboard shortcuts and a plain line mode for redirected input.
- Show all 42 tube positions in the staggered 8–9–8–9–8 arrangement; update only after committed writes, including undo and resume. Full bundles remain open until manually closed.
- Keep measurement history hidden by default in the screen; press F10 to show it.
- Combine all bundles into one PNG report and verification CSV per Part Number and specification. Generate reports when the Part Number context closes, without waiting for session closure.
- Retry exports with `/report part ITEM_ID`; retain explicit closed-session exports and preserve previous exports when a Part Number is measured again.
- Include report scope, capture times, and source bundle provenance in report format 2 metadata.

## 0.1.0

- Initial dimensional inspection capture, SQLite persistence, statistics, PNG/CSV reporting, and portable Windows release.
