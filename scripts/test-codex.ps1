<#
.SYNOPSIS
  Tests Codex transport, history rendering, and desktop provider rules.
.DESCRIPTION
  Uses a local stdio fixture. Does not contact a model, read private chats,
  or grant permissions. Run cargo build first to provide dependency libraries.
#>
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$deps = Join-Path $projectRoot 'src-tauri\target\debug\deps'
$out = Join-Path $projectRoot 'src-tauri\target\codex-tests'
New-Item -ItemType Directory -Force -Path $out | Out-Null
$oldVersion = $env:CARGO_PKG_VERSION
try {
  $env:CARGO_PKG_VERSION = (Get-Content -Raw (Join-Path $projectRoot 'package.json') | ConvertFrom-Json).version
  foreach ($module in @('codex_rpc', 'codex_history', 'permission_provider')) {
    $source = Join-Path $projectRoot "src-tauri\src\$module.rs"
    $binary = Join-Path $out "$module-tests.exe"
    $arguments = @('--edition=2021', '--test', $source, '-A', 'dead_code', '-o', $binary, '-L', "dependency=$deps")
    $externs = switch ($module) { 'codex_rpc' { @('serde_json') }; 'codex_history' { @('serde', 'serde_json') }; default { @() } }
    foreach ($dependency in $externs) {
      $library = Get-ChildItem -LiteralPath $deps -Filter "lib$dependency-*.rlib" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
      if (!$library) { throw "Missing $dependency library. Run cargo build --manifest-path src-tauri/Cargo.toml first." }
      $arguments += @('--extern', "$dependency=$($library.FullName)")
    }
    & rustc @arguments
    if ($LASTEXITCODE -ne 0) { throw "Compilation failed: $module" }
    & $binary
    if ($LASTEXITCODE -ne 0) { throw "Tests failed: $module" }
  }
} finally {
  $env:CARGO_PKG_VERSION = $oldVersion
}
