# scripts/sign/sign-windows.ps1
#
# Spec: deploy/01-windows-build.md 1.3.2 (PowerShell signing command) + 1.3.3 (signing object
# list) + 1.3.4 (subprocess Authenticode check). Authenticode code signing via signtool.exe.
#
# DOCUMENTED SEAM (per task hard constraints): the OV/EV code-signing certificate is a real
# external prerequisite that is genuinely unavailable in this environment (R1 ships OV per
# 1.3.1; the cert is acquired against the L0-01 legal subject and stored as CI secrets
# WINDOWS_CERT_BASE64 / WINDOWS_CERT_PASSWORD / WINDOWS_CERT_THUMBPRINT). This script wires the
# REAL signtool invocation; without a real cert+thumbprint signtool fails honestly (no faking).
#
# When -CertThumbprint is the literal "SEAM" (or omitted), the script reports the exact command
# it WOULD run and exits 3 (seam-not-satisfied) so a pipeline can distinguish "no cert yet" from
# "signing genuinely failed".
#
# Cross-machine: signtool.exe is resolved by probing the Windows Kits bin dirs, then PATH. No Emoji.
#
# Usage:
#   pwsh scripts/sign/sign-windows.ps1 -FilePath build\...\stuchka.exe -CertThumbprint <40hex>
#   pwsh scripts/sign/sign-windows.ps1 -FilePath ... -CertThumbprint SEAM   # report-only seam mode

[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][string]$FilePath,
  [Parameter(Mandatory = $false)][string]$CertThumbprint = "SEAM",
  [string]$TimestampUrl = "http://timestamp.digicert.com",
  [string]$Description = "Stučka - China labour-dispute AI workbench",
  [string]$Url = "https://github.com/stuchka/stuchka"
)

$ErrorActionPreference = "Stop"

function Resolve-SignTool {
  # 1) Prefer the spec-pinned Windows 11 SDK 10.0.22621 x64 signtool, then any Kits version, then PATH.
  $kits = "${env:ProgramFiles(x86)}\Windows Kits\10\bin"
  $candidates = @()
  if (Test-Path $kits) {
    $candidates += "$kits\10.0.22621.0\x64\signtool.exe"
    Get-ChildItem -Path $kits -Directory -ErrorAction SilentlyContinue |
      Sort-Object Name -Descending |
      ForEach-Object { $candidates += (Join-Path $_.FullName "x64\signtool.exe") }
  }
  foreach ($c in $candidates) { if (Test-Path $c) { return $c } }
  $onPath = Get-Command signtool.exe -ErrorAction SilentlyContinue
  if ($onPath) { return $onPath.Source }
  return $null
}

if (-not (Test-Path $FilePath)) {
  Write-Error "[sign] target file not found: $FilePath"
  exit 1
}

$SignTool = Resolve-SignTool
if (-not $SignTool) {
  Write-Warning "[sign] SEAM: signtool.exe not found. Install the Windows 11 SDK (10.0.22621.x) 'Signing Tools for Desktop Apps'."
  exit 3
}
Write-Host "[sign] signtool: $SignTool"

if ([string]::IsNullOrWhiteSpace($CertThumbprint) -or $CertThumbprint -eq "SEAM") {
  Write-Warning "[sign] SEAM: no code-signing certificate thumbprint supplied."
  Write-Warning "[sign] The OV/EV cert is acquired against the L0-01 legal subject and injected as CI secret WINDOWS_CERT_THUMBPRINT."
  Write-Host "[sign] Command that WILL run once the cert is present:"
  Write-Host "  `"$SignTool`" sign /sha1 <THUMBPRINT> /fd sha256 /tr $TimestampUrl /td sha256 /d `"$Description`" /du `"$Url`" `"$FilePath`""
  Write-Host "  `"$SignTool`" verify /pa /v `"$FilePath`""
  exit 3
}

# REAL signing path (executes when a genuine cert thumbprint is present in the Windows cert store / CI).
& $SignTool sign `
  /sha1 $CertThumbprint `
  /fd sha256 `
  /tr $TimestampUrl `
  /td sha256 `
  /d $Description `
  /du $Url `
  $FilePath

if ($LASTEXITCODE -ne 0) {
  Write-Error "[sign] Signing failed for $FilePath (exit $LASTEXITCODE)"
  exit 1
}

& $SignTool verify /pa /v $FilePath
if ($LASTEXITCODE -ne 0) {
  Write-Error "[sign] Verification failed for $FilePath (exit $LASTEXITCODE)"
  exit 1
}

Write-Host "[sign] signed + verified: $FilePath"
exit 0
