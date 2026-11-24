# Release task for H-Compare.
#
# Runs the quality gates, builds the release binary, publishes it under
# publish/ with BUILD.txt + SHA256SUMS.txt, commits h-compare and pushes.
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
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $repoRoot = (& git rev-parse --show-toplevel).Trim()
    if ([string]::IsNullOrWhiteSpace($repoRoot)) {
        throw "Not inside a git repository."
    }

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

    Write-Host "==> cargo build --release"
    cargo build --release
    if ($LASTEXITCODE -ne 0) { throw "Release build failed." }

    $exe = Join-Path $root "target\release\hcompare.exe"
    if (-not (Test-Path $exe)) { throw "Release binary not found at $exe." }
    $versionText = (& $exe --version).Trim()
    $buildMatch = [regex]::Match($versionText, 'build (\d{4}-\d{2}-\d{2})')
    if (-not $buildMatch.Success) {
        throw "Could not read the build date from '$versionText'."
    }
    $packageMatch = [regex]::Match($versionText, '^hcompare (\S+)')
    $packageVersion = if ($packageMatch.Success) { $packageMatch.Groups[1].Value } else { "0.0.0" }
    $stamp = $buildMatch.Groups[1].Value.Substring(2).Replace("-", ".")

    if ([string]::IsNullOrWhiteSpace($Version)) {
        $next = 1
        $publishDir = Join-Path $root "publish"
        if (Test-Path $publishDir) {
            $pattern = "^hcompare-v$([regex]::Escape($stamp))\.(\d+)\.exe$"
            foreach ($file in Get-ChildItem $publishDir -Filter "hcompare-v$stamp.*.exe") {
                $match = [regex]::Match($file.Name, $pattern)
                if ($match.Success) {
                    $candidate = [int]$match.Groups[1].Value + 1
                    if ($candidate -gt $next) { $next = $candidate }
                }
            }
        }
        $Version = "v$stamp.$next"
    } elseif (-not $Version.StartsWith("v")) {
        $Version = "v$Version"
    }

    $publish = Join-Path $root "publish"
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

    $built = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ss.fffZ")
    $commit = (& git -C $repoRoot rev-parse --short HEAD).Trim()
    $dirty = if ((& git -C $repoRoot status --porcelain -- h-compare | Measure-Object).Count -gt 0) {
        "true"
    } else {
        "false"
    }
    $rustc = (& rustc --version).Trim()
    $cargo = (& cargo --version).Trim()
    $buildVersion = $Version.TrimStart("v")
    @"
H-Compare release build
tag: $Version
version: $packageVersion
buildVersion: $buildVersion
commit: $commit
dirty: $dirty
built: $built
rustc: $rustc
cargo: $cargo
signed: no

artifacts:
$hash $size $artifact
"@ | Set-Content -Encoding ascii (Join-Path $publish "BUILD.txt")

    if ([string]::IsNullOrWhiteSpace($Message)) {
        $Message = "release(h-compare): publish $Version"
    }

    Write-Host "==> git add / commit / push"
    & git -C $repoRoot add h-compare
    if ($LASTEXITCODE -ne 0) { throw "git add failed." }

    & git -C $repoRoot diff --cached --quiet
    if ($LASTEXITCODE -ne 0) {
        & git -C $repoRoot commit -m $Message
        if ($LASTEXITCODE -ne 0) { throw "git commit failed." }
    } else {
        Write-Host "No staged changes to commit."
    }

    if (-not $NoPush) {
        & git -C $repoRoot push
        if ($LASTEXITCODE -ne 0) { throw "git push failed." }
    }

    Write-Host ""
    Write-Host "Published $artifact"
    Write-Host "SHA256   $hash"
    Write-Host "Build    $versionText"
} finally {
    Pop-Location
}
