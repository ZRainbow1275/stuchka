# Font fetcher (spec 01 §1.4) — PowerShell wrapper around the canonical Dart implementation.
#
# Downloads the redistributable SIL OFL fonts (Source Han Sans/Serif SC + JetBrains Mono) into
# assets/fonts/ and enables the managed `fonts:` block in pubspec.yaml. The build stays GREEN
# without the OTFs because StuchkaTheme.fontFamily falls back to the system font (graceful
# fallback). Zero runtime network — this is a build-time tool only (prd §5.5.1 零外发).
#
# Usage (run from Stučka/frontend):
#   pwsh tools/fetch_fonts.ps1            # download + enable
#   pwsh tools/fetch_fonts.ps1 -Check     # report presence only
#   pwsh tools/fetch_fonts.ps1 -Disable   # comment the fonts: block back out
#
# This simply delegates to `dart run tools/fetch_fonts.dart`, which is the single source of truth
# for the mirror list and the pubspec toggle.
param(
    [switch]$Check,
    [switch]$Disable
)

$ErrorActionPreference = 'Stop'
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$frontendDir = Split-Path -Parent $scriptDir
Push-Location $frontendDir
try {
    $dartArgs = @('run', 'tools/fetch_fonts.dart')
    if ($Check) { $dartArgs += '--check' }
    if ($Disable) { $dartArgs += '--disable' }
    & dart @dartArgs
    exit $LASTEXITCODE
}
finally {
    Pop-Location
}
