# Release task for H-Finvasia.
#
# Same shape as h-compare's release task: run the quality gates, commit source,
# build the release binary, publish the portable executable under releases/ with
# BUILD.txt + SHA256SUMS.txt + README.md, commit the release artifacts and push.
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
    param(
        [string]$ReleaseDir,
        [string]$Stamp
    )
    $next = 1
    if (Test-Path $ReleaseDir) {
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

    # Windows keeps the release binary locked while it is running.
    Get-Process $toolName -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 300

    if (-not $SkipTests) {
        Write-Host "==> cargo fmt --all -- --check"
        cargo fmt --all -- --check
        if ($LASTEXITCODE -ne 0) { throw "Formatting check failed." }

        Write-Host "==> cargo clippy --workspace --all-targets"
        cargo clippy --workspace --all-targets -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw "Clippy failed." }

        Write-Host "==> cargo test --workspace"
        cargo test --workspace
        if ($LASTEXITCODE -ne 0) { throw "Tests failed." }
    } else {
        Write-Host "==> quality gates skipped (-SkipTests)"
    }

    # Commit source before building, so BUILD.txt names a real, clean commit and
    # the shipped binary corresponds to committed code.
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

    Write-Host "==> cargo build --release"
    cargo build --release --workspace
    if ($LASTEXITCODE -ne 0) { throw "Release build failed." }

    $exe = Join-Path $root "target\release\$toolName.exe"
    if (-not (Test-Path $exe)) { throw "Release binary not found at $exe." }
    $build = Get-BuildDate $exe
    $stamp = $build.Date.Substring(2).Replace("-", ".")

    if ([string]::IsNullOrWhiteSpace($Version)) {
        $Version = Get-NextVersion $releases $stamp
    } elseif (-not $Version.StartsWith("v")) {
        $Version = "v$Version"
    }

    New-Item -ItemType Directory -Force -Path $releases | Out-Null
    $artifact = "$toolName-$Version.exe"
    $artifactPath = Join-Path $releases $artifact
    Copy-Item $exe $artifactPath -Force

    $hash = (Get-FileHash $artifactPath -Algorithm SHA256).Hash.ToLower()
    $size = (Get-Item $artifactPath).Length

    # Keep checksums for every published portable executable.
    Get-ChildItem $releases -Filter "$toolName-v*.exe" |
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
signed: no
target: $([System.Runtime.InteropServices.RuntimeInformation]::OSDescription.Trim()) / $([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture)

artifacts:
$hash $size $artifact
"@ | Set-Content -Encoding ascii (Join-Path $releases "BUILD.txt")

    $readmeHeader = @"
# Released builds

Committed, ready-to-run portable executables for H-Finvasia. ``BUILD.txt`` records
the source commit, toolchain and build time for the current build, and
``SHA256SUMS.txt`` covers every executable in this folder.

## Current

| File | Size | SHA-256 |
| --- | ---: | --- |
| ``$artifact`` | $size | ``$hash`` |

## Running

``````powershell
.\$artifact doctor
.\$artifact master stats
``````

Requires a local MongoDB (``FV_MONGO_URI``, default ``mongodb://127.0.0.1:27017``)
for the ``master`` commands; ``doctor``, ``replay`` and ``bench`` work without it.

## Adding a build

``````powershell
.\scripts\release.ps1
``````

Old executables are intentionally kept until the maintainer removes them.
"@
    Set-Content -Encoding ascii (Join-Path $releases "README.md") $readmeHeader

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
    Write-Host "Published $artifact"
    Write-Host "SHA256   $hash"
    Write-Host "Size     $size bytes"
    Write-Host "Build    $($build.Text)"
    Write-Host "Folder   $releases"
} finally {
    Pop-Location
}
