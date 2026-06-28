# Build a full Rekaptr release locally.
#
# Produces in target/installer/:
#   rekaptr-setup-<version>.exe   Inno Setup installer
#   rekaptr-installer.ps1         axoupdater-compatible bootstrap script
#
# Usage:
#   pwsh scripts/build-release.ps1
#   pwsh scripts/build-release.ps1 -SkipBuild   # reuse existing dist artifacts

param(
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$Root = (Resolve-Path "$PSScriptRoot\..").Path
Set-Location $Root

$Version = (Select-String -Path "Cargo.toml" -Pattern '^version\s*=\s*"([^"]+)"').Matches[0].Groups[1].Value
Write-Host "Building Rekaptr $Version"

# Bundled tools the app shells out to at runtime. ffprobe in particular is
# required for cross-session recording: the decode-time-offset that keeps
# successive sessions on one continuous timeline is derived by probing on-disk
# segments. A missing ffprobe.exe makes the offset 0, which resets each new
# session's timestamps and breaks playback at the seam — fail the build instead.
$RequiredRuntimeBins = @("runtime\ffmpeg.exe", "runtime\ffprobe.exe")
$MissingBins = $RequiredRuntimeBins | Where-Object { -not (Test-Path $_) }
if ($MissingBins) {
    throw "Missing required runtime binaries: $($MissingBins -join ', '). " +
          "Place them in runtime\ (same FFmpeg build for both) before packaging."
}

# Mirror assets into runtime\ (what gets bundled) so dev and release can't drift.
Write-Host "==> sync assets -> runtime\assets"
robocopy "assets" "runtime\assets" /MIR /NJH /NJS /NDL /NFL /NP | Out-Null
# robocopy: exit codes < 8 are success; only >= 8 is a real failure.
if ($LASTEXITCODE -ge 8) { throw "asset sync (robocopy) failed with code $LASTEXITCODE" }
$global:LASTEXITCODE = 0

if (-not $SkipBuild) {
    Write-Host "==> dist build"
    & dist build
    if ($LASTEXITCODE -ne 0) { throw "dist build failed" }
}

$Iscc = "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
if (-not (Test-Path $Iscc)) {
    $Iscc = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
}
if (-not (Test-Path $Iscc)) {
    throw "Inno Setup 6 not found. Install with: winget install JRSoftware.InnoSetup"
}

Write-Host "==> iscc installer.iss"
& $Iscc "/DMyAppVersion=$Version" "installer.iss"
if ($LASTEXITCODE -ne 0) { throw "iscc failed" }

Write-Host "==> generating rekaptr-installer.ps1"
$Template = Get-Content "scripts\rekaptr-installer.ps1.template" -Raw
$Rendered = $Template.Replace("{{VERSION}}", $Version)
Set-Content -Path "target\installer\rekaptr-installer.ps1" -Value $Rendered -NoNewline

Write-Host ""
Write-Host "Release artifacts ready in target\installer\:"
Get-ChildItem "target\installer" | Select-Object Name, @{N='Size';E={'{0:N1} MB' -f ($_.Length/1MB)}}
