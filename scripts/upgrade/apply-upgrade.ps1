# scripts/upgrade/apply-upgrade.ps1
#
# Spec: deploy/02-upgrade-rollback.md 2.3.2 (PowerShell backup + replace + migrate + restart) and
# 2.1 (six-step atomic operation) and 2.3.3 (rollback boundary). Implemented field-for-field.
#
# Atomicity contract (deploy/02 2.1): Step 4 (backup) must fully land -- app dir + main-db snapshot
# + audit.sqlite snapshot + integrity gate (Step 4d) -- before Step 5 may run. Any Step 5-6 failure
# rolls back ALL THREE (dual binary + main-db + audit.sqlite) as a unit; a partial restore is a
# rollback FAILURE.
#
# REAL paths wired:
#   - GPG verify of the staged package was already performed by the Dart layer (deploy/02 2.3.1)
#     OR can be run here via scripts/upgrade/verify-update.ps1 before invoking this script.
#   - Authenticode re-check of the dual binary (Step 3 tail) runs for real via Get-AuthenticodeSignature.
#
# DOCUMENTED SEAMS:
#   - stuchka-core.exe is the DB/audit authority (D1): --db-snapshot / --db-migrate / --db-restore
#     / --backup-verify are its real subcommands. If stuchka-core.exe is absent (not yet built on
#     this host), those steps cannot run; the script reports the seam and aborts BEFORE touching
#     user data (it never proceeds past a failed snapshot, honouring the atomicity gate).
#   - The signing certificate is the OV/EV seam (Authenticode 'Valid' requires a real signed binary).
#
# No Emoji anywhere.

[CmdletBinding()]
param(
  [Parameter(Mandatory = $true)][string]$UpdatePackage,
  [Parameter(Mandatory = $true)][string]$InstallDir,
  [Parameter(Mandatory = $true)][string]$DataDir,
  [Parameter(Mandatory = $true)][string]$CurrentVersion,
  [Parameter(Mandatory = $true)][string]$NewVersion,
  # When set, skip the Authenticode re-check (only for hosts where the staged binaries are
  # intentionally unsigned source builds -- documents the signing seam explicitly).
  [switch]$AllowUnsigned
)

$ErrorActionPreference = "Stop"
$BackupDir   = Join-Path $DataDir "backups\upgrade-$CurrentVersion-$(Get-Date -Format 'yyyyMMddHHmmss')"
$LogFile     = Join-Path $DataDir "logs\upgrade-$NewVersion.log"
$CoreExe     = Join-Path $InstallDir "stuchka-core.exe"   # DB / audit authority (D1)
$AuditLive   = Join-Path $DataDir "audit\audit.sqlite"
$AuditBackup = Join-Path $BackupDir "audit.sqlite"

New-Item -ItemType Directory -Path (Split-Path $LogFile) -Force | Out-Null

function Write-UpgradeLog {
  param($Message)
  $Timestamp = Get-Date -Format 'yyyy-MM-ddTHH:mm:ssZ'
  Add-Content -Path $LogFile -Value "[$Timestamp] $Message"
  Write-Host "[upgrade] $Message"
}

# Atomic rollback: dual binary + main-db snapshot + audit.sqlite must be restored as a UNIT.
function Invoke-Rollback {
  Write-UpgradeLog "ROLLBACK initiated (app + main-db + audit.sqlite)"
  if (Test-Path "$BackupDir\app") {
    Copy-Item -Path "$BackupDir\app\*" -Destination $InstallDir -Recurse -Force
  }
  if (Test-Path $CoreExe) {
    & $CoreExe --db-restore --snapshot "$BackupDir\db-snapshot" --data-dir $DataDir
    if ($LASTEXITCODE -ne 0) { Write-UpgradeLog "FATAL: main-db restore failed"; throw "Rollback failed: main-db" }
  }
  if (Test-Path $AuditBackup) { Copy-Item -Path $AuditBackup -Destination $AuditLive -Force }
  Write-UpgradeLog "ROLLBACK completed"
}

