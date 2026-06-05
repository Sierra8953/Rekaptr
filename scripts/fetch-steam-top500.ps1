# Regenerate the bundled popular-games snapshot used by src/game_catalog.rs.
#
# Pulls SteamSpy's top games (ranked by owners), keeps the top 500 as
# {appid, name}, and writes assets/steam_top500.json (UTF-8, no BOM, compact).
# Run occasionally to refresh the compiled-in snapshot; the app also refreshes a
# runtime cache copy from the same source (see game_catalog::refresh_if_stale).
#
# Usage: pwsh scripts/fetch-steam-top500.ps1

$ErrorActionPreference = "Stop"
$Root = (Resolve-Path "$PSScriptRoot\..").Path
$Dest = Join-Path $Root "assets\steam_top500.json"

Write-Host "Fetching SteamSpy top games..."
$resp = Invoke-RestMethod -Uri "https://steamspy.com/api.php?request=all&page=0" -TimeoutSec 60
$items = @($resp.PSObject.Properties | ForEach-Object { $_.Value })
Write-Host "Fetched $($items.Count) entries"

# Rank by the lower bound of the "owners" range (e.g. "100,000,000 .. 200,000,000").
$ranked = $items |
    Where-Object { $_.appid -and $_.name } |
    Sort-Object -Property @{ Expression = {
        $lower = ($_.owners -split '\.\.')[0]
        [int64](($lower -replace '[^0-9]','') )
    }} -Descending |
    Select-Object -First 500 |
    ForEach-Object { [pscustomobject]@{ appid = [int]$_.appid; name = $_.name } }

$json = $ranked | ConvertTo-Json -Compress
[System.IO.File]::WriteAllText($Dest, $json, (New-Object System.Text.UTF8Encoding($false)))

Write-Host "Wrote $($ranked.Count) games to $Dest ($([math]::Round((Get-Item $Dest).Length/1KB,1)) KB)"
Write-Host "Top 3: $(($ranked[0..2] | ForEach-Object { $_.name }) -join ', ')"
