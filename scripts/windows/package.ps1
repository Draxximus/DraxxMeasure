[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SourceDirectory,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][ValidatePattern('^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$')][string]$Version,
    [Parameter(Mandatory)][ValidatePattern('^[0-9a-fA-F]{40,64}$')][string]$Commit
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$source = (Resolve-Path -LiteralPath $SourceDirectory).Path
$output = [IO.Path]::GetFullPath($OutputDirectory)
$executable = Join-Path $source 'target/x86_64-pc-windows-msvc/release/draxxmeasure.exe'
if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) { throw 'Build the Windows x64 release executable before packaging.' }
[IO.Directory]::CreateDirectory($output) | Out-Null
$archive = Join-Path $output "DraxxMeasure-v${Version}-windows-x64.zip"
$checksum = "$archive.sha256"
if ((Test-Path -LiteralPath $archive) -or (Test-Path -LiteralPath $checksum)) { throw 'Package already exists; refusing to overwrite it.' }
$stage = Join-Path $output ('.stage-' + [Guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($stage) | Out-Null
try {
    # No recursive source-tree copy: a package can never contain a data/report folder.
    Copy-Item -LiteralPath $executable -Destination (Join-Path $stage 'draxxmeasure.exe')
    Copy-Item -LiteralPath (Join-Path $source 'docs/windows-portable-readme.txt') -Destination (Join-Path $stage 'WINDOWS-README.txt')
    Copy-Item -LiteralPath (Join-Path $source 'crates/draxx-reporting/assets/FONT-LICENSE.txt') -Destination (Join-Path $stage 'FONT-LICENSE.txt')
    @"
DraxxMeasure $Version
Source commit: $Commit
Target: x86_64-pc-windows-msvc
Rust: 1.97.1; static CRT; bundled SQLite; embedded DejaVu Sans
The package contains no inspection data, databases, reports, or runtime installers.
"@ | Set-Content -LiteralPath (Join-Path $stage 'BUILD-INFO.txt') -Encoding utf8
    $allowlist = @('draxxmeasure.exe', 'WINDOWS-README.txt', 'FONT-LICENSE.txt', 'BUILD-INFO.txt')
    $files = @($allowlist | ForEach-Object { Join-Path $stage $_ })
    Compress-Archive -LiteralPath $files -DestinationPath $archive -CompressionLevel Optimal
    $digest = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
    "$digest  $([IO.Path]::GetFileName($archive))" | Set-Content -LiteralPath $checksum -Encoding ascii
    Write-Host "Package: $archive"
    Write-Host "SHA-256: $digest"
}
finally {
    # Only this invocation's private, freshly-created staging folder is removed.
    Remove-Item -LiteralPath $stage -Recurse -Force
}
