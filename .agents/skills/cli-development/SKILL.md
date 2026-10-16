---
name: cli-development
description: Design or implement DraxxMeasure keyboard-first CLI measurement entry, commands, undo, interruption, and recovery. Use for terminal workflows and Windows terminal behavior, not desktop UI or reconstruction policy itself.
---

# CLI development

## Responsibilities and activation

Own terminal interaction and operator feedback. Follow project `AGENTS.md` for
data integrity and traceability. Keep domain rules in the Rust core.

## Procedure

1. Inspect the current CLI and map the smallest operator path: resume/start →
   select/create Part Number and specification → start bundle → capture →
   transition bundle → review → close → export. Show the active session,
   Part Number, specification/unit, bundle, and accepted count without extra input.
2. Define a state/command table, including which commands are legal during entry,
   errors, undo, correction, bundle changes, closing, and recovery. Separate
   measurement syntax from commands and full-length overrides; document the
   chosen grammar rather than assuming all short numbers are abbreviations.
3. Make a valid unambiguous entry plus Enter the normal capture path. Briefly show
   raw input → interpreted value, status, and committed sequence. Request an
   explicit choice only for ambiguity or a destructive transition, not every row.
   Out-of-spec observations remain valid observations, not parsing errors.
4. Route validation and reconstruction through
   [dimensional-metrology](../dimensional-metrology/SKILL.md). On malformed input,
   retain the session, active bundle, and editable text; explain how to correct it.
   Do not substitute a guessed value, clear the current context, or drop paste lines.
5. Commit each accepted measurement through the application use case before
   showing success or advancing the accepted count. On storage failure, retain
   pending input and clearly distinguish it from committed data. Prevent double
   submission and make uncertain retries safe through the persistence contract.
6. Define undo as a traceable correction/retraction, with an explicit target
   measurement ID and bundle. Show the target before changing historical data;
   never silently remove the latest row across a different bundle or Part Number.
7. Handle Ctrl+C, EOF, cancellation, terminal resize, and unexpected shutdown.
   Specify pending-input treatment, restore terminal modes on every exit path,
   and resume from committed state without assuming a graceful shutdown occurred.
8. Evaluate dependencies only against concrete needs: Clap for structured startup
   arguments/subcommands; plain line input for a minimal capture loop; Crossterm
   for cross-platform key/raw-mode handling; Ratatui for a justified persistent
   dashboard. Do not add all three by default or introduce Unix-only assumptions.

## Validation and acceptance

- Document keystrokes for recording several measurements and switching bundles.
- Exercise invalid text, ambiguity, full override, empty input, rapid Enter,
  multiline paste, correction, write failure, and interruption without losing data.
- Check Windows Terminal/PowerShell and redirected/noninteractive input where
  supported; state actual test coverage versus untested assumptions. Do not rely
  on POSIX signals or ANSI support without a fallback or declared requirement.
- Verify that restart preserves identity, order, active context, and accepted count
  with [inspection-data-model](../inspection-data-model/SKILL.md) and
  [rust-testing](../rust-testing/SKILL.md).
- Consult the selected library's current docs:
  [Clap](https://docs.rs/clap/), [Crossterm](https://docs.rs/crossterm/),
  [Ratatui](https://ratatui.rs/). Record versions when choosing APIs.
