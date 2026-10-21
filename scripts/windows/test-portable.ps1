[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$ArchivePath,
    [Parameter(Mandatory)][string]$ExpectedVersion
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not $IsWindows) { throw 'Native Windows verification requires Windows and PowerShell 7.' }
$archive = (Resolve-Path -LiteralPath $ArchivePath).Path
$work = Join-Path ([IO.Path]::GetTempPath()) ('DraxxMeasure-portable-test-' + [Guid]::NewGuid().ToString('N'))
$app = Join-Path $work 'portable application - µ'
$cwd = Join-Path $work 'unrelated working directory'
[IO.Directory]::CreateDirectory($cwd) | Out-Null

function Assert-That([bool]$Condition, [string]$Message) {
    if (-not $Condition) { throw $Message }
}
function Invoke-Cli([string[]]$CliArguments, [string]$InputText) {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = Join-Path $app 'draxxmeasure.exe'
    $start.WorkingDirectory = $cwd
    $start.UseShellExecute = $false
    $start.RedirectStandardInput = $true
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.StandardInputEncoding = [Text.UTF8Encoding]::new($false)
    $start.StandardOutputEncoding = [Text.UTF8Encoding]::new($false)
    $start.StandardErrorEncoding = [Text.UTF8Encoding]::new($false)
    foreach ($argument in $CliArguments) { $start.ArgumentList.Add($argument) }
    # The child has no Rust, Cargo, Python, Git, Tauri, or workspace PATH entries.
    $start.Environment['PATH'] = "$env:SystemRoot\System32;$env:SystemRoot"
    foreach ($name in @('CARGO_HOME', 'CARGO_MANIFEST_DIR', 'RUSTUP_HOME', 'PYTHONHOME', 'PYTHONPATH', 'VIRTUAL_ENV')) {
        $start.Environment.Remove($name) | Out-Null
    }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $start
    try {
        Assert-That ($process.Start()) 'Could not start standalone executable.'
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        $process.StandardInput.Write($InputText)
        $process.StandardInput.Close()
        if (-not $process.WaitForExit(90000)) { $process.Kill($true); throw 'Standalone executable timed out.' }
        $out = $stdout.GetAwaiter().GetResult()
        $err = $stderr.GetAwaiter().GetResult()
        Assert-That ($process.ExitCode -eq 0) "Standalone executable failed: $err`n$out"
        Assert-That ([string]::IsNullOrWhiteSpace($err)) "Unexpected stderr: $err"
        return $out
    }
    finally { $process.Dispose() }
}

