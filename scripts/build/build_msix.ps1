# scripts/build/build_msix.ps1
#
# Spec: deploy/01-windows-build.md 1.5 (.msix packaging, Microsoft Store compatible). The
# msix_config block lives in frontend/pubspec.yaml (this script asserts it is present, then runs
# the documented build command). Dual binary (D1): cargo build + trim_release.ps1 must precede
# this so stuchka-core.exe is in the Release dir before `dart run msix:create`.
#
# DOCUMENTED SEAMS:
#   - The signing certificate (WINDOWS_CERT_PATH / WINDOWS_CERT_PASSWORD) is the same OV/EV cert
#     seam as sign-windows.ps1; without it `msix:create` cannot sign the package.
#   - .msix Store listing additionally requires L0-02 (law-firm filing-exemption opinion). Until
#     then the .msix is an internal-test artifact only (deploy/01 1.5, deploy/00 0.4).
#   - flutter + dart are resolved from PATH; absent on a headless runner -> exit 2 (tool seam).
#
# No Emoji anywhere.

[CmdletBinding()]
param(
  [string]$CertPath = $env:WINDOWS_CERT_PATH,
  [string]$CertPassword = $env:WINDOWS_CERT_PASSWORD,
  [switch]$SkipClean
)

$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..\..")).Path
$Frontend = Join-Path $RepoRoot "frontend"
$Pubspec = Join-Path $Frontend "pubspec.yaml"

function Resolve-Tool([string]$name) {
  $cmd = Get-Command $name -ErrorAction SilentlyContinue
  if ($null -eq $cmd) { return $null }
  return $cmd.Source
}

$flutter = Resolve-Tool "flutter"
$dart = Resolve-Tool "dart"
if (-not $flutter -or -not $dart) {
  Write-Warning "[msix] SEAM: flutter/dart not on PATH. Install Flutter 3.27.x stable. Cannot build .msix."
  exit 2
}

# Assert the msix_config block is present in pubspec.yaml (deploy/01 1.5).
$pubspecText = Get-Content $Pubspec -Raw
if ($pubspecText -notmatch '(?m)^msix_config:') {
  Write-Error "[msix] pubspec.yaml is missing the msix_config block (deploy/01 1.5). Aborting."
  exit 1
}
Write-Host "[msix] msix_config block present in pubspec.yaml"

Push-Location $Frontend
try {
  if (-not $SkipClean) {
    & $flutter clean
    if ($LASTEXITCODE -ne 0) { Write-Error "[msix] flutter clean failed"; exit $LASTEXITCODE }
  }
  & $flutter pub get
  if ($LASTEXITCODE -ne 0) { Write-Error "[msix] flutter pub get failed"; exit $LASTEXITCODE }

  & $flutter build windows --release --obfuscate --split-debug-info=build\symbols
  if ($LASTEXITCODE -ne 0) { Write-Error "[msix] flutter build windows failed"; exit $LASTEXITCODE }

  # Ensure the dual binary is staged before packaging (D1) -- run trim_release if core is missing.
  $releaseDir = Join-Path $Frontend "build\windows\x64\runner\Release"
  if (-not (Test-Path (Join-Path $releaseDir "stuchka-core.exe"))) {
    Write-Host "[msix] staging stuchka-core.exe via trim_release.ps1 (D1)"
    & (Join-Path $RepoRoot "scripts\build\trim_release.ps1")
    if ($LASTEXITCODE -ne 0) { Write-Error "[msix] trim_release.ps1 failed; .msix would lack the subprocess."; exit $LASTEXITCODE }
  }

  if ([string]::IsNullOrWhiteSpace($CertPath)) {
    Write-Warning "[msix] SEAM: no certificate (WINDOWS_CERT_PATH unset). Reporting the command that WILL run with the OV/EV cert:"
    Write-Host "  dart run msix:create --certificate-path=<cert.pfx> --certificate-password=<password>"
    Write-Warning "[msix] Also note: Store listing requires L0-02 (law-firm opinion); until then .msix is internal-test only."
    exit 3
  }

  & $dart run msix:create --certificate-path=$CertPath --certificate-password=$CertPassword
  if ($LASTEXITCODE -ne 0) { Write-Error "[msix] dart run msix:create failed"; exit $LASTEXITCODE }
  Write-Host "[msix] .msix built and signed."
} finally {
  Pop-Location
}
exit 0
