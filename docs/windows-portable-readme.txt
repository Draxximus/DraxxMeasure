DraxxMeasure - Portable Windows x64
==================================

Requires 64-bit Windows 10/11. No Rust, Python, Tauri, separate SQLite,
VC++ redistributable installer, webview or system font installation is needed.
The executable uses OS-provided Windows DLLs only. Keep FONT-LICENSE.txt
with copies of the application. BUILD-INFO.txt identifies the source/version.

DOWNLOAD AND START
1. Download DraxxMeasure-vVERSION-windows-x64.zip from GitHub Releases Assets
   (not the automatically generated Source code ZIP).
2. Optionally compare Get-FileHash -Algorithm SHA256 on the ZIP with the
   downloaded .zip.sha256 file. Follow your organization's security policy
   for unsigned applications; do not disable Windows security protections.
3. Extract ALL files to a writable LOCAL folder, such as
   C:\Users\YourName\DraxxMeasure. Do not run from inside the ZIP or Program Files.
4. Double-click draxxmeasure.exe, or open PowerShell in that folder and run:
      .\draxxmeasure.exe
   No administrator access or development workspace is required.

DATA LOCATIONS
By default the application creates these beside the executable, regardless of
the terminal/shortcut working directory:
   data\draxxmeasure.sqlite3    inspection data and correction history
   reports\                    generated PNG/CSV reports and manifests
Check the printed Database path before starting an inspection.

For a separate writable local data location:
   .\draxxmeasure.exe --data-dir "D:\Inspection Data"
This uses D:\Inspection Data\draxxmeasure.sqlite3 and
D:\Inspection Data\reports. Prefer absolute paths for shortcuts.

Individual overrides, which take precedence regardless of argument order:
   .\draxxmeasure.exe --database "D:\Inspection Data\existing.sqlite3" --reports-dir "D:\Inspection Reports"
Explicit relative paths are relative to the shell's working directory.
The application never silently falls back to a different database if a directory
is unwritable. Select a writable folder or supply explicit paths.

WORKFLOW
   /session        start session
   /partnumber     create/select Part Number and specification
   /bundle         start bundle
   68              record abbreviated entry + Enter
   =9368.0         record an explicit full-length value + Enter
   /close          close only the current bundle / Part Number / session
   /help           show all commands
Setup asks for explicit instrument resolution and abbreviation interval approval.
There are at most 42 ACTIVE measurements per bundle. A full bundle stays open
until /close. No bundle may close empty. Undo/corrections retain audit history.

RECOVERY AND REPORTS
/exit or an interruption leaves committed sessions recoverable. Restart using
the SAME database path, then /sessions and /resume ID. Unacknowledged draft input
is not guaranteed durable; inspect recovered state before re-entering it.
Closing a Part Number generates one PNG/CSV report combining all its bundles,
even while the session stays open. If export fails, inspection data is preserved;
use /report to retry the last closed part, or /report part ITEM_ID after restart.
You can restart with a writable --reports-dir. Session closure does not generate
another automatic report; /report SESSION_ID explicitly exports a closed session.
Import canonical CSV as TEXT because original '=' inputs may look like formulas.

SAFE UPDATE (NEVER REPLACE YOUR DATA FOLDER)
1. Stop entry and exit ALL DraxxMeasure processes using this database.
2. Back up the entire local data directory while no process has the database open,
   INCLUDING any SQLite -wal, -shm and journal files that remain. Retain reports.
   Never copy only the main database from a running WAL database or delete sidecars.
3. Extract the new ZIP to a SEPARATE empty folder.
4. Copy ONLY draxxmeasure.exe, WINDOWS-README.txt, FONT-LICENSE.txt and BUILD-INFO.txt
   into the existing application folder. Do not delete or replace data/ or reports/.
   Release ZIPs contain no database, report or data directory.
5. Launch with the same data configuration, verify the printed Database path,
   and use /sessions before continuing. Unsupported newer schemas are refused,
   not reset. Do not downgrade a database with an incompatible older executable.

Older development builds used working-directory-relative defaults. For an
existing inspection, explicitly select its original absolute --database path;
this version does not search for, move or delete old databases automatically.

Use local storage, not network shares or live cloud-synchronized database folders.
Never unplug removable media while the application is running. Windows execution
and dependency checks are performed by the release workflow; these are not a
power-loss guarantee or industrial/regulatory certification.
