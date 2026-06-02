# scripts/upgrade/verify-update.ps1
#
# Spec: deploy/02-upgrade-rollback.md 2.2.2 (GPG detached signature) + 2.3.1 (verifyGpgSignature +
# checksum) + 2.2.3 (pre-deployed public key). The REAL GPG + SHA-256 verification of an upgrade
# package, equivalent to the Dart UpgradeService.verifyGpgSignature / _downloadWithChecksum path,
# exposed for ops / CI / pre-apply checks before scripts/upgrade/apply-upgrade.ps1.
#
# Trust anchor: %ProgramData%\Stuchka\trust\stuchka-pubkey.asc (the APP UPGRADE master key, distinct
# from the KB subkey). The armored .asc is imported into an ephemeral GNUPGHOME for verification
# (gpg 2.4 cannot use an armored file directly as --keyring), keeping the same trust root the
# installer ships.
#
# DOCUMENTED SEAMS: gpg (deploy/00 0.7 GPG 2.4.x) and the real project keypair (see keys/README.md).
# If gpg is absent -> exit 2 (seam). A bad signature -> exit 1 (reject, never silently pass).
#
# No Emoji anywhere.

[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][string]$Package,
  [string]$Signature,
  [string]$Keyring = "$env:ProgramData\Stuchka\trust\stuchka-pubkey.asc",
  [string]$ExpectedSha256
)

$ErrorActionPreference = "Stop"
if (-not $Signature) { $Signature = "$Package.asc" }

if (-not (Test-Path $Package)) { Write-Error "[verify] package not found: $Package"; exit 1 }
if (-not (Test-Path $Signature)) { Write-Error "[verify] detached signature not found: $Signature"; exit 1 }

# Optional SHA-256 gate (deploy/02 2.3.1 _downloadWithChecksum).
if ($ExpectedSha256) {
  $actual = (Get-FileHash -Algorithm SHA256 -Path $Package).Hash.ToLower()
  if ($actual -ne $ExpectedSha256.ToLower()) {
    Write-Error "[verify] SHA-256 mismatch: expected $ExpectedSha256, got $actual"
    exit 1
  }
  Write-Host "[verify] SHA-256 OK"
}

$gpg = Get-Command gpg -ErrorAction SilentlyContinue
if (-not $gpg) {
  Write-Warning "[verify] SEAM: gpg not installed (deploy/00 0.7: GPG 2.4.x). Cannot verify upgrade package."
  exit 2
}
if (-not (Test-Path $Keyring)) {
  Write-Warning "[verify] SEAM: trust anchor keyring not found: $Keyring (installed by setup.iss to %ProgramData%\Stuchka\trust)."
  exit 2
}

$ephemeral = Join-Path $env:TEMP ("stuchka-upd-gpg-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $ephemeral -Force | Out-Null
try {
  & $gpg.Source --homedir $ephemeral --batch --quiet --import $Keyring 2>$null
  if ($LASTEXITCODE -ne 0) { Write-Error "[verify] could not import trust anchor keyring: $Keyring"; exit 1 }

  & $gpg.Source --homedir $ephemeral --trust-model always --batch --verify $Signature $Package
  if ($LASTEXITCODE -ne 0) {
    Write-Error "[verify] GPG signature INVALID for $Package -- reject (E_UPDATE_SIGNATURE_INVALID)"
    exit 1
  }
  Write-Host "[verify] GPG signature VALID for $Package"
  exit 0
} finally {
  Remove-Item $ephemeral -Recurse -Force -ErrorAction SilentlyContinue
}
