# Generates src-tauri/icons (PNG + multi-size ICO) for H-Finvasia.
# Run from H-Finvasia: powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\make-icon.ps1
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$icons = Join-Path $root "src-tauri\icons"
New-Item -ItemType Directory -Force -Path $icons | Out-Null

function New-RoundedPath {
    param([System.Drawing.RectangleF]$Rect, [single]$Radius)
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = $Radius * 2.0
    $path.AddArc($Rect.X, $Rect.Y, $d, $d, 180, 90)
    $path.AddArc($Rect.Right - $d, $Rect.Y, $d, $d, 270, 90)
    $path.AddArc($Rect.Right - $d, $Rect.Bottom - $d, $d, $d, 0, 90)
    $path.AddArc($Rect.X, $Rect.Bottom - $d, $d, $d, 90, 90)
    $path.CloseFigure()
    return $path
}

function New-MarketBitmap {
    param([int]$Size)
    $bmp = New-Object System.Drawing.Bitmap($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.Clear([System.Drawing.Color]::Transparent)

    $s = [single]$Size / 256.0
    $rect = New-Object System.Drawing.RectangleF((6 * $s), (6 * $s), (244 * $s), (244 * $s))
    $path = New-RoundedPath -Rect $rect -Radius (54 * $s)
    $gradient = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        $rect,
        [System.Drawing.Color]::FromArgb(255, 18, 27, 38),
        [System.Drawing.Color]::FromArgb(255, 8, 12, 19),
        [single]45
    )
    $g.FillPath($gradient, $path)
    $border = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(255, 42, 110, 168), ([single](5 * $s)))
    $g.DrawPath($border, $path)

    $up = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 61, 199, 126))
    $down = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 226, 92, 92))
    $wick = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(255, 120, 146, 172), ([single](6 * $s)))

    # wick + body helper (x centre, body top, body height in 256-space)
    $bars = @(
        @(78, 112, 74, $up),
        @(128, 74, 128, $down),
        @(178, 132, 66, $up)
    )
    foreach ($b in $bars) {
        $cx = $b[0] * $s
        $top = $b[1] * $s
        $h = $b[2] * $s
        $g.DrawLine($wick, $cx, ($top - 22 * $s), $cx, ($top + $h + 18 * $s))
        $body = New-RoundedPath -Rect (New-Object System.Drawing.RectangleF(($cx - 17 * $s), $top, (34 * $s), $h)) -Radius (7 * $s)
        $g.FillPath($b[3], $body)
    }

    $g.Dispose()
    return $bmp
}

foreach ($size in @(32, 128, 256)) {
    $bmp = New-MarketBitmap $size
    $target = Join-Path $icons ("{0}x{0}.png" -f $size)
    $bmp.Save($target, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    Write-Host "wrote $target"
}

# ICO containing PNG-compressed entries (valid for Windows Vista+).
$sizes = @(256, 128, 48, 32, 16)
$entries = @()
foreach ($size in $sizes) {
    $bmp = New-MarketBitmap $size
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    $entries += , @($size, $ms.ToArray())
    $ms.Dispose()
}

$icoPath = Join-Path $icons "icon.ico"
$fs = [System.IO.File]::Create($icoPath)
$bw = New-Object System.IO.BinaryWriter($fs)
$bw.Write([UInt16]0)
$bw.Write([UInt16]1)
$bw.Write([UInt16]$entries.Count)
$offset = 6 + (16 * $entries.Count)
foreach ($entry in $entries) {
    $size = [int]$entry[0]
    $data = [byte[]]$entry[1]
    if ($size -ge 256) { $bw.Write([byte]0) } else { $bw.Write([byte]$size) }
    if ($size -ge 256) { $bw.Write([byte]0) } else { $bw.Write([byte]$size) }
    $bw.Write([byte]0)
    $bw.Write([byte]0)
    $bw.Write([UInt16]1)
    $bw.Write([UInt16]32)
    $bw.Write([UInt32]$data.Length)
    $bw.Write([UInt32]$offset)
    $offset += $data.Length
}
foreach ($entry in $entries) { $bw.Write([byte[]]$entry[1]) }
$bw.Flush()
$bw.Dispose()
$fs.Dispose()
Write-Host "wrote $icoPath"
