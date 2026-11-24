# Release task for H-Remote.
#
# Runs the quality gates, commits source changes, builds the release bundles,
# publishes the portable executable under publish/ (BUILD.txt + SHA256SUMS.txt),
# commits the release artifacts and pushes. Use it after every completed feature.
#
# Usage:
#   .\scripts\release.ps1
#   .\scripts\release.ps1 -Message "feat(h-remote): ..." -Bundles nsis
#   .\scripts\release.ps1 -SkipTests -NoPush
[CmdletBinding()]
param(
    [ValidateSet("nsis", "msi", "all", "none")]
    [string]$Bundles = "nsis",
    [string]$Message = "",
    [switch]$SkipTests,
    [switch]$NoPush,
    [string]$Date = (Get-Date -Format "yy.MM.dd"),
    [string]$SignCertificate = $env:HREMOTE_SIGN_CERT,
    [string]$SignPassword = $env:HREMOTE_SIGN_PASSWORD,
    [string]$TimestampUrl = $(if ($env:HREMOTE_TIMESTAMP_URL) { $env:HREMOTE_TIMESTAMP_URL } else { "http://timestamp.digicert.com" })
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$publish = Join-Path $root "publish"

function Invoke-Step {
    param(
        [string]$Label,
        [scriptblock]$Action
    )
    Write-Host "==> $Label"
    & $Action
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed with exit code $LASTEXITCODE."
    }
}

function Update-PublishReadme {
    param(
        [string]$PublishDir,
        [string]$Artifact,
        [string]$Hash
    )
    $file = Join-Path $PublishDir "README.md"
    $header = @"
# Published builds

Committed, ready-to-run portable executables live here so they survive across
workspaces and machines. ``release/`` remains the ignored per-tag audit folder
(installer, metadata, hashes); only the portable exe and its checksum are
committed.

## Current

| File | SHA-256 |
| --- | --- |
| ``$Artifact`` | ``$Hash`` |

"@
    $footer = @"

``BUILD.txt`` records the source commit, toolchain and build time.

## Adding a build

``````powershell
.\scripts\release.ps1
``````

Old executables are intentionally kept until the maintainer removes them.
"@
    Set-Content -Encoding ascii $file ($header + $footer)
}

