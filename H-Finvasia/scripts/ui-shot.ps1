# Capture the current UI to a PNG without touching the broker.
#
# Starts the release app with auto-connect disabled, drives the DevTools protocol,
# screenshots the window and stops the app.
#
# Usage:
#   .\scripts\ui-shot.ps1                                  # workspace (Index tab)
#   .\scripts\ui-shot.ps1 -Page "Option chain" -Out chain.png
[CmdletBinding()]
param(
    [string]$Out = "ui-shot.png",
    [string]$Page = "",
    [string]$Action = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
$exe = Join-Path $root "src-tauri\target\release\h-finvasia-desktop.exe"
if (-not (Test-Path $exe)) {
    throw "Release build missing at $exe. Run 'npm run tauri:build' first."
}

Add-Type @"
using System;
using System.Runtime.InteropServices;
public class FvWin {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
}
"@

# Never let a screenshot open a live socket.
$env:FV_NO_AUTOCONNECT = "1"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9222"

Get-Process "h-finvasia-desktop", "msedgewebview2" -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 900

$process = Start-Process -FilePath $exe -PassThru
try {
    # Wait for the DevTools endpoint instead of guessing.
    $ready = $false
    for ($i = 0; $i -lt 45; $i++) {
        try {
            $null = Invoke-RestMethod "http://127.0.0.1:9222/json" -TimeoutSec 2
            $ready = $true
            break
        } catch { Start-Sleep -Seconds 1 }
    }
    if (-not $ready) { throw "DevTools endpoint never came up" }

    # WebView2 does not paint while occluded, so bring the window forward.
    $live = Get-Process -Id $process.Id -ErrorAction SilentlyContinue
    if ($live -and $live.MainWindowHandle -ne 0) {
        [FvWin]::ShowWindow($live.MainWindowHandle, 9) | Out-Null   # SW_RESTORE
        [FvWin]::SetForegroundWindow($live.MainWindowHandle) | Out-Null
    }
    Start-Sleep -Seconds 2

    $args = @((Join-Path $PSScriptRoot "ui-shot.mjs"), $Out)
    if ($Page -ne "") { $args += $Page }
    if ($Action -ne "") { $args += $Action }
    & node @args
    if ($LASTEXITCODE -ne 0) { throw "screenshot failed" }
    Write-Host "Screenshot: $(Join-Path (Get-Location) $Out)"
}
finally {
    if (-not $process.HasExited) { $process.Kill() }
    Remove-Item Env:\FV_NO_AUTOCONNECT -ErrorAction SilentlyContinue
    Remove-Item Env:\WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS -ErrorAction SilentlyContinue
}
