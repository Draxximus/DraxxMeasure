# ADR 0007 — Ratatui terminal presentation

- Status: Accepted and implemented
- Date: 2026-10-21

## Decision

Add the requested persistent terminal screen using Ratatui 0.30.2 with its
Crossterm backend and Crossterm 0.29 events. Both dependencies belong only to
`draxx-cli`; core, application, storage, statistics and reporting remain headless.
The versions follow [Ratatui's installation guidance](https://ratatui.rs/installation/).
Disable Ratatui's default features and enable the Crossterm backend explicitly.

Retain the existing controller and line interface. An observation hook supplies
committed state and explicit rejection feedback. One standard-library worker
owns the controller and SQLite connection; channels adapt complete lines and
output to the UI thread. This keeps drawing responsive during writes/exports
without an async runtime or a second implementation of command/setup semantics.

Automatically select the screen for interactive stdin/stdout, with `--line` and
`--tui` overrides and a line fallback for `TERM=dumb`. Redirected automation and
portable package checks retain plain input. This supersedes ADR 0005's decision
to use only a line presentation, following the explicit request for Ratatui.

## Alternatives and tradeoffs

A separate TUI command state machine would duplicate setup, correction and retry
behavior. Reusing the synchronous controller adds a small channel bridge and one
worker while preserving the tested transaction boundary. Line mode remains the
lowest-dependency interaction path, but the executable now includes terminal UI
dependencies. No additional CLI parser or GUI dependency is introduced.

Screen views refresh at controller input boundaries and on `/status`; the screen
does not poll SQLite on each frame. Pasted lines advance only on explicit Enter,
so validation failures preserve editable text and queued entries. Pending input
retains the existing in-memory lifetime; durable uncertain-write recovery is a
separate outstanding issue from the prior review.

## Validation

Exercise the real controller/SQLite path through the screen channels, plus Ratatui
TestBackend rendering, input editing, error retention and retries. Run workspace
formatting, tests and Clippy. Check a real Linux pseudo-terminal for raw-mode
events, resize and restoration. Native Windows raw-mode coverage requires a
Windows console; redirected scripts continue using the existing line path.
