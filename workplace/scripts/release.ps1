# Release task for Daybook (workplace).
#
# Runs the quality gates, commits source, builds the portable release binary and
# publishes it under releases/ (daybook-v<version>.exe, SHA256SUMS.txt and
# BUILD.txt). start-daybook.bat runs the newest version from that folder.
#
# Version format follows the other projects: vYY.MM.DD.N (e.g. v26.11.25.1).
#
# Usage:
#   .\scripts\release.ps1
#   .\scripts\release.ps1 -Message "feat(workplace): ..." -SkipTests -NoPush
[CmdletBinding()]
param(
    [string]$Message = "",
    [switch]$SkipTests,
    [switch]$NoPush,
    [string]$Date = (Get-Date -Format "yy.MM.dd")
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$releases = Join-Path $root "releases"

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

Push-Location $root
try {
    $repoRoot = (& git rev-parse --show-toplevel).Trim()
    if ([string]::IsNullOrWhiteSpace($repoRoot)) {
        throw "Not inside a git repository."
    }

    # Windows keeps the running binary locked while the server is up.
    Get-Process daybook -ErrorAction SilentlyContinue | Stop-Process -Force
    Start-Sleep -Milliseconds 300

    if (-not $SkipTests) {
        Invoke-Step "cargo fmt --check" { cargo fmt --all -- --check }
        Invoke-Step "cargo clippy" { cargo clippy --all-targets -- -D warnings }
        Invoke-Step "cargo test" { cargo test }
        Invoke-Step "node tests" { node --test tests/js/*.test.mjs }
    } else {
        Write-Host "==> quality gates skipped (-SkipTests)"
    }

    # Commit source (excluding releases/) so BUILD.txt names a real, clean commit.
    Write-Host "==> commit source changes (releases/ excluded)"
    & git -C $repoRoot add workplace
    if ($LASTEXITCODE -ne 0) { throw "git add failed." }
    & git -C $repoRoot reset --quiet -- workplace/releases
    if ($LASTEXITCODE -ne 0) { throw "git reset for releases failed." }
    & git -C $repoRoot diff --cached --quiet
    if ($LASTEXITCODE -ne 0) {
        $sourceMessage = if ([string]::IsNullOrWhiteSpace($Message)) {
            "chore(workplace): prepare release"
        } else {
            $Message
        }
        & git -C $repoRoot commit -m $sourceMessage
        if ($LASTEXITCODE -ne 0) { throw "Source commit failed." }
    } else {
        Write-Host "No source changes to commit."
    }

    # Reserve the next version for today.
    New-Item -ItemType Directory -Force $releases | Out-Null
    $sequence = 1
    while (Test-Path (Join-Path $releases "daybook-v$Date.$sequence.exe")) {
        $sequence += 1
    }
    $tag = "v$Date.$sequence"

    Invoke-Step "cargo build --release" { cargo build --release }

    $exe = Join-Path $root "target\release\daybook.exe"
    if (-not (Test-Path $exe)) {
        throw "Missing $exe."
    }

    $portableName = "daybook-$tag.exe"
    $portable = Join-Path $releases $portableName
    Copy-Item $exe $portable -Force

    $commit = (& git -C $repoRoot rev-parse --short HEAD).Trim()
    $rustc = (& rustc --version).Trim()
    $buildLines = @(
        "Daybook release",
        "build version: $($tag.TrimStart('v'))",
        "tag: $tag",
        "commit: $commit",
        "rustc: $rustc",
        "built: $((Get-Date).ToString('o'))",
        "artifact: $portableName"
    )
    $buildLines | Set-Content -Encoding ascii (Join-Path $releases "BUILD.txt")

    # Checksums for every published portable executable, newest last.
    Get-ChildItem $releases -Filter "daybook-v*.exe" |
        Sort-Object Name |
        ForEach-Object {
            "$((Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower())  $($_.Name)"
        } |
        Set-Content -Encoding ascii (Join-Path $releases "SHA256SUMS.txt")

    Write-Host "==> commit and push release artifacts"
    & git -C $repoRoot add workplace/releases
    if ($LASTEXITCODE -ne 0) { throw "git add for releases failed." }
    & git -C $repoRoot diff --cached --quiet
    if ($LASTEXITCODE -ne 0) {
        & git -C $repoRoot commit -m "release(workplace): publish $tag"
        if ($LASTEXITCODE -ne 0) { throw "Release commit failed." }
    } else {
        Write-Host "No release artifacts to commit."
    }

    if (-not $NoPush) {
        & git -C $repoRoot push
        if ($LASTEXITCODE -ne 0) { throw "git push failed." }
    }

    Write-Host ""
    Write-Host "Published $portableName"
    Write-Host "Version  $tag"
    Write-Host "Commit   $commit"
    Write-Host "Folder   $releases"
}
finally {
    Pop-Location
}
