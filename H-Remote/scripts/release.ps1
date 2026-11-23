param(
    [ValidateSet("nsis", "msi", "all", "none")]
    [string]$Bundles = "nsis",
    [switch]$NoBuild,
    [switch]$AllowDirty,
    [string]$Date = (Get-Date -Format "yy.MM.dd"),
    [string]$SignCertificate = $env:HREMOTE_SIGN_CERT,
    [string]$SignPassword = $env:HREMOTE_SIGN_PASSWORD,
    [string]$TimestampUrl = $(if ($env:HREMOTE_TIMESTAMP_URL) { $env:HREMOTE_TIMESTAMP_URL } else { "http://timestamp.digicert.com" })
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Push-Location $root
try {
    $dirty = (git status --porcelain -- .)
    if ($dirty -and -not $AllowDirty) {
        throw "The working tree has uncommitted changes. Commit them first or pass -AllowDirty."
    }
    $commit = (git rev-parse --short HEAD).Trim()

    # The tag doubles as the build version so update checks compare release
    # versions instead of the crate's development version.
    New-Item -ItemType Directory -Force (Join-Path $root "release") | Out-Null
    $sequence = 1
    # Both the flat artifact and the per-tag folder are release evidence; a
    # previous run may have cleaned only the flat file, so never reuse a folder
    # (doing so used to overwrite an earlier release's BUILD.txt and binaries).
    while (
        (Test-Path (Join-Path $root "release\h-remote-v$Date.$sequence.exe")) -or
        (Test-Path (Join-Path $root "release\v$Date.$sequence"))
    ) {
        $sequence += 1
    }
    $tag = "v$Date.$sequence"
    $env:HREMOTE_BUILD_VERSION = $tag.TrimStart("v")

    if (-not $NoBuild) {
        # Native build tools write to stderr; temporarily relax Stop so those
        # lines are logged rather than treated as terminating errors.
        $previousEap = $ErrorActionPreference
        $ErrorActionPreference = "Continue"
        if ($Bundles -eq "none") {
            cmd /c "npm run tauri:build -- --no-bundle"
        } else {
            cmd /c "npm run tauri:build -- --bundles $Bundles"
        }
        $buildCode = $LASTEXITCODE
        $ErrorActionPreference = $previousEap
        if ($buildCode -ne 0) {
            throw "tauri build failed with exit code $buildCode"
        }
    }

    $exe = Join-Path $root "target\release\hremote.exe"
    if (-not (Test-Path $exe)) {
        throw "Missing $exe. Run without -NoBuild first."
    }

    New-Item -ItemType Directory -Force (Join-Path $root "release") | Out-Null
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
            $extension = $setup.Extension
            $installer = "h-remote-$tag-setup$extension"
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
            if ($LASTEXITCODE -ne 0) {
                throw "signtool failed for $($file.Name)"
            }
            & signtool verify /pa $file.FullName
            if ($LASTEXITCODE -ne 0) {
                throw "signature verification failed for $($file.Name)"
            }
        }
        $signed = "yes"
    }

    # Keep the previous flat layout for convenience while the tag folder remains
    # the auditable, self-contained release.
    Copy-Item (Join-Path $tagDir "*.exe") (Join-Path $root "release")
    Copy-Item (Join-Path $tagDir "*.msi") (Join-Path $root "release") -ErrorAction SilentlyContinue

    node scripts/build-info.mjs --tag $tag --dir $tagDir --signed $signed
    if ($LASTEXITCODE -ne 0) {
        throw "build-info failed with exit code $LASTEXITCODE"
    }

    $metadata = @(
        "H-Remote release",
        "app version: 0.1.0",
        "build version: $($tag.TrimStart('v'))",
        "tag: $tag",
        "commit: $commit",
        "dirty: $([bool]$dirty)",
        "built: $((Get-Date).ToString('o'))",
        "signed: $signed",
        "portable: $portableName",
        $(if ($installer) { "installer: $installer" }),
        "build info: release\$tag\BUILD.txt"
    )
    $metadata | Set-Content (Join-Path $root "release\RELEASE-$Date.$sequence.txt")

    Get-ChildItem (Join-Path $root "release") -Filter "h-remote-$tag*" -File | ForEach-Object {
        $hash = (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower()
        "$hash  $($_.Name)"
    } | Set-Content (Join-Path $root "release\SHA256SUMS-$tag.txt")

    Write-Host "Release artifacts:"
    Get-ChildItem $tagDir | Select-Object Name, Length
    Write-Host "Auditable build: $tagDir\BUILD.txt"
    Write-Host "Portable executable: $portable"
}
finally {
    Pop-Location
}
