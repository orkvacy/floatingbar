<#
.SYNOPSIS
  Optional: let FloatingBar be the only place notifications pop up.

.DESCRIPTION
  For every app that has sent notifications before, turns off Windows'
  pop-up banners (ShowBanner = 0) while keeping notifications in the
  notification center, which is where FloatingBar reads them from. It also
  turns off flashing taskbar buttons, so an auto-hidden taskbar stays hidden.

  Only your user settings (HKCU) are changed; no admin rights are needed.
  Previous values are saved, and -Undo puts them back.

  Apps that send their first notification later still get banners; run the
  script again to quiet them too. Do NOT turn notifications off completely in
  Windows Settings, or FloatingBar receives nothing either.

.PARAMETER Undo
  Restore the values saved by the last run.

.PARAMETER KeepTaskbarFlashing
  Leave the taskbar flashing setting alone.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\quiet-windows-notifications.ps1 -WhatIf
  Shows what would change without changing anything.

.EXAMPLE
  powershell -ExecutionPolicy Bypass -File scripts\quiet-windows-notifications.ps1
  powershell -ExecutionPolicy Bypass -File scripts\quiet-windows-notifications.ps1 -Undo
#>
[CmdletBinding(SupportsShouldProcess)]
param(
  [switch]$Undo,
  [switch]$KeepTaskbarFlashing
)

$ErrorActionPreference = "Stop"
$notifications = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Notifications\Settings"
$explorer = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced"
# "Show flashing on taskbar apps" (Windows 11 22H2 and later).
$flashValue = "TaskbarFlashing"
$backupFile = Join-Path $env:APPDATA "id.orkvacy.floatingbar\quiet-notifications-backup.json"

# Current value of a registry DWORD, or $null when it isn't set (Windows default).
function Get-Dword([string]$Path, [string]$Name) {
  $item = Get-ItemProperty -Path $Path -Name $Name -ErrorAction SilentlyContinue
  if ($item) { return [int]$item.$Name }
  return $null
}

# Set a DWORD, or remove it to go back to the Windows default.
function Set-Dword([string]$Path, [string]$Name, $Value) {
  if ($null -eq $Value) {
    Remove-ItemProperty -Path $Path -Name $Name -ErrorAction SilentlyContinue
  } else {
    New-ItemProperty -Path $Path -Name $Name -Value $Value -PropertyType DWord -Force | Out-Null
  }
}

if ($Undo) {
  if (-not (Test-Path $backupFile)) {
    Write-Host "No backup found at $backupFile; nothing to undo."
    exit 0
  }
  $backup = Get-Content $backupFile -Raw | ConvertFrom-Json
  $restored = 0
  foreach ($app in $backup.banners.PSObject.Properties) {
    $path = Join-Path $notifications $app.Name
    if ((Test-Path -LiteralPath $path) -and $PSCmdlet.ShouldProcess($app.Name, "Restore banners")) {
      Set-Dword $path "ShowBanner" $app.Value
      $restored++
    }
  }
  if ($backup.PSObject.Properties.Name -contains "taskbarFlashing" -and
      $PSCmdlet.ShouldProcess("Taskbar", "Restore flashing")) {
    Set-Dword $explorer $flashValue $backup.taskbarFlashing
  }
  if (-not $WhatIfPreference) { Remove-Item $backupFile }
  Write-Host "Restored banners for $restored apps."
  exit 0
}

# Remember what each setting was before this run, so -Undo is exact.
$banners = [ordered]@{}
$changed = 0
foreach ($key in Get-ChildItem -LiteralPath $notifications) {
  $path = $key.PSPath
  # Apps whose notifications are switched off entirely stay as they are.
  if ((Get-Dword $path "Enabled") -eq 0) { continue }
  $before = Get-Dword $path "ShowBanner"
  if ($before -eq 0) { continue }
  if ($PSCmdlet.ShouldProcess($key.PSChildName, "Turn off pop-up banners (keep in notification center)")) {
    $banners[$key.PSChildName] = $before
    Set-Dword $path "ShowBanner" 0
    $changed++
  }
}

$backup = [ordered]@{ banners = $banners }
if (-not $KeepTaskbarFlashing) {
  $flashBefore = Get-Dword $explorer $flashValue
  if ($flashBefore -ne 0 -and $PSCmdlet.ShouldProcess("Taskbar", "Turn off flashing taskbar buttons")) {
    $backup.taskbarFlashing = $flashBefore
    Set-Dword $explorer $flashValue 0
  }
}

if (-not $WhatIfPreference) {
  # Merge with an earlier run so -Undo still knows the original values.
  if (Test-Path $backupFile) {
    $old = Get-Content $backupFile -Raw | ConvertFrom-Json
    foreach ($app in $old.banners.PSObject.Properties) { $backup.banners[$app.Name] = $app.Value }
    if ($old.PSObject.Properties.Name -contains "taskbarFlashing") { $backup.taskbarFlashing = $old.taskbarFlashing }
  }
  New-Item -ItemType Directory -Force (Split-Path $backupFile) | Out-Null
  $backup | ConvertTo-Json -Depth 3 | Set-Content $backupFile -Encoding UTF8
  Write-Host "Turned off pop-up banners for $changed apps. Notifications still reach the notification center and FloatingBar."
  Write-Host "Backup saved to $backupFile (run with -Undo to restore)."
  Write-Host "If banners still appear, sign out and back in so Windows reloads the settings."
}
