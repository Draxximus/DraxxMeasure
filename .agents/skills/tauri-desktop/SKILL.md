---
name: tauri-desktop
description: Design or implement the future DraxxMeasure Tauri 2 desktop adapter, keyboard-first UI, Rust commands, and safe native exports. Use when desktop work is requested, not to introduce a GUI into core or CLI tasks.
---

# Tauri desktop

## Responsibilities and activation

Own the lightweight Tauri 2 adapter and frontend interaction, not measurement
policy. Follow project `AGENTS.md`; keep the desktop layer optional and local-first.

## Procedure

1. Inspect current core use cases and the installed Tauri version. If the core is
   not ready, specify the required interfaces instead of duplicating logic in UI
   code or scaffolding a frontend without a request.
2. Design a keyboard flow matching the CLI: visible active Part Number/specification
   and bundle, focused entry, Enter to submit, shortcuts for bundle transitions,
   corrections, review, and export. Preserve focus and input after validation errors.
3. Separate ephemeral UI state (focus, draft input, selected view, progress) from
   authoritative Rust/persisted state. Send raw entry to Rust; return interpreted
   values, committed identifiers, classification, and structured errors.
4. Keep reconstruction, rounding, statistics, correction history, and report data
   preparation in Rust. Transport exact dimensional values as documented decimal
   strings or safe fixed-point DTOs; avoid JavaScript number precision loss.
5. Expose narrow Tauri commands over application use cases. Validate IPC input in
   Rust even when the frontend validates it. Coordinate simultaneous submissions,
   block duplicate requests, and acknowledge only committed measurements. On
   restart, obtain current context from storage, not stale browser state.
6. Keep SQLite and filesystem access behind Rust adapters. Use Tauri 2 capabilities
   with least-privilege scopes; do not grant broad shell, filesystem, or remote-web
   permissions merely to export a file. Prefer a native save dialog and a
   user-selected path with explicit overwrite confirmation.
7. Make PNG/CSV exports snapshot-based and safe to cancel. Coordinate temporary
   files and final publication without clobbering another export. Report failures
   without changing inspection records. Keep untrusted labels out of executable
   HTML/commands and do not require network resources for ordinary operation.
8. Keep blocking database/report work off the UI thread through a small justified
   mechanism. Measure startup, memory, and responsiveness before adding a large
   frontend framework, state library, worker pool, or new runtime.

## Validation and acceptance

- The CLI/headless core still works without Tauri dependencies or a webview.
- Exercise keyboard-only capture, focus recovery, ambiguity, duplicate submission,
  close/reopen, denied file access, cancelled save, and failed export.
- Verify least-privilege capability files, IPC validation, exact value round-trips,
  and Windows path/save behavior. Separate verified platforms from planned tests.
- Use [rust-testing](../rust-testing/SKILL.md) for Rust contracts and appropriate
  UI/IPC tests for adapter behavior; do not replace domain tests with UI snapshots.
- Read current [Tauri 2 documentation](https://v2.tauri.app/), particularly
  [capabilities](https://v2.tauri.app/security/capabilities/), when changing commands,
  plugins, or permissions; do not apply Tauri 1 allowlist conventions.
