# scripts/build/build_workshop.ps1
#
# Cyrillic-path build workshop (README "西里尔路径构建坊").
#
# The repository root directory name is `Stučka` and contains a Cyrillic / diacritic
# character (`c with caron`). On Windows a handful of build tools (CMake, individual
# Rust build scripts, native Node modules) handle non-ASCII absolute paths or non-UTF-8
# code pages unreliably and may fail when the absolute path contains that character.
#
# This script isolates the BUILD PATH only (it never edits source): it mirrors the repo
# into a pure-ASCII staging directory under %TEMP%, runs the requested build command there,
# then copies the produced Release artifacts back into the real repo's
# build\windows\x64\runner\Release directory so downstream steps (trim_release.ps1,
# installer/setup.iss) see them at the canonical path.
#
# Cross-machine: tools (cargo / flutter) are resolved from PATH; UTF-8 code page is forced
# (chcp 65001) inside the staging shell. No Emoji anywhere.
#
# Usage:
#   pwsh scripts/build/build_workshop.ps1                       # full: cargo + flutter release
#   pwsh scripts/build/build_workshop.ps1 -Stage RustOnly       # only cargo build --release
#   pwsh scripts/build/build_workshop.ps1 -DryRun               # report the plan, build nothing
#
# Exit codes: 0 success; 2 a required tool is missing (documented seam, e.g. flutter on a
# headless CI without the windows toolchain); non-zero otherwise (real build failure).

[CmdletBinding()]
param(
  [ValidateSet("Full", "RustOnly", "FlutterOnly")]
  [string]$Stage = "Full",
  [string]$StagingRoot = (Join-Path $env:TEMP "stuchka-build-ascii"),
  [switch]$DryRun
)

$ErrorActionPreference = "Stop"

# Resolve the real repository root (two levels up from this script: scripts/build -> repo).
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
Write-Host "[workshop] real repo root : $RepoRoot"
Write-Host "[workshop] ascii staging  : $StagingRoot"
Write-Host "[workshop] stage          : $Stage"

# Detect whether the repo path is already pure ASCII; if so the workshop is a no-op passthrough.
$IsAsciiPath = ($RepoRoot -match '^[\x00-\x7F]+$')
if ($IsAsciiPath) {
  Write-Host "[workshop] repo path is already pure ASCII; building in place (no staging needed)."
  $WorkDir = $RepoRoot
} else {
  Write-Host "[workshop] repo path contains non-ASCII characters; staging to ASCII path."
  $WorkDir = $StagingRoot
}

function Resolve-Tool([string]$name) {
  $cmd = Get-Command $name -ErrorAction SilentlyContinue
  if ($null -eq $cmd) { return $null }
  return $cmd.Source
}

$cargo = Resolve-Tool "cargo"
$flutter = Resolve-Tool "flutter"

$needCargo = ($Stage -eq "Full" -or $Stage -eq "RustOnly")
$needFlutter = ($Stage -eq "Full" -or $Stage -eq "FlutterOnly")

if ($needCargo -and -not $cargo) {
  Write-Warning "[workshop] SEAM: 'cargo' not on PATH. Install the Rust 1.84 toolchain (rust-toolchain.toml). Cannot build stuchka-core.exe."
  exit 2
}
if ($needFlutter -and -not $flutter) {
  Write-Warning "[workshop] SEAM: 'flutter' not on PATH. Install Flutter 3.27.x stable + 'flutter config --enable-windows-desktop'. Cannot build stuchka.exe."
  exit 2
}

if ($DryRun) {
  Write-Host "[workshop] DRY RUN -- planned actions:"
  if (-not $IsAsciiPath) { Write-Host "  - mirror $RepoRoot -> $WorkDir (robocopy, excludes target/build/.git/node_modules)" }
  if ($needCargo) { Write-Host "  - (in $WorkDir\backend) cargo build --release --bin stuchka-core --target x86_64-pc-windows-msvc" }
  if ($needFlutter) { Write-Host "  - (in $WorkDir\frontend) flutter build windows --release --obfuscate --split-debug-info=build\symbols" }
  if (-not $IsAsciiPath) { Write-Host "  - copy artifacts back to $RepoRoot\build\windows\x64\runner\Release" }
  exit 0
}

# 1) Mirror the repo into the ASCII staging dir (source isolation only; never edits the original).
if (-not $IsAsciiPath) {
  if (-not (Test-Path $WorkDir)) { New-Item -ItemType Directory -Path $WorkDir -Force | Out-Null }
  Write-Host "[workshop] mirroring repo to ASCII staging (this preserves source, excludes heavy build dirs)..."
  # robocopy mirror; /XD excludes regenerable dirs. robocopy exit codes < 8 are success.
  $rc = Start-Process -FilePath "robocopy.exe" -ArgumentList @(
    "`"$RepoRoot`"", "`"$WorkDir`"", "/MIR",
    "/XD", ".git", "target", "build", "node_modules", ".dart_tool", "installer\output",
    "/NFL", "/NDL", "/NP", "/NJH", "/NJS"
  ) -Wait -PassThru -NoNewWindow
  if ($rc.ExitCode -ge 8) { Write-Error "[workshop] robocopy mirror failed (exit $($rc.ExitCode))"; exit $rc.ExitCode }
}

# 2) Force UTF-8 code page for the build subshell (chcp 65001) and run the build commands.
$prevCp = (chcp) -replace '[^0-9]', ''
try {
  chcp 65001 | Out-Null

  if ($needCargo) {
    Write-Host "[workshop] cargo build --release --bin stuchka-core --target x86_64-pc-windows-msvc"
    Push-Location (Join-Path $WorkDir "backend")
    try {
      & $cargo build --release --bin stuchka-core --target x86_64-pc-windows-msvc
      if ($LASTEXITCODE -ne 0) { Write-Error "[workshop] cargo build failed (exit $LASTEXITCODE)"; exit $LASTEXITCODE }
    } finally { Pop-Location }
  }

  if ($needFlutter) {
    Write-Host "[workshop] flutter build windows --release --obfuscate --split-debug-info=build\symbols"
    Push-Location (Join-Path $WorkDir "frontend")
    try {
      & $flutter build windows --release --obfuscate --split-debug-info=build\symbols
      if ($LASTEXITCODE -ne 0) { Write-Error "[workshop] flutter build failed (exit $LASTEXITCODE)"; exit $LASTEXITCODE }
    } finally { Pop-Location }
  }
} finally {
  if ($prevCp) { chcp $prevCp | Out-Null }
}

# 3) Copy artifacts back to the canonical repo path so trim_release.ps1 / setup.iss see them.
if (-not $IsAsciiPath) {
  Write-Host "[workshop] copying Release artifacts back to the real repo path..."
  $srcRelease = Join-Path $WorkDir "frontend\build\windows\x64\runner\Release"
  $dstRelease = Join-Path $RepoRoot "frontend\build\windows\x64\runner\Release"
  if (Test-Path $srcRelease) {
    New-Item -ItemType Directory -Path $dstRelease -Force | Out-Null
    Copy-Item -Path (Join-Path $srcRelease "*") -Destination $dstRelease -Recurse -Force
  }
  $srcCore = Join-Path $WorkDir "backend\target\x86_64-pc-windows-msvc\release\stuchka-core.exe"
  $dstCoreDir = Join-Path $RepoRoot "backend\target\x86_64-pc-windows-msvc\release"
  if (Test-Path $srcCore) {
    New-Item -ItemType Directory -Path $dstCoreDir -Force | Out-Null
    Copy-Item -Path $srcCore -Destination $dstCoreDir -Force
  }
}

Write-Host "[workshop] done."
exit 0
