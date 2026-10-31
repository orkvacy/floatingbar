<#
.SYNOPSIS
  Captures README screenshots from the dev server's demo states with headless Edge.

.DESCRIPTION
  Needs the dev server running (bun run dev or bun run tauri dev). Writes PNGs to
  docs/screenshots. Uses a throwaway Edge profile, so your own browser is untouched.
#>
param([string]$Url = "http://localhost:1420/")

$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
$out = Join-Path $root "docs\screenshots"
New-Item -ItemType Directory -Force $out | Out-Null
(Get-Item (Join-Path $root "src\lib\native.ts")).LastWriteTime = Get-Date  # make Vite serve fresh files

$edge = @("${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe",
          "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
$profileDir = Join-Path $env:TEMP "floatingbar-shots-profile"

# name = demo state, height of the shot in CSS px
$shots = [ordered]@{ permission = 240; claude = 480; discord = 540; 'discord-compact' = 100; todos = 460; calendar = 680; compact = 100; music = 300; notifications = 340; notification = 180 }
# Edge reports "bytes written" on stderr, which Windows PowerShell treats as an error.
$ErrorActionPreference = "Continue"
foreach ($name in $shots.Keys) {
  $file = Join-Path $out "$name.png"
  & $edge --headless=new --disable-gpu --hide-scrollbars --no-first-run --user-data-dir="$profileDir" `
    --force-device-scale-factor=2 --window-size="720,$($shots[$name])" --virtual-time-budget=5000 `
    --screenshot="$file" "$($Url)?demo=$name&capture=1" 2>$null | Out-Null
  Write-Host ("{0,-14} {1}" -f $name, $(if (Test-Path $file) { "ok" } else { "FAILED" }))
}
