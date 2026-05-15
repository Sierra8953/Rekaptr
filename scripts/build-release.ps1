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
