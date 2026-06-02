# scripts/build/trim_release.ps1
#
# Spec: deploy/01-windows-build.md 1.2.2 (product trim + dual-binary existence assertion +
# total size gate) and 1.7 (acceptance: < 200 MB incl. WebView2 offline, ~180 MB hard floor
# excl. WebView2/model). Implemented field-for-field, with the repo's real layout:
#   - Flutter Release output : frontend\build\windows\x64\runner\Release
#   - Rust subprocess binary  : backend\target\x86_64-pc-windows-msvc\release\stuchka-core.exe
#
# D1 dual-binary contract: stuchka.exe (Flutter main process) + stuchka-core.exe (Rust core
# subprocess) MUST both ship. This script copies stuchka-core.exe into the Release directory,
# strips non-distributed files, asserts the dual binary exists, and gates total size.
#
# Cross-machine: no Emoji; paths resolved relative to the repo root; works under the Cyrillic
# `Stučka` root because it only touches build outputs (already produced by build_workshop.ps1).
#
# Exit codes: 0 success; 1 a required binary is missing or the size gate is exceeded.

[CmdletBinding()]
param(
  # Allow CI to point at a custom Release dir / core binary if needed; defaults follow the repo layout.
  [string]$ReleaseDir,
  [string]$CoreSource,
  [double]$MaxSizeMB = 180
)

$ErrorActionPreference = "Stop"

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
if (-not $ReleaseDir) {
  $ReleaseDir = Join-Path $RepoRoot "frontend\build\windows\x64\runner\Release"
}
if (-not $CoreSource) {
  $CoreSource = Join-Path $RepoRoot "backend\target\x86_64-pc-windows-msvc\release\stuchka-core.exe"
}
$CoreBin = Join-Path $ReleaseDir "stuchka-core.exe"

Write-Host "[trim] release dir : $ReleaseDir"
Write-Host "[trim] core source : $CoreSource"

if (-not (Test-Path $ReleaseDir)) {
  Write-Error "[trim] Release directory not found: $ReleaseDir (run scripts/build/build_workshop.ps1 first)"
  exit 1
}

# 1) Copy the Rust subprocess binary next to the Flutter binary (D1 -- ships in the same {app} dir).
if (Test-Path $CoreSource) {
  Copy-Item $CoreSource $CoreBin -Force
  Write-Host "[trim] copied stuchka-core.exe into Release dir"
} else {
  Write-Warning "[trim] stuchka-core.exe source missing: $CoreSource"
  Write-Warning "[trim] SEAM: build it via 'cargo build --release --bin stuchka-core --target x86_64-pc-windows-msvc' (or build_workshop.ps1 RustOnly)."
  # Do NOT fake it -- fall through to the existence assertion which will fail honestly.
}

# 2) Strip non-distributed files (keep stuchka.exe / stuchka-core.exe / flutter_windows.dll / data/ / plugins).
foreach ($pat in @("*.pdb", "*.exp", "*.lib")) {
  Remove-Item (Join-Path $ReleaseDir $pat) -Force -ErrorAction SilentlyContinue
}

# 3) Dual-binary existence assertion (D1: missing any one fails the build; never ship main-process only).
$required = @(
  (Join-Path $ReleaseDir "stuchka.exe"),
  $CoreBin,
  (Join-Path $ReleaseDir "flutter_windows.dll")
)
$missing = @()
foreach ($bin in $required) {
  if (-not (Test-Path $bin)) { $missing += $bin }
}
if ($missing.Count -gt 0) {
  Write-Error ("[trim] Required binary missing (D1 dual-binary assertion): " + ($missing -join ", "))
  exit 1
}
Write-Host "[trim] dual-binary assertion PASS (stuchka.exe + stuchka-core.exe + flutter_windows.dll present)"

# 4) Total size gate: dual binary + all runtime files < MaxSizeMB (default 180; 20 MB headroom to the 200 MB hard gate after WebView2 offline).
$Size = (Get-ChildItem $ReleaseDir -Recurse -File | Measure-Object -Property Length -Sum).Sum / 1MB
$SizeRounded = [math]::Round($Size, 2)
Write-Host "[trim] Release build size (incl. stuchka-core.exe): $SizeRounded MB"
if ($Size -gt $MaxSizeMB) {
  Write-Error "[trim] Build size $SizeRounded MB exceeds $MaxSizeMB MB threshold (excl. WebView2 offline / model); review dependency tree (pubspec.lock + Cargo.lock)."
  exit 1
}
Write-Host "[trim] size gate PASS (<= $MaxSizeMB MB)"
exit 0
