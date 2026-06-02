# scripts/recovery/restore-from-backup.ps1
#
# Spec: deploy/03-disaster-recovery.md 3.6.1 (new-machine restore from .stuchka-backup).
# Implemented field-for-field. Restores TWO independent DB assets (main-db snapshot + independent
# audit.sqlite, D2/D3) plus file assets (evidence / kb-pin / config) from an age-encrypted +
# zstd-compressed backup.
#
# Pipeline (deploy/03 3.2.1): .stuchka-backup --age decrypt--> .tar.zst --zstd--> .tar --tar--> tree.
#
# DOCUMENTED SEAMS:
#   - age (deploy/00 0.7: age 1.2.x) decrypts the outer envelope. If absent -> exit 2 (seam).
#   - stuchka-core.exe (D1) is the DB restore/integrity/audit-chain authority via --db-restore /
#     --db-integrity-check / --audit-chain-verify. If absent -> exit 2 (seam), reported clearly.
#   - zstd + tar are resolved from PATH (zstd is in the deploy baseline; tar ships with Windows 10+).
#
# Safety: an existing target dir is renamed (never silently overwritten) unless -DryRun.
# No Emoji anywhere.

[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][string]$BackupFile,
  [Parameter(Mandatory = $true)][string]$PassphraseFile,
  [string]$TargetDataDir = "$env:LOCALAPPDATA\Stuchka",
  [string]$InstallDir = "$env:ProgramFiles\Stuchka",
  [switch]$DryRun
)

$ErrorActionPreference = "Stop"
$CoreExe = Join-Path $InstallDir "stuchka-core.exe"   # DB restore authority (D1)

function Resolve-Tool([string]$name) {
  $cmd = Get-Command $name -ErrorAction SilentlyContinue
  if ($null -eq $cmd) { return $null }
  return $cmd.Source
}

# Step 1: preflight.
if (-not (Test-Path $BackupFile)) { throw "Backup file not found: $BackupFile" }
if (-not (Test-Path $PassphraseFile)) { throw "Passphrase file not found: $PassphraseFile" }

$age = Resolve-Tool "age"
$zstd = Resolve-Tool "zstd"
$tar = Resolve-Tool "tar"
if (-not $age) { Write-Warning "[restore] SEAM: 'age' not installed (deploy/00 0.7: age 1.2.x). Cannot decrypt .stuchka-backup."; exit 2 }
if (-not $zstd) { Write-Warning "[restore] SEAM: 'zstd' not on PATH (deploy baseline). Cannot decompress."; exit 2 }
if (-not $tar) { Write-Warning "[restore] SEAM: 'tar' not on PATH (Windows 10+ bundles bsdtar)."; exit 2 }

if ((Test-Path $TargetDataDir) -and -not $DryRun) {
  $Confirm = Read-Host "Target directory exists. Type 'OVERWRITE' to continue (it will be renamed, not deleted)"
  if ($Confirm -ne "OVERWRITE") { throw "Restore cancelled" }
  Rename-Item $TargetDataDir "$TargetDataDir.before-restore-$(Get-Date -Format 'yyyyMMddHHmmss')"
}

# Step 2: verify backup integrity (the app/core CLI runs the manifest-bound per-file sha256 check).
if (Test-Path $CoreExe) {
  Write-Host "[restore] verifying backup integrity via stuchka-core --verify-backup..."
  & $CoreExe --verify-backup --input $BackupFile --passphrase-file $PassphraseFile --json
  if ($LASTEXITCODE -ne 0) { throw "Backup verification failed" }
} else {
  Write-Warning "[restore] SEAM: stuchka-core.exe not present; skipping --verify-backup (run scripts/recovery/verify-backup.ps1 for a standalone manifest check)."
}

if ($DryRun) { Write-Host "[restore] DRY RUN complete (no files written)."; exit 0 }

# Step 3: decrypt + decompress + extract.
$StagingDir = Join-Path $env:TEMP "stuchka-restore-$(Get-Random)"
New-Item -ItemType Directory -Path $StagingDir | Out-Null
try {
  Write-Host "[restore] decrypting + extracting..."
  $tarZst = Join-Path $StagingDir 'backup.tar.zst'
  Get-Content $PassphraseFile | & $age --decrypt --output $tarZst $BackupFile
  if ($LASTEXITCODE -ne 0) { throw "age decrypt failed" }
  $tarFile = Join-Path $StagingDir 'backup.tar'
  & $zstd --decompress $tarZst -o $tarFile
  if ($LASTEXITCODE -ne 0) { throw "zstd decompress failed" }
  & $tar -xf $tarFile -C $StagingDir
  if ($LASTEXITCODE -ne 0) { throw "tar extract failed" }

  # Step 4: land the two DB assets + file assets.
  New-Item -ItemType Directory -Path $TargetDataDir -Force | Out-Null

  if (-not (Test-Path $CoreExe)) {
    Write-Warning "[restore] SEAM: stuchka-core.exe missing; cannot run --db-restore / integrity checks."
    throw "stuchka-core.exe (DB restore authority, D1) required to land the main-db snapshot."
  }

  # 4a: main database restore (DB choice TBD; --db-restore masks PG/SQLite, D2).
  & $CoreExe --db-restore --snapshot "$StagingDir\main-db\snapshot" --data-dir $TargetDataDir
  if ($LASTEXITCODE -ne 0) { throw "Main database restore failed" }

  # 4b: independent audit.sqlite restore (D3) + OTS proofs.
  New-Item -ItemType Directory -Path (Join-Path $TargetDataDir 'audit') -Force | Out-Null
  Copy-Item -Path "$StagingDir\audit.sqlite" -Destination (Join-Path $TargetDataDir 'audit\audit.sqlite') -Force
  if (Test-Path "$StagingDir\audit-ots") {
    Copy-Item -Path "$StagingDir\audit-ots" -Destination (Join-Path $TargetDataDir 'audit\ots') -Recurse -Force
  }

  # 4c: file assets.
  foreach ($asset in @("evidence", "kb-pin")) {
    if (Test-Path "$StagingDir\$asset") { Copy-Item -Path "$StagingDir\$asset" -Destination $TargetDataDir -Recurse -Force }
  }
  if (Test-Path "$StagingDir\config.toml") { Copy-Item -Path "$StagingDir\config.toml" -Destination $TargetDataDir -Force }

  # Step 5: consistency check (main-db integrity + audit.sqlite hash-chain continuity, D3).
  & $CoreExe --db-integrity-check --data-dir $TargetDataDir
  if ($LASTEXITCODE -ne 0) { throw "Main database integrity check failed after restore" }
  & $CoreExe --audit-chain-verify --data-dir $TargetDataDir
  if ($LASTEXITCODE -ne 0) { throw "Audit hash chain verification failed after restore" }

  Write-Host "[restore] Restore completed. Launch Stučka to verify data."
  exit 0
} finally {
  Remove-Item $StagingDir -Recurse -Force -ErrorAction SilentlyContinue
}
