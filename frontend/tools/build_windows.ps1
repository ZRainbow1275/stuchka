<#
.SYNOPSIS
  Cross-machine Windows build harness for the Stucka Flutter desktop shell.

.DESCRIPTION
  The repo root lives at a path containing a Cyrillic 'c' with caron (...\Stucka\...) which MUST
  stay there (product constraint). The Flutter Windows toolchain's native shader compiler
  (impellerc) cannot write its output into a build path that contains non-ASCII characters, so
  `flutter build windows` fails at the shader-compile step ONLY because of the non-ASCII repo path
  (the Dart code itself compiles cleanly: app.dill builds, impellerc exits 0 to an ASCII path).

  Rather than move the repo (forbidden), this harness:
    1. stages the project into an ASCII-only working directory under $env:TEMP, copying the source
       trees with robocopy and EXCLUDING the throwaway build/ and .dart_tool/ caches;
    2. runs `flutter pub get` then `flutter build windows` IN that ASCII directory, where impellerc
       can write shaders;
    3. copies the produced Runner bundle (incl. the .exe) back into the real repo at
       frontend\build\windows-ascii-staged\ and reports the staged artifact path + the REAL
       `flutter build windows` exit code.

  The harness is idempotent (re-running mirrors fresh sources and overwrites prior artifacts) and
  path-agnostic: it derives the project root from its own location, so it works on any machine
  regardless of where the repo is checked out (cross-machine compatible).

.PARAMETER Mode
  release (default) or debug. Passed through to `flutter build windows --<mode>`.

.PARAMETER StageRoot
  Override the ASCII staging root. Defaults to $env:TEMP\stuchka_build (which is ASCII on a normal
  Windows install). Supply an explicit ASCII path if your %TEMP% itself contains non-ASCII chars.

.EXAMPLE
  pwsh -File tools/build_windows.ps1
  pwsh -File tools/build_windows.ps1 -Mode debug