try {
  if (-not (Test-Path $UpdatePackage)) { throw "Update package not found: $UpdatePackage" }

  # Step 3 (tail): Authenticode re-check of the staged dual binary (GPG already verified upstream).
  Write-UpgradeLog "Step 3: Authenticode verify staged dual binary"
  $StagingDir = Join-Path $env:TEMP "stuchka-staging-$NewVersion"
  if (Test-Path $StagingDir) { Remove-Item $StagingDir -Recurse -Force }
  Expand-Archive -Path $UpdatePackage -DestinationPath $StagingDir -Force
  foreach ($bin in @("stuchka.exe", "stuchka-core.exe")) {
    $binPath = Join-Path $StagingDir $bin
    if (-not (Test-Path $binPath)) { throw "Staged package missing required binary: $bin (D1)" }
    if (-not $AllowUnsigned) {
      $st = (Get-AuthenticodeSignature $binPath).Status
      if ($st -ne 'Valid') { throw "Authenticode invalid for $bin ($st)" }
    } else {
      Write-UpgradeLog "SEAM: -AllowUnsigned set; skipping Authenticode for $bin (unsigned source build)"
    }
  }

  # Step 4a: back up application binaries (dual binary + runtime).
  Write-UpgradeLog "Step 4a: backing up application binaries"
  New-Item -ItemType Directory -Path "$BackupDir\app" -Force | Out-Null
  if (Test-Path "$InstallDir\*") { Copy-Item -Path "$InstallDir\*" -Destination "$BackupDir\app" -Recurse -Force }

  if (-not (Test-Path $CoreExe)) {
    Write-UpgradeLog "SEAM: stuchka-core.exe not present at $CoreExe; DB snapshot/migrate cannot run."
    throw "stuchka-core.exe (DB/audit authority, D1) missing; aborting BEFORE any DB mutation (atomicity gate honoured)."
  }

  # Step 4b: main database online snapshot (DB choice TBD; --db-snapshot masks PG/SQLite, D2).
  Write-UpgradeLog "Step 4b: snapshotting main database"
  & $CoreExe --db-snapshot --output "$BackupDir\db-snapshot" --data-dir $DataDir
  if ($LASTEXITCODE -ne 0) { throw "Main database snapshot failed" }

  # Step 4c: independent audit.sqlite snapshot (D3).
  Write-UpgradeLog "Step 4c: snapshotting audit.sqlite"
  if (Test-Path $AuditLive) {
    New-Item -ItemType Directory -Path $BackupDir -Force | Out-Null
    Copy-Item -Path $AuditLive -Destination $AuditBackup -Force
  }

  # Step 4d: backup integrity gate -- all three assets' checksums must land before Step 5.
  Write-UpgradeLog "Step 4d: backup integrity gate"
  & $CoreExe --backup-verify --backup-dir $BackupDir
  if ($LASTEXITCODE -ne 0) { throw "Backup integrity gate failed; refuse to enter Step 5" }

  # Step 5b: wait for the running process tree (main process cascade-kills the core subprocess).
  Write-UpgradeLog "Step 5b: waiting for stuchka.exe / stuchka-core.exe to exit"
  $Timeout = 30
  while (((Get-Process -Name stuchka -ErrorAction SilentlyContinue) -or `
          (Get-Process -Name stuchka-core -ErrorAction SilentlyContinue)) -and $Timeout -gt 0) {
    Start-Sleep -Seconds 1
    $Timeout--
  }
  if ((Get-Process -Name stuchka -ErrorAction SilentlyContinue) -or `
      (Get-Process -Name stuchka-core -ErrorAction SilentlyContinue)) {
    throw "Stuchka process tree did not exit within 30 seconds"
  }

  # Step 5c: replace files (dual binary + runtime; migrations not copied into the install dir).
  Write-UpgradeLog "Step 5c: replacing application files (dual binary)"
  Copy-Item -Path "$StagingDir\*" -Destination $InstallDir -Recurse -Force -Exclude "migrations"

  # Step 5d: main-db migration (sqlx migrate via stuchka-core; one-way, protected by Step 4b/4c).
  Write-UpgradeLog "Step 5d: applying main-db migrations via stuchka-core (sqlx)"
  & $CoreExe --db-migrate --migrations-dir "$StagingDir\migrations" --data-dir $DataDir
  if ($LASTEXITCODE -ne 0) { throw "Database migration failed" }

  # Step 6: restart + health check (covers fork subprocess + READY parse, D1).
  Write-UpgradeLog "Step 6: launching new version (health-check covers subprocess READY)"
  $Process = Start-Process -FilePath "$InstallDir\stuchka.exe" -ArgumentList "--health-check" -Wait -PassThru -NoNewWindow
  if ($Process.ExitCode -ne 0) { throw "Health check failed with exit code $($Process.ExitCode)" }

  Write-UpgradeLog "Upgrade completed successfully: $CurrentVersion -> $NewVersion"
  Start-Process -FilePath "$InstallDir\stuchka.exe"
  exit 0

} catch {
  Write-UpgradeLog "ERROR: $($_.Exception.Message)"
  try { Invoke-Rollback } catch { Write-UpgradeLog "ROLLBACK ERROR: $($_.Exception.Message)" }
  if (Test-Path "$InstallDir\stuchka.exe") {
    Start-Process -FilePath "$InstallDir\stuchka.exe" -ArgumentList "--upgrade-failed", "--rolled-back-from", $NewVersion
  }
  exit 1
}
