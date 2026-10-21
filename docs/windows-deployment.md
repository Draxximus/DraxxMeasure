# Portable Windows deployment

## Download and launch (no development tools)

Supported package: **Windows 10/11 x64**, target `x86_64-pc-windows-msvc`.
The ZIP contains a release-mode `draxxmeasure.exe`, `WINDOWS-README.txt`,
`FONT-LICENSE.txt`, and `BUILD-INFO.txt`. SQLite, the C runtime and font data are
compiled/embedded into the application. Only OS-provided DLLs are permitted by
the packaging checks. No Rust, Python, Tauri, SQLite installation, VC++ runtime
installer, webview, or system font setup is required on the inspection computer.

1. Open the repository's **Releases**, select a version, and download
   `DraxxMeasure-v0.2.0-windows-x64.zip` under **Assets** (substitute your version).
   Do not download GitHub's automatic **Source code** ZIP as an application.
2. Download its `.zip.sha256` companion if you want to verify the SHA-256:

   ```powershell
   Get-FileHash .\DraxxMeasure-v0.2.0-windows-x64.zip -Algorithm SHA256
   Get-Content .\DraxxMeasure-v0.2.0-windows-x64.zip.sha256
   ```

   Compare the hex digests. The checksum detects download changes; it is not a
   code-signing certificate. These builds are unsigned. Follow organizational
   SmartScreen/security policy; do not disable Windows protections.
3. Use Explorer **Extract All** into a writable **local** folder, for example
   `C:\Users\YourName\DraxxMeasure`. Do not launch from within the ZIP. Avoid
   `Program Files`, read-only folders, network shares and live cloud-sync folders.
4. Double-click `draxxmeasure.exe`, or run it in PowerShell:

   ```powershell
   .\draxxmeasure.exe
   ```

No administrator access is needed. `/help` lists the existing measurement workflow.
Interactive consoles open the Ratatui screen with F1 keyboard help. Use `--line`
for plain input, or `--tui` to require the screen. Redirected scripts keep line mode.
The 42-active-measurement rule, reconstruction, correction and closure semantics
are unchanged. If an error closes a double-clicked console, launch from PowerShell
to read the diagnostic and choose a writable path.

## Portable and configurable data paths

```text
DraxxMeasure/
├── draxxmeasure.exe
├── WINDOWS-README.txt
├── FONT-LICENSE.txt
├── BUILD-INFO.txt
├── data/                         created on first inspection launch
│   └── draxxmeasure.sqlite3       plus any SQLite WAL/SHM/journal sidecars
└── reports/                      created on first report export
    └── session-001-part-405363-item-001.png  also a CSV and manifest per part
```

Defaults are resolved with the executable's actual location, not the source
workspace, shell directory, shortcut's **Start in** directory, or a development
machine's absolute paths. `--help` and `--version` do not create data directories.

For a separately managed writable local data root:

```powershell
.\draxxmeasure.exe --data-dir "D:\Inspection Data"
```

This selects `D:\Inspection Data\draxxmeasure.sqlite3` and
`D:\Inspection Data\reports`. Individual overrides win regardless of argument order:

```powershell
.\draxxmeasure.exe --database "D:\Inspection Data\existing.sqlite3" --reports-dir "D:\Inspection Reports"
```

Explicit relative paths retain normal command-line behavior (relative to the
working directory). Use absolute paths in managed shortcuts. There is no automatic
fallback to a different database on a write/permission failure: the application
reports the error, preserves existing data, and lets the operator choose paths.

**Existing development installations:** older builds used working-directory-relative
defaults. Before an upgrade, identify the actual old database and pass its absolute
`--database` path and desired `--reports-dir`. Check the printed Database path and
`/sessions`. Do not assume an empty new location means the old data was deleted.
No automatic database searching, moving, replacement or migration-by-copy occurs.

## Update without overwriting inspection records

1. Exit every instance using the database. Resolve uncertain submissions first.
2. With all processes stopped, back up the **whole data directory**, including any
   remaining `-wal`, `-shm` or journal files, and retain reports. Do not copy only
   the main file from an active WAL database, or delete sidecars to fix startup.
   Automated online backup/restore is not implemented.
