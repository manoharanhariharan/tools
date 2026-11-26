# Capture the current UI to a PNG without touching the broker.
#
# Starts the release app with auto-connect disabled, drives the DevTools protocol,
# screenshots the window and stops the app.
#
# Usage:
#   .\scripts\ui-shot.ps1                                  # market watch
#   .\scripts\ui-shot.ps1 -Page "Option chain" -Out chain.png
[CmdletBinding()]
param(
    [string]$Out = "ui-shot.png",
    [string]$Page = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root "src-tauri\target\release\h-finvasia-desktop.exe"
if (-not (Test-Path $exe)) {
    throw "Release build missing at $exe. Run 'npm run tauri:build' first."
}

# Never let a screenshot open a live socket.
$env:FV_NO_AUTOCONNECT = "1"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"

Get-Process "h-finvasia-desktop", "msedgewebview2" -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 800

$process = Start-Process -FilePath $exe -PassThru
try {
    # Give WebView2 time to boot and the master to load.
    Start-Sleep -Seconds 8
    $args = @((Join-Path $PSScriptRoot "ui-shot.mjs"), $Out)
    if ($Page -ne "") { $args += $Page }
    & node @args
    if ($LASTEXITCODE -ne 0) { throw "screenshot failed" }
    Write-Host "Screenshot: $(Join-Path (Get-Location) $Out)"
}
finally {
    if (-not $process.HasExited) { $process.Kill() }
    Remove-Item Env:\FV_NO_AUTOCONNECT -ErrorAction SilentlyContinue
    Remove-Item Env:\WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
}
