# Repairs UTF-8 text that was round-tripped through a CP1252 read/write and then
# normalises remaining typographic punctuation to ASCII.
#
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\fix-encoding.ps1
$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$cp1252 = [System.Text.Encoding]::GetEncoding(1252)
$utf8 = New-Object System.Text.UTF8Encoding($false)

# Punctuation -> ASCII, applied after the double-encoding repair.
$map = @{
    [char]0x2014 = "-"      # em dash
    [char]0x2013 = "-"      # en dash
    [char]0x2026 = "..."    # ellipsis
    [char]0x00D7 = "x"      # multiplication sign
    [char]0x00B7 = "-"      # middle dot
    [char]0x00B1 = "+/-"    # plus-minus
    [char]0x2192 = "->"     # arrow
    [char]0x00A0 = " "      # nbsp
    [char]0x2019 = "'"
    [char]0x2018 = "'"
    [char]0x201C = '"'
    [char]0x201D = '"'
    [char]0x00E2 = ""       # leftover from mojibake
    [char]0x00C3 = ""
    [char]0x00C2 = ""
}

$extensions = @("*.ts", "*.svelte", "*.css", "*.rs", "*.md", "*.json", "*.ps1", "*.mjs")
$changed = 0

Get-ChildItem (Join-Path $root "ui\src") -Recurse -File -Include *.ts, *.svelte, *.css | ForEach-Object {
    $path = $_.FullName
    $text = [System.IO.File]::ReadAllText($path, $utf8)
    $original = $text

    # Reverse "UTF-8 decoded as CP1252, then saved as UTF-8".
    if ($text -match "[^\x00-\x7F]") {
        $candidate = [System.Text.Encoding]::UTF8.GetString($cp1252.GetBytes($text))
        # The mojibake marker is "â€" (U+00E2 U+20AC). If the candidate no longer
        # contains it, the repair succeeded.
        $marker = [string][char]0x00E2 + [string][char]0x20AC
        if (-not $candidate.Contains($marker)) {
            $text = $candidate
        }
    }

    foreach ($key in $map.Keys) {
        $text = $text.Replace([string]$key, $map[$key])
    }

    # Anything still outside ASCII is replaced with '?' so the file is stable.
    $text = [regex]::Replace($text, "[^\x00-\x7F]", "?")

    if ($text -ne $original) {
        [System.IO.File]::WriteAllText($path, $text, $utf8)
        $changed += 1
        Write-Host "fixed $($_.FullName.Replace($root + '\', ''))"
    }
}

Write-Host "files changed: $changed"