3. Download and extract the new ZIP into a **separate empty directory**.
4. Replace **only** these files in the existing application folder:
   `draxxmeasure.exe`, `WINDOWS-README.txt`, `FONT-LICENSE.txt`, `BUILD-INFO.txt`.
   Never delete/replace `data/` or `reports/`, and never replace an existing database
   with an empty distribution database. The package contains no such database.
5. Relaunch with the same path flags/shortcut configuration, verify Database path,
   then `/sessions` and `/resume ID` as appropriate. A report retry uses `/report ID`.

Replacing the executable does not overwrite records or reports. A future release
that changes the database schema needs its own reviewed migration/backup plan;
the current application rejects newer/unknown schemas instead of resetting them.
Do not downgrade an upgraded database unless that release documents compatibility.

## Workflow location and prerequisites

This public repository is standalone: `Cargo.toml` and
[`.github/workflows/draxxmeasure-windows.yml`](../.github/workflows/draxxmeasure-windows.yml)
are both at its Git root. The original authoring checkout had a parent Git root;
the workflow also supports that layout by selecting a nested `DraxxMeasure/`
workspace when present. GitHub discovers workflows only at the repository root's
`.github/workflows/`, not inside a nested source folder.

Enable Actions in the GitHub repository. The build job uses read-only repository
permissions; only the tag-release job gets `contents: write`. The built-in
`GITHUB_TOKEN` is sufficient. Organization/repository policy must allow creating
releases; no additional PAT, deployment server or target-machine secret is needed.

## CI/package verification

For PRs, pushes to `main`/`master`, manual dispatches, and `v*` tags, the Windows
Server 2022 runner installs the pinned Rust 1.97.1 MSVC build toolchain and runs:

```text
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked --target x86_64-pc-windows-msvc -p draxx-cli --bin draxxmeasure
```

`.cargo/config.toml` enables static CRT for this target, including the bundled
SQLite build. Packaging copies an explicit **four-file allowlist**, not the workspace.
Inspection databases, rows, reports, runtime installers and source files cannot
accidentally enter the ZIP. The versioned ZIP gets a SHA-256 companion.

Before upload, `scripts/windows/test-portable.ps1` checks the actual archive,
extracts it to a temporary folder with spaces/Unicode, inspects PE imports using
MSVC `dumpbin`, and rejects non-OS DLL dependencies. It launches with an unrelated
working directory and a child PATH containing only Windows system directories,
without Rust/Cargo/Python paths. It verifies version/help, 42 accepted measurements,
#43 rejection, SQLite/CSV population, PNG output, no working-directory data writes,
preservation during a binary-only update, re-reporting old data, and custom paths.
All smoke data is generated in private temporary folders, never included in packages.

Successful runs upload **draxxmeasure-windows-x64-portable** in the Actions run's
Artifacts section (30-day retention; download may require GitHub sign-in). That
outer Actions artifact contains the versioned application ZIP and checksum; extract
the artifact first, then the inner application ZIP. Releases expose those files
directly as persistent download assets, subject to repository visibility/access.

These native Windows checks execute on GitHub. A green workflow is required
evidence for the Windows package; release publication is gated on that verification.
Local Rust tests
also exercise copied-executable isolation and data preservation on the host OS.

## Publish a release

1. Review changes and ensure no production data is staged. In the standalone
   public repository, keep the workflow at the root `.github/workflows/`.
   Commit/push the source and documentation with a configured `origin` remote.
2. Set `[workspace.package].version` in `Cargo.toml` to the release version.
   Refresh/commit `Cargo.lock` with `cargo check --workspace` and run the tests.
   The current release version is `0.2.0`.
3. From the Git repository root, tag the committed source and push the tag:

   ```text
   git tag -a v0.2.0 -m "DraxxMeasure 0.2.0"
   git push origin v0.2.0
   ```

4. Open **Actions → DraxxMeasure Windows portable**. Only after all checks and
   extracted-release verification succeed does the release job create
   **DraxxMeasure v0.2.0**, with the ZIP and SHA-256 attached and generated notes.
   The tag must match the CLI Cargo version exactly; mismatches fail before release.
   Prerelease versions such as `v0.2.0-rc.1` are marked prerelease.
5. Download that release ZIP on the inspection PC and follow the first section.

For later versions, update the workspace version/lockfile and use a **new immutable
tag**, for example `v0.2.1`. Do not force-move tags or replace published assets.
Re-running a job for an already published release deliberately refuses to overwrite
it. No release is created for PRs, branch builds or manual checks without a tag push.