Push-Location $root
try {
    $repoRoot = (& git rev-parse --show-toplevel).Trim()
    if ([string]::IsNullOrWhiteSpace($repoRoot)) {
        throw "Not inside a git repository."
    }

    # Windows keeps the release binary locked while the app is running.
    Get-Process hremote -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 300

    if (-not $SkipTests) {
        Invoke-Step "cargo fmt --check" { cargo fmt --all -- --check }
        Invoke-Step "cargo clippy" { cargo clippy --workspace --all-targets -- -D warnings }
        Invoke-Step "cargo test" { cargo test --workspace }
        Invoke-Step "npm run verify" { npm run verify }
    } else {
        Write-Host "==> quality gates skipped (-SkipTests)"
    }

    # Commit source before building so BUILD.txt names a real, clean commit.
    Write-Host "==> commit source changes (publish artifacts excluded)"
    & git -C $repoRoot add H-Remote
    if ($LASTEXITCODE -ne 0) { throw "git add failed." }
    & git -C $repoRoot reset --quiet -- H-Remote/publish
    if ($LASTEXITCODE -ne 0) { throw "git reset for publish failed." }
    & git -C $repoRoot diff --cached --quiet
    if ($LASTEXITCODE -ne 0) {
        $sourceMessage = if ([string]::IsNullOrWhiteSpace($Message)) {
            "chore(h-remote): prepare release"
        } else {
            $Message
        }
        & git -C $repoRoot commit -m $sourceMessage
        if ($LASTEXITCODE -ne 0) { throw "Source commit failed." }
    } else {
        Write-Host "No source changes to commit."
    }

    # Reserve the next tag across both the audit folder and committed publish/.
    New-Item -ItemType Directory -Force (Join-Path $root "release") | Out-Null
    $sequence = 1
    while (
        (Test-Path (Join-Path $root "release\h-remote-v$Date.$sequence.exe")) -or
        (Test-Path (Join-Path $root "release\v$Date.$sequence")) -or
        (Test-Path (Join-Path $publish "h-remote-v$Date.$sequence.exe"))
    ) {
        $sequence += 1
    }
    $tag = "v$Date.$sequence"
    $buildVersion = $tag.TrimStart("v")
    $env:HREMOTE_BUILD_VERSION = $buildVersion

    if ($Bundles -eq "none") {
        Invoke-Step "tauri build (no bundle)" { cmd /c "npm run tauri:build -- --no-bundle" }
    } else {
        Invoke-Step "tauri build ($Bundles)" { cmd /c "npm run tauri:build -- --bundles $Bundles" }
    }

    $exe = Join-Path $root "target\release\hremote.exe"
    if (-not (Test-Path $exe)) {
        throw "Missing $exe."
    }

    $tagDir = Join-Path $root "release\$tag"
    New-Item -ItemType Directory -Force $tagDir | Out-Null
    $portableName = "h-remote-$tag.exe"
    $portable = Join-Path $tagDir $portableName
    Copy-Item $exe $portable

    $installer = $null
    if ($Bundles -ne "none") {
        $setup = Get-ChildItem (Join-Path $root "target\release\bundle") -Recurse -Filter "*-setup.exe" -ErrorAction SilentlyContinue |
            Sort-Object LastWriteTime -Descending |
            Select-Object -First 1
        if (-not $setup) {
            $setup = Get-ChildItem (Join-Path $root "target\release\bundle") -Recurse -Filter "*.msi" -ErrorAction SilentlyContinue |
                Sort-Object LastWriteTime -Descending |
                Select-Object -First 1
        }
        if ($setup) {
            $installer = "h-remote-$tag-setup$($setup.Extension)"
            Copy-Item $setup.FullName (Join-Path $tagDir $installer)
        }
    }

    $signed = "no"
    if ($SignCertificate) {
        if (-not (Get-Command signtool -ErrorAction SilentlyContinue)) {
            throw "signtool is required to sign artifacts. Install the Windows SDK or unset HREMOTE_SIGN_CERT."
        }
        $toSign = Get-ChildItem $tagDir -File | Where-Object { $_.Extension -in ".exe", ".msi" }
        foreach ($file in $toSign) {
            & signtool sign /f $SignCertificate /p $SignPassword /fd SHA256 /tr $TimestampUrl /td SHA256 $file.FullName
            if ($LASTEXITCODE -ne 0) { throw "signtool failed for $($file.Name)" }
            & signtool verify /pa $file.FullName
            if ($LASTEXITCODE -ne 0) { throw "signature verification failed for $($file.Name)" }
        }
        $signed = "yes"
    }

    Copy-Item (Join-Path $tagDir "*.exe") (Join-Path $root "release")
    Copy-Item (Join-Path $tagDir "*.msi") (Join-Path $root "release") -ErrorAction SilentlyContinue

    Invoke-Step "build-info" { node scripts/build-info.mjs --tag $tag --dir $tagDir --signed $signed }

    $metadata = @(
        "H-Remote release",
        "app version: 0.1.0",
        "build version: $buildVersion",
        "tag: $tag",
        "commit: $((& git -C $repoRoot rev-parse --short HEAD).Trim())",
        "dirty: false",
        "built: $((Get-Date).ToString('o'))",
        "signed: $signed",
        "portable: $portableName",
        $(if ($installer) { "installer: $installer" }),
        "build info: release\$tag\BUILD.txt"
    )
    $metadata | Set-Content (Join-Path $root "release\RELEASE-$Date.$sequence.txt")

    Get-ChildItem (Join-Path $root "release") -Filter "h-remote-$tag*" -File | ForEach-Object {
        "$((Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower())  $($_.Name)"
    } | Set-Content (Join-Path $root "release\SHA256SUMS-$tag.txt")

    Write-Host "==> publish portable executable"
    New-Item -ItemType Directory -Force $publish | Out-Null
    Copy-Item $portable (Join-Path $publish $portableName) -Force
    Copy-Item (Join-Path $tagDir "BUILD.txt") (Join-Path $publish "BUILD.txt") -Force

    # Keep checksums for every published portable executable.
    Get-ChildItem $publish -Filter "h-remote-v*.exe" |
        Sort-Object Name |
        ForEach-Object {
            "$((Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower())  $($_.Name)"
        } |
        Set-Content -Encoding ascii (Join-Path $publish "SHA256SUMS.txt")

    $publishedHash = (Get-FileHash (Join-Path $publish $portableName) -Algorithm SHA256).Hash.ToLower()
    Update-PublishReadme -PublishDir $publish -Artifact $portableName -Hash $publishedHash

    Write-Host "==> commit and push release artifacts"
    & git -C $repoRoot add H-Remote/publish
    if ($LASTEXITCODE -ne 0) { throw "git add for publish failed." }
    & git -C $repoRoot diff --cached --quiet
    if ($LASTEXITCODE -ne 0) {
        & git -C $repoRoot commit -m "release(h-remote): publish $tag"
        if ($LASTEXITCODE -ne 0) { throw "Release commit failed." }
    }

    if (-not $NoPush) {
        & git -C $repoRoot push
        if ($LASTEXITCODE -ne 0) { throw "git push failed." }
    }

    Write-Host ""
    Write-Host "Published $portableName"
    Write-Host "SHA256   $publishedHash"
    Write-Host "Tag      $tag"
    Write-Host "Audit    $tagDir\BUILD.txt"
}
finally {
    Pop-Location
}
