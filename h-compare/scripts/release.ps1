# Release task for H-Compare.
#
# Runs the quality gates, commits source changes, builds the release binary,
# publishes it under publish/ with BUILD.txt + SHA256SUMS.txt, commits the
# release artifacts and pushes.
#
# Usage:
#   .\scripts\release.ps1
#   .\scripts\release.ps1 -Version v26.11.24.3 -Message "fix(h-compare): ..."
#   .\scripts\release.ps1 -SkipTests -NoPush
[CmdletBinding()]
param(
    [string]$Version = "",
    [string]$Message = "",
    [switch]$SkipTests,
    [switch]$NoPush
)

$ErrorActionPreference = "Stop"

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
        [string]$PublishDir,
        [string]$Stamp
    )
    $next = 1
    if (Test-Path $PublishDir) {
        $pattern = "^hcompare-v$([regex]::Escape($Stamp))\.(\d+)\.exe$"
        foreach ($file in Get-ChildItem $PublishDir -Filter "hcompare-v$Stamp.*.exe") {
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
    $publish = Join-Path $root "publish"

    # Windows keeps the release binary locked while the app is running.
    Get-Process hcompare -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 300

    if (-not $SkipTests) {
        Write-Host "==> cargo fmt --check"
        cargo fmt -- --check
        if ($LASTEXITCODE -ne 0) { throw "Formatting check failed." }

        Write-Host "==> cargo test"
        cargo test
        if ($LASTEXITCODE -ne 0) { throw "Tests failed." }

        Write-Host "==> cargo clippy --all-targets"
        cargo clippy --all-targets -- -D warnings
        if ($LASTEXITCODE -ne 0) { throw "Clippy failed." }
    }

    # Commit the source before building, so the binary's embedded commit hash
    # names a real commit and BUILD.txt is not marked dirty for a normal release.
    Write-Host "==> commit source changes (publish artifacts excluded)"
    & git -C $repoRoot add h-compare
    if ($LASTEXITCODE -ne 0) { throw "git add failed." }
    & git -C $repoRoot reset --quiet -- h-compare/publish
    if ($LASTEXITCODE -ne 0) { throw "git reset for publish failed." }
    & git -C $repoRoot diff --cached --quiet
    if ($LASTEXITCODE -ne 0) {
        $sourceMessage = if ([string]::IsNullOrWhiteSpace($Message)) {
            "chore(h-compare): prepare release"
        } else {
            $Message
        }
        & git -C $repoRoot commit -m $sourceMessage
        if ($LASTEXITCODE -ne 0) { throw "Source commit failed." }
    } else {
        Write-Host "No source changes to commit."
    }

    Write-Host "==> cargo build --release"
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "Release build failed." }

    $exe = Join-Path $root "target\release\hcompare.exe"
    if (-not (Test-Path $exe)) { throw "Release binary not found at $exe." }
    $build = Get-BuildDate $exe
    $stamp = $build.Date.Substring(2).Replace("-", ".")

    if ([string]::IsNullOrWhiteSpace($Version)) {
        $Version = Get-NextVersion $publish $stamp
    } elseif (-not $Version.StartsWith("v")) {
        $Version = "v$Version"
    }

    New-Item -ItemType Directory -Force -Path $publish | Out-Null
    $artifact = "hcompare-$Version.exe"
    $artifactPath = Join-Path $publish $artifact
    Copy-Item $exe $artifactPath -Force

    $hash = (Get-FileHash $artifactPath -Algorithm SHA256).Hash.ToLower()
    $size = (Get-Item $artifactPath).Length

    # Keep checksums for every published portable executable.
    Get-ChildItem $publish -Filter "hcompare-v*.exe" |
        Sort-Object Name |
        ForEach-Object {
            "$((Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower())  $($_.Name)"
        } |
        Set-Content -Encoding ascii (Join-Path $publish "SHA256SUMS.txt")

    $packageMatch = [regex]::Match($build.Text, '^hcompare (\S+)')
    $packageVersion = if ($packageMatch.Success) { $packageMatch.Groups[1].Value } else { "0.0.0" }
    $built = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffZ")
    $commit = (& git -C $repoRoot rev-parse --short HEAD).Trim()
    $rustc = (& rustc --version).Trim()
    $cargo = (& cargo --version).Trim()
    $buildVersion = $Version.TrimStart("v")
    @"
H-Compare release build
tag: $Version
version: $packageVersion
buildVersion: $buildVersion
commit: $commit
dirty: false
built: $built
rustc: $rustc
cargo: $cargo
signed: no

artifacts:
$hash $size $artifact
"@ | Set-Content -Encoding ascii (Join-Path $publish "BUILD.txt")

    Write-Host "==> commit and push release artifacts"
    & git -C $repoRoot add h-compare/publish
    if ($LASTEXITCODE -ne 0) { throw "git add for publish failed." }
    & git -C $repoRoot commit -m "release(h-compare): publish $Version"
    if ($LASTEXITCODE -ne 0) { throw "Release commit failed." }

    if (-not $NoPush) {
        & git -C $repoRoot push
        if ($LASTEXITCODE -ne 0) { throw "git push failed." }
    }

    Write-Host ""
    Write-Host "Published $artifact"
    Write-Host "SHA256   $hash"
    Write-Host "Build    $($build.Text)"
} finally {
    Pop-Location
}