try {
    # Inspect before extracting, rejecting databases, reports and unexpected paths.
    $zip = [IO.Compression.ZipFile]::OpenRead($archive)
    try {
        $names = @($zip.Entries | ForEach-Object FullName | Sort-Object)
        $expected = @('BUILD-INFO.txt', 'FONT-LICENSE.txt', 'WINDOWS-README.txt', 'draxxmeasure.exe') | Sort-Object
        Assert-That (($names -join '|') -ceq ($expected -join '|')) 'ZIP contents do not match the data-free allowlist.'
    }
    finally { $zip.Dispose() }
    Expand-Archive -LiteralPath $archive -DestinationPath $app
    $exe = Join-Path $app 'draxxmeasure.exe'

    # Hosted runners have build tools installed; inspect PE imports as well as
    # restricting the child PATH so they cannot mask a runtime redistributable need.
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    $dumpbins = @(& $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -find 'VC\Tools\MSVC\*\bin\Hostx64\x64\dumpbin.exe')
    Assert-That ($LASTEXITCODE -eq 0 -and $dumpbins.Count -gt 0) 'MSVC dumpbin not found for dependency inspection.'
    $dependents = & ($dumpbins[0]) /nologo /dependents $exe
    Assert-That ($LASTEXITCODE -eq 0) 'PE dependency inspection failed.'
    $dlls = @($dependents | ForEach-Object { $_.Trim() } | Where-Object { $_ -match '^[a-zA-Z0-9_.-]+\.dll$' } | Sort-Object -Unique)
    Assert-That ($dlls.Count -gt 0) 'No PE dependencies parsed; cannot assert portability.'
    # ProcessPrng is provided by the Windows OS, not a bundled redistributable:
    # https://learn.microsoft.com/en-us/windows/win32/seccng/processprng
    $systemDlls = @('KERNEL32.dll','NTDLL.dll','BCRYPT.dll','BCRYPTPRIMITIVES.dll','ADVAPI32.dll','USERENV.dll','WS2_32.dll','OLE32.dll','OLEAUT32.dll','SHELL32.dll','USER32.dll','GDI32.dll','SECHOST.dll','CRYPT32.dll','RPCRT4.dll','SECUR32.dll','NETAPI32.dll','IPHLPAPI.dll','WINHTTP.dll','PSAPI.dll','VERSION.dll','SYNCHAPI.dll')
    Write-Host "PE imports to verify: $($dlls -join ', ')"
    foreach ($dll in $dlls) {
        Assert-That ($dll -in $systemDlls -or $dll -match '^(api-ms-win-(core|security)-|ext-ms-win-)[a-zA-Z0-9_.-]+\.dll$') "Unexpected non-OS runtime dependency: $dll (no VC++, SQLite or other sidecar DLLs are allowed)."
    }
    Write-Host "Verified OS-only PE imports: $($dlls -join ', ')"

    $version = Invoke-Cli -CliArguments @('--version') -InputText ''
    Assert-That ($version.Trim() -ceq "draxxmeasure $ExpectedVersion") 'Executable version mismatch.'
    $help = Invoke-Cli -CliArguments @('--help') -InputText ''
    Assert-That ($help.Contains('--data-dir')) 'Portable path help is missing.'
    Assert-That (-not (Test-Path -LiteralPath (Join-Path $app 'data'))) 'Help/version created inspection data.'

    $inputLines = @('/session','/partnumber','405363','9368.9','5','0.1','yes','/bundle') + (@('68') * 42) + @('71','/close','/close','/close','/exit')
    $captured = Invoke-Cli -CliArguments @() -InputText (($inputLines -join "`n") + "`n")
    Assert-That ($captured.Contains('saved [42/42]')) 'Release binary did not accept 42 measurements.'
    Assert-That ($captured.Contains('bundle is full')) 'Release binary did not reject measurement #43.'
    Assert-That (-not $captured.Contains('saved [43/42]')) 'Release binary exceeded capacity.'
    Assert-That (([regex]::Matches($captured, ' mm saved \[')).Count -eq 42) 'Unexpected number of accepted measurements.'
    Assert-That ($captured.Contains('PNG report generated') -and -not $captured.Contains('Report failed:')) 'Release PNG generation failed.'
    $database = Join-Path $app 'data/draxxmeasure.sqlite3'
    $png = Join-Path $app 'reports/session-001-part-405363-item-001.png'
    Assert-That (Test-Path -LiteralPath $database -PathType Leaf) 'Executable-relative SQLite database missing.'
    $bytes = [IO.File]::ReadAllBytes($png)
    Assert-That ($bytes.Length -gt 1000) 'PNG output is unexpectedly small.'
    Assert-That ([Convert]::ToHexString([byte[]]($bytes[0..7])) -eq '89504E470D0A1A0A') 'Invalid PNG signature.'
    Assert-That ([Convert]::ToHexString([byte[]]($bytes[16..23])) -eq '00000708000007D0') 'PNG dimensions are not 1800x2000.'
    $manifest = Get-Content -LiteralPath (Join-Path $app 'reports/session-001-part-405363-item-001.manifest.json') -Raw | ConvertFrom-Json
    Assert-That ($manifest.complete -and $manifest.items[0].measurement_count -eq 42) 'Manifest population is incorrect.'
    $rows = @(Import-Csv -LiteralPath (Join-Path $app 'reports/session-001-part-405363-item-001.csv') | Where-Object { $_.row_kind -eq 'effective' -and $_.value_mm -ne '' })
    Assert-That ($rows.Count -eq 42 -and @($rows | Where-Object raw_input -NE '68').Count -eq 0) 'Rejected measurement #43 was persisted.'
    Assert-That (-not (Test-Path -LiteralPath (Join-Path $cwd 'data'))) 'Default data depends on current working directory.'
    Assert-That (-not (Test-Path -LiteralPath (Join-Path $cwd 'reports'))) 'Default reports depend on current working directory.'

    # Update only binary/docs, with no data payload and with all processes stopped.
    $beforeDatabase = (Get-FileHash -LiteralPath $database).Hash
    $beforePng = (Get-FileHash -LiteralPath $png).Hash
    $update = Join-Path $work 'update payload'
    Expand-Archive -LiteralPath $archive -DestinationPath $update
    foreach ($name in $expected) { Copy-Item -LiteralPath (Join-Path $update $name) -Destination (Join-Path $app $name) -Force }
    Assert-That ((Get-FileHash -LiteralPath $database).Hash -eq $beforeDatabase) 'Updating binary/docs modified the database.'
    Assert-That ((Get-FileHash -LiteralPath $png).Hash -eq $beforePng) 'Updating binary/docs modified an existing report.'
    $reopened = Invoke-Cli -CliArguments @() -InputText "/sessions`n/report 1`n/exit`n"
    Assert-That ($reopened.Contains('Session 1: Closed') -and $reopened.Contains('PNG report generated')) 'Existing inspection could not be reopened/reported after update.'
    Assert-That (-not $reopened.Contains('Error:')) 'Reopening existing inspection failed.'
    Assert-That ((Get-FileHash -LiteralPath $png).Hash -eq $beforePng) 'Retry overwrote an existing report.'

    $custom = Join-Path $work 'custom data - µ'
    $configured = Invoke-Cli -CliArguments @('--data-dir', $custom) -InputText "/session`n/exit`n"
    Assert-That (-not $configured.Contains('Error:')) 'Custom data directory failed.'
    Assert-That (Test-Path -LiteralPath (Join-Path $custom 'draxxmeasure.sqlite3')) 'Custom data directory was ignored.'
    Write-Host 'PASS: extracted release runs outside the workspace, captures safely, exports PNG/CSV, preserves data on binary-only update, and supports custom paths.'
}
finally {
    # Generated synthetic fixtures only; never touch a user-selected data directory.
    if (Test-Path -LiteralPath $work) { Remove-Item -LiteralPath $work -Recurse -Force }
}