#>
[CmdletBinding()]
param(
  [ValidateSet('release', 'debug')]
  [string]$Mode = 'release',
  [string]$StageRoot = (Join-Path $env:TEMP 'stuchka_build')
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# --- Resolve the real frontend project root from this script's own location (path-agnostic). ---
$scriptDir   = Split-Path -Parent $MyInvocation.MyCommand.Path  # ...\frontend\tools
$projectRoot = Split-Path -Parent $scriptDir                    # ...\frontend
Write-Host "[stuchka-build] project root : $projectRoot"
Write-Host "[stuchka-build] staging root : $StageRoot"
Write-Host "[stuchka-build] build mode   : $Mode"

# --- Locate the Flutter CLI (PATH first, then common install dirs) for cross-machine robustness. ---
$flutter = (Get-Command flutter -ErrorAction SilentlyContinue)
if ($null -ne $flutter) {
  $flutterCmd = $flutter.Source
} else {
  $candidates = @(
    (Join-Path $env:LOCALAPPDATA 'flutter\bin\flutter.bat'),
    'C:\flutter\bin\flutter.bat',
    'C:\src\flutter\bin\flutter.bat'
  )
  $flutterCmd = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
  if (-not $flutterCmd) {
    throw 'flutter CLI not found on PATH or in common install locations. Install Flutter or add it to PATH.'
  }
}
Write-Host "[stuchka-build] flutter      : $flutterCmd"

# --- Warn (do not fail) if the chosen staging root is itself non-ASCII. ---
if ($StageRoot -match '[^\x00-\x7F]') {
  Write-Warning "[stuchka-build] StageRoot contains non-ASCII characters; impellerc may still fail. Pass an ASCII -StageRoot."
}

# --- (1) Stage the project into the ASCII working dir. Mirror sources; drop caches. ---
if (Test-Path $StageRoot) {
  Write-Host "[stuchka-build] cleaning prior stage at $StageRoot"
  Remove-Item -Recurse -Force -LiteralPath $StageRoot
}
New-Item -ItemType Directory -Force -Path $StageRoot | Out-Null

# Directory trees that must be present for `flutter build windows`.
$dirTrees = @('lib', 'test', 'tools', 'assets', 'windows', 'web')
foreach ($d in $dirTrees) {
  $src = Join-Path $projectRoot $d
  if (-not (Test-Path $src)) { continue }
  $dst = Join-Path $StageRoot $d
  # robocopy /MIR mirrors; /XD prunes the throwaway caches anywhere in the tree.
  # robocopy exit codes 0-7 are success (>=8 is a real error).
  robocopy $src $dst /MIR /NFL /NDL /NJH /NJS /NP /XD build .dart_tool ephemeral | Out-Null
  if ($LASTEXITCODE -ge 8) { throw "robocopy failed mirroring $d (exit $LASTEXITCODE)" }
}

# Top-level files needed by the build.
$files = @('pubspec.yaml', 'pubspec.lock', 'analysis_options.yaml', '.metadata', 'README.md')
foreach ($f in $files) {
  $src = Join-Path $projectRoot $f
  if (Test-Path $src) { Copy-Item -LiteralPath $src -Destination (Join-Path $StageRoot $f) -Force }
}

Write-Host "[stuchka-build] staged sources into $StageRoot"

# --- (2) pub get + build IN the ASCII dir. ---
Push-Location $StageRoot
try {
  Write-Host "[stuchka-build] running: flutter pub get"
  & $flutterCmd pub get
  if ($LASTEXITCODE -ne 0) { throw "flutter pub get failed (exit $LASTEXITCODE)" }

  Write-Host "[stuchka-build] running: flutter build windows --$Mode"
  & $flutterCmd build windows "--$Mode"
  $buildExit = $LASTEXITCODE
  Write-Host "[stuchka-build] flutter build windows exit code: $buildExit"
}
finally {
  Pop-Location
}

# --- (3) Copy the produced bundle back into the real repo + report. ---
$runnerOut = Join-Path $StageRoot ("build\windows\x64\runner\" + (Get-Culture).TextInfo.ToTitleCase($Mode))
if (-not (Test-Path $runnerOut)) {
  # Older/newer SDKs sometimes omit the x64 segment; fall back to a search.
  $runnerOut = Get-ChildItem -Path (Join-Path $StageRoot 'build\windows') -Recurse -Directory -ErrorAction SilentlyContinue |
    Where-Object { $_.Name -ieq $Mode -and (Test-Path (Join-Path $_.FullName 'stuchka.exe')) } |
    Select-Object -First 1 -ExpandProperty FullName
}

$stagedArtifactDir = Join-Path $projectRoot 'build\windows-ascii-staged'
$exePath = $null
if ($runnerOut -and (Test-Path $runnerOut)) {
  New-Item -ItemType Directory -Force -Path $stagedArtifactDir | Out-Null
  robocopy $runnerOut $stagedArtifactDir /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
  if ($LASTEXITCODE -ge 8) { throw "robocopy failed copying artifacts back (exit $LASTEXITCODE)" }
  $exe = Get-ChildItem -Path $stagedArtifactDir -Filter 'stuchka.exe' -Recurse -ErrorAction SilentlyContinue | Select-Object -First 1
  if ($exe) { $exePath = $exe.FullName }
  Write-Host "[stuchka-build] copied bundle back to: $stagedArtifactDir"
} else {
  Write-Warning "[stuchka-build] no runner output found under $StageRoot\build\windows (build likely failed)."
}

Write-Host "============================================================"
Write-Host "[stuchka-build] RESULT"
Write-Host "  flutter build windows exit code : $buildExit"
Write-Host "  staged artifact dir             : $stagedArtifactDir"
Write-Host "  produced .exe                   : $(if ($exePath) { $exePath } else { '(none)' })"
Write-Host "============================================================"

# Propagate the real build exit code so CI / callers see the truth.
exit $buildExit
