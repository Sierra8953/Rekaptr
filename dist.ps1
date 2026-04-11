# Luma Portable Distribution Script
# This script builds Luma and bundles it with all dependencies (FFmpeg, MPV, GStreamer)
# into a self-contained portable folder.

$ProjectRoot = Get-Location
$DistDir = Join-Path $ProjectRoot "dist"
$GstVersion = "1.24.1" # Change this to your preferred GStreamer version
$GstUrl = "https://gstreamer.freedesktop.org/data/pkg/windows/$GstVersion/msvc/gstreamer-1.0-msvc-x86_64-$GstVersion.zip"
$GstCache = Join-Path $ProjectRoot "deps\cache\gstreamer-$GstVersion.zip"

Write-Host "--- Starting Luma Portable Build ---" -ForegroundColor Cyan

# 1. Build the App
Write-Host "[1/5] Building Luma in Release mode..." -ForegroundColor Yellow
cargo build --release
if ($LASTEXITCODE -ne 0) { Write-Error "Cargo build failed!"; exit }

# 2. Prepare Dist Folder
Write-Host "[2/5] Preparing distribution folder..." -ForegroundColor Yellow
if (Test-Path $DistDir) { Remove-Item -Recurse -Force $DistDir }
New-Item -ItemType Directory -Path $DistDir | Out-Null
New-Item -ItemType Directory -Path (Join-Path $DistDir "bin") | Out-Null
New-Item -ItemType Directory -Path (Join-Path $DistDir "Recordings") | Out-Null

# 3. Handle GStreamer (The "Smart" Part)
$GstDest = Join-Path $DistDir "gstreamer"
if (-not (Test-Path (Join-Path $ProjectRoot "deps\cache"))) { New-Item -ItemType Directory -Path (Join-Path $ProjectRoot "deps\cache") | Out-Null }

if (-not (Test-Path $GstCache)) {
    Write-Host "[3/5] GStreamer not found in cache. Downloading $GstVersion..." -ForegroundColor Cyan
    Invoke-WebRequest -Uri $GstUrl -OutFile $GstCache
} else {
    Write-Host "[3/5] Using cached GStreamer $GstVersion." -ForegroundColor Green
}

Write-Host "      Extracting GStreamer to dist folder (this may take a minute)..." -ForegroundColor Gray
Expand-Archive -Path $GstCache -DestinationPath $GstDest -Force

# 4. Copy Local Binaries (FFmpeg, MPV)
Write-Host "[4/5] Bundling FFmpeg and MPV..." -ForegroundColor Yellow
$FoundFFmpeg = $false
$FoundMPV = $false

# Search for FFmpeg
foreach ($loc in @("bin\ffmpeg.exe", "ffmpeg.exe", "deps\ffmpeg.exe")) {
    if (Test-Path (Join-Path $ProjectRoot $loc)) {
        Copy-Item (Join-Path $ProjectRoot $loc) (Join-Path $DistDir "bin\ffmpeg.exe")
        $FoundFFmpeg = $true
        break
    }
}

# Search for MPV
foreach ($loc in @("bin\mpv-2.dll", "mpv-2.dll", "deps\libmpv\mpv-2.dll")) {
    if (Test-Path (Join-Path $ProjectRoot $loc)) {
        Copy-Item (Join-Path $ProjectRoot $loc) (Join-Path $DistDir "bin\mpv-2.dll")
        $FoundMPV = $true
        break
    }
}

if (-not $FoundFFmpeg) { Write-Warning "FFmpeg not found! Clipping will not work in the portable build." }
if (-not $FoundMPV) { Write-Warning "mpv-2.dll not found! Playback will not work in the portable build." }

# 5. Copy App & Assets
Write-Host "[5/5] Finalizing bundle..." -ForegroundColor Yellow
Copy-Item (Join-Path $ProjectRoot "target\release\luma.exe") (Join-Path $DistDir "luma.exe")
Copy-Item -Recurse (Join-Path $ProjectRoot "assets") (Join-Path $DistDir "assets")

Write-Host "`n--- Build Complete! ---" -ForegroundColor Green
Write-Host "Portable release is ready in: $DistDir"
Write-Host "You can now zip the 'dist' folder and share it."
