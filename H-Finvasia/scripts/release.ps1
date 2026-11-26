# Release task for H-Finvasia.
#
# Same shape as h-compare's release task: run the quality gates, commit source,
# build the release binaries, publish them under releases/ with BUILD.txt +
# SHA256SUMS.txt + README.md, commit the release artifacts and push.
#
# Two artifacts are produced:
#   h-finvasia-v<tag>.exe      <- desktop app (double-click this)
#   h-finvasia-cli-v<tag>.exe  <- headless harness
#
# Deviation from the sibling tools on purpose: artifacts go to `releases/`
# (H-Compare and H-Remote use `publish/`). Keep this folder committed.
#
# Usage:
#   .\scripts\release.ps1
#   .\scripts\release.ps1 -Message "feat(h-finvasia): ..."
#   .\scripts\release.ps1 -SkipTests -NoPush
#   .\scripts\release.ps1 -IncludeUntracked     # first release / new files
[CmdletBinding()]
param(
    [string]$Version = "",
    [string]$Message = "",
    [switch]$SkipTests,
    [switch]$IncludeUntracked,
    [switch]$NoPush
)

$ErrorActionPreference = "Stop"

$toolName = "h-finvasia"
$folder = "H-Finvasia"

function Invoke-Step {
    param([string]$Label, [scriptblock]$Action)
    Write-Host "==> $Label"
    & $Action
    if ($LASTEXITCODE -ne 0) { throw "$Label failed with exit code $LASTEXITCODE." }
}

function Get-BuildDate {
    param([string]$Exe)
    $versionText = (& $Exe --version).Trim()
    $match = [regex]::Match($versionText, 'build (\d{4}-\d{2}-\d{2})')
    if (-not $match.Success) {
        throw "Could not read the build date from '$versionText'."
    }
    return @{
        Text = $versionText
        Date = $match.Groups[1].Value
    }
}

function Get-NextVersion {
    param([string]$ReleaseDir, [string]$Stamp)
    $next = 1
    if (Test-Path $ReleaseDir) {
        # Matches the desktop name only; `h-finvasia-cli-v...` does not collide.
        $pattern = "^$([regex]::Escape($toolName))-v$([regex]::Escape($Stamp))\.(\d+)\.exe$"
        foreach ($file in Get-ChildItem $ReleaseDir -Filter "$toolName-v$Stamp.*.exe") {
            $match = [regex]::Match($file.Name, $pattern)
            if ($match.Success) {
                $candidate = [int]$match.Groups[1].Value + 1
                if ($candidate -gt $next) { $next = $candidate }
            }
        }
    }
    return "v$Stamp.$next"
}

$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $repoRoot = (& git rev-parse --show-toplevel).Trim()
    if ([string]::IsNullOrWhiteSpace($repoRoot)) {
        throw "Not inside a git repository."
    }
    $releases = Join-Path $root "releases"

    # Windows keeps the binaries locked while they are running.
    Get-Process "h-finvasia", "h-finvasia-desktop" -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 300

    if (-not $SkipTests) {
        if (-not (Test-Path (Join-Path $root "node_modules"))) {
            throw "node_modules is missing. Run 'npm install' before releasing."
        }
        Invoke-Step "npm run check" { npm run check }
        Invoke-Step "npm test" { npm test }
        Invoke-Step "cargo fmt --all -- --check" { cargo fmt --all -- --check }
        Invoke-Step "cargo clippy --workspace" { cargo clippy --workspace --all-targets -- -D warnings }
        Invoke-Step "cargo test --workspace" { cargo test --workspace }
    } else {
        Write-Host "==> quality gates skipped (-SkipTests)"
    }

    # The desktop build embeds ui/dist at compile time, so this is required even
    # when tests are skipped.
    Invoke-Step "npm run build:ui" { npm run build:ui }

    Push-Location (Join-Path $root "src-tauri")
    try {
        if (-not $SkipTests) {
            Invoke-Step "cargo fmt (src-tauri)" { cargo fmt -- --check }
            Invoke-Step "cargo clippy (src-tauri)" { cargo clippy --all-targets -- -D warnings }
        }
    } finally {
        Pop-Location
    }

    # Commit source before building, so BUILD.txt names a real, clean commit.
    $untracked = @(
        & git -C $repoRoot ls-files --others --exclude-standard -- $folder |
            Where-Object { $_ -notmatch "^$folder/releases/" }
    )
    if ($untracked.Count -gt 0 -and -not $IncludeUntracked) {
        throw "Untracked $folder files would be skipped or accidentally committed: $($untracked -join ', '). Stage them with 'git add' first, remove them, or pass -IncludeUntracked."
    }

    Write-Host "==> commit source changes (release artifacts excluded)"
    if ($IncludeUntracked) {
        & git -C $repoRoot add $folder
    } else {
        & git -C $repoRoot add -u $folder
    }
    if ($LASTEXITCODE -ne 0) { throw "git add failed." }
    & git -C $repoRoot reset --quiet -- "$folder/releases"
    if ($LASTEXITCODE -ne 0) { throw "git reset for releases failed." }
    & git -C $repoRoot diff --cached --quiet
    if ($LASTEXITCODE -ne 0) {
        $sourceMessage = if ([string]::IsNullOrWhiteSpace($Message)) {
            "chore(h-finvasia): prepare release"
        } else {
            $Message
        }
        & git -C $repoRoot commit -m $sourceMessage
        if ($LASTEXITCODE -ne 0) { throw "Source commit failed." }
    } else {
        Write-Host "No source changes to commit."
    }

    Invoke-Step "cargo build --release (workspace)" { cargo build --release --workspace }

    Push-Location (Join-Path $root "src-tauri")
    try {
        Invoke-Step "cargo build --release (desktop)" { cargo build --release }
    } finally {
        Pop-Location
    }

    $cliExe = Join-Path $root "target\release\$toolName.exe"
    $guiExe = Join-Path $root "src-tauri\target\release\$toolName-desktop.exe"
    if (-not (Test-Path $cliExe)) { throw "CLI binary not found at $cliExe." }
    if (-not (Test-Path $guiExe)) { throw "Desktop binary not found at $guiExe." }

    $build = Get-BuildDate $cliExe
    $stamp = $build.Date.Substring(2).Replace("-", ".")

    if ([string]::IsNullOrWhiteSpace($Version)) {
        $Version = Get-NextVersion $releases $stamp
    } elseif (-not $Version.StartsWith("v")) {
        $Version = "v$Version"
    }

    New-Item -ItemType Directory -Force -Path $releases | Out-Null
    $guiArtifact = "$toolName-$Version.exe"
    $cliArtifact = "$toolName-cli-$Version.exe"
    $guiPath = Join-Path $releases $guiArtifact
    $cliPath = Join-Path $releases $cliArtifact
    Copy-Item $guiExe $guiPath -Force
    Copy-Item $cliExe $cliPath -Force

    $guiHash = (Get-FileHash $guiPath -Algorithm SHA256).Hash.ToLower()
    $guiSize = (Get-Item $guiPath).Length
    $cliHash = (Get-FileHash $cliPath -Algorithm SHA256).Hash.ToLower()
    $cliSize = (Get-Item $cliPath).Length

    # Keep checksums for every published executable.
    Get-ChildItem $releases -Filter "*.exe" |
        Sort-Object Name |
        ForEach-Object {
            "$((Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower())  $($_.Name)"
        } |
        Set-Content -Encoding ascii (Join-Path $releases "SHA256SUMS.txt")

    $packageMatch = [regex]::Match($build.Text, '^h-finvasia (\S+)')
    $packageVersion = if ($packageMatch.Success) { $packageMatch.Groups[1].Value } else { "0.0.0" }
    $built = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffZ")
    $commit = (& git -C $repoRoot rev-parse --short HEAD).Trim()
    $rustc = (& rustc --version).Trim()
    $cargo = (& cargo --version).Trim()
    $node = (& node --version).Trim()
    $buildVersion = $Version.TrimStart("v")
    @"
