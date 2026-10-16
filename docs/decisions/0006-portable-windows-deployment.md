# ADR 0006 — Portable Windows x64 builds and explicit writable data locations

- Status: Implemented configuration; native Windows validation gates release publication
- Date: 2026-10-16

## Decision

Build the existing Rust CLI on GitHub's Windows MSVC runner, with formatting,
workspace check, Clippy and tests before release packaging. Enable static CRT for
`x86_64-pc-windows-msvc`; retain bundled SQLite and embedded fonts. Introduce no
application runtime dependency, installer, GUI, Python, or Tauri requirement.

Default storage/report paths are anchored to `current_exe()`'s parent. Explicit
`--data-dir` chooses a data root, and individual database/report flags override it
independently of argument order. Explicit relative flags retain caller-directory
semantics. Fail visibly on unwritable paths; never fall back silently, search/move
databases, or auto-elevate. Older default path users must explicitly select their
existing absolute database path during transition to portable defaults.

Create ZIPs from a four-file allowlist: executable, usage instructions, font license,
and build identity. Do not recursively copy the repository or a user's app directory.
No package can replace a database or a report directory. Updates require stopped
processes, a safe backup, and binary/documentation-only replacement.

Verify the extracted release in a private folder outside the workspace, with a
different working directory and a child PATH limited to Windows system locations.
Check PE imports to prevent runner-installed VC++/SQLite/runtime DLLs masking a
missing portable dependency. Exercise capture/capacity, persistence, PNG/CSV,
configuration and binary-only update preservation before artifact upload.

Only a pushed matching `v<Cargo version>` tag can publish release assets. Builds/PRs
get read-only repository permissions; the separate release job gets contents write.
Use pinned first-party actions and the built-in token. Never overwrite an existing
published release or move a release tag. Include SHA-256 for download checking.

## Consequences and validation

The executable is larger due to static CRT/font/SQLite embedding but needs no
developer tooling on the target PC. Portable folders must be writable local
storage; configured paths support managed data locations and legacy installations.
Default behavior changes only path resolution, not inspection or capacity rules.
No schema change or automatic data migration is part of deployment configuration.

The original authoring checkout had a parent Git root. This public repository is
standalone, with the workflow installed at its root `.github/workflows/`. The same
workflow also supports the original nested workspace layout.
Host Rust tests and isolated release smoke checks provide local evidence; native
Windows behavior and ZIP contents require the configured Windows job to succeed.
No code signing, power-loss certification or automatic backup/restore is implied.