H-Finvasia release build
tag: $Version
version: $packageVersion
buildVersion: $buildVersion
commit: $commit
dirty: false
built: $built
rustc: $rustc
cargo: $cargo
node: $node
signed: no
target: $([System.Runtime.InteropServices.RuntimeInformation]::OSDescription.Trim()) / $([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture)

artifacts:
$guiHash $guiSize $guiArtifact (desktop app)
$cliHash $cliSize $cliArtifact (headless CLI)
"@ | Set-Content -Encoding ascii (Join-Path $releases "BUILD.txt")

    $readme = @"
# Released builds

Committed, ready-to-run builds for H-Finvasia. ``BUILD.txt`` records the source
commit, toolchain and build time for the current build, and ``SHA256SUMS.txt``
covers every executable in this folder.

## Current

| File | Kind | Size | SHA-256 |
| --- | --- | ---: | --- |
| ``$guiArtifact`` | **desktop app** (double-click) | $guiSize | ``$guiHash`` |
| ``$cliArtifact`` | headless CLI | $cliSize | ``$cliHash`` |

## Running the desktop app

Double-click ``$guiArtifact``. It loads the instrument master from the local
MongoDB (``STOCK.FINVASIA_SYMBOL``) and falls back to a bundled sample when Mongo
is unavailable. Press **Demo feed** to see live-updating quotes without
credentials.

## Running the CLI

``````powershell
.\$cliArtifact doctor
.\$cliArtifact master stats
``````

``master`` needs a local MongoDB (``FV_MONGO_URI``, default
``mongodb://127.0.0.1:27017``); ``doctor``, ``replay`` and ``bench`` do not.

## A note on older files

``h-finvasia-v*.exe`` builds **before** ``$Version`` were CLI-only (the UI did not
exist yet). From ``$Version`` the unqualified name is the desktop app and the CLI
is suffixed ``-cli``.

## Adding a build

``````powershell
.\scripts\release.ps1
``````
"@
    Set-Content -Encoding ascii (Join-Path $releases "README.md") $readme

    Write-Host "==> commit and push release artifacts"
    & git -C $repoRoot add "$folder/releases"
    if ($LASTEXITCODE -ne 0) { throw "git add for releases failed." }
    & git -C $repoRoot diff --cached --quiet
    if ($LASTEXITCODE -ne 0) {
        & git -C $repoRoot commit -m "release(h-finvasia): publish $Version"
        if ($LASTEXITCODE -ne 0) { throw "Release commit failed." }
    } else {
        Write-Host "No release artifact changes to commit."
    }

    if (-not $NoPush) {
        & git -C $repoRoot push
        if ($LASTEXITCODE -ne 0) { throw "git push failed." }
    }

    Write-Host ""
    Write-Host "Published $guiArtifact   (desktop)"
    Write-Host "SHA256   $guiHash"
    Write-Host "Published $cliArtifact   (CLI)"
    Write-Host "SHA256   $cliHash"
    Write-Host "Build    $($build.Text)"
    Write-Host "Folder   $releases"
} finally {
    Pop-Location
}
