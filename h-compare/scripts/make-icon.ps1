# Generates assets/hcompare.ico from the H-Compare mark.
# Run from h-compare: powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\make-icon.ps1
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$assets = Join-Path $root "assets"
New-Item -ItemType Directory -Force -Path $assets | Out-Null
$target = Join-Path $assets "hcompare.ico"

function New-RoundedPath {
    param(
        [System.Drawing.RectangleF]$Rect,
        [single]$Radius
    )
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $diameter = $Radius * 2.0
    $path.AddArc($Rect.X, $Rect.Y, $diameter, $diameter, 180, 90)
    $path.AddArc($Rect.Right - $diameter, $Rect.Y, $diameter, $diameter, 270, 90)
    $path.AddArc($Rect.Right - $diameter, $Rect.Bottom - $diameter, $diameter, $diameter, 0, 90)
    $path.AddArc($Rect.X, $Rect.Bottom - $diameter, $diameter, $diameter, 90, 90)
    $path.CloseFigure()
    return $path
}

function New-IconData {
    param([int]$Size)
    $bitmap = New-Object System.Drawing.Bitmap($Size, $Size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $graphics.Clear([System.Drawing.Color]::Transparent)

    $scale = [single]$Size / 256.0
    $rect = New-Object System.Drawing.RectangleF((4 * $scale), (4 * $scale), (248 * $scale), (248 * $scale))
    $path = New-RoundedPath -Rect $rect -Radius (52 * $scale)
    $gradient = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        $rect,
        [System.Drawing.Color]::FromArgb(255, 30, 44, 62),
        [System.Drawing.Color]::FromArgb(255, 9, 14, 21),
        [single]45
    )
    $graphics.FillPath($gradient, $path)
    $border = New-Object System.Drawing.Pen([System.Drawing.Color]::FromArgb(255, 47, 128, 237), ([single](6 * $scale)))
    $graphics.DrawPath($border, $path)

    $left = New-RoundedPath -Rect (New-Object System.Drawing.RectangleF((66 * $scale), (62 * $scale), (32 * $scale), (132 * $scale))) -Radius (12 * $scale)
    $right = New-RoundedPath -Rect (New-Object System.Drawing.RectangleF((158 * $scale), (62 * $scale), (32 * $scale), (132 * $scale))) -Radius (12 * $scale)
    $bar = New-RoundedPath -Rect (New-Object System.Drawing.RectangleF((84 * $scale), (114 * $scale), (88 * $scale), (28 * $scale))) -Radius (9 * $scale)
    $leftBrush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        (New-Object System.Drawing.RectangleF((66 * $scale), (62 * $scale), (32 * $scale), (132 * $scale))),
        [System.Drawing.Color]::FromArgb(255, 240, 107, 107),
        [System.Drawing.Color]::FromArgb(255, 196, 62, 62),
        [single]90
    )
    $rightBrush = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        (New-Object System.Drawing.RectangleF((158 * $scale), (62 * $scale), (32 * $scale), (132 * $scale))),
        [System.Drawing.Color]::FromArgb(255, 88, 211, 139),
        [System.Drawing.Color]::FromArgb(255, 39, 154, 92),
        [single]90
    )
    $barBrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 246, 250, 255))
    $graphics.FillPath($leftBrush, $left)
    $graphics.FillPath($rightBrush, $right)
    $graphics.FillPath($barBrush, $bar)

    # ICO entries use a 32-bit DIB (BITMAPINFOHEADER + BGRA pixels + AND mask).
    $pixelRect = New-Object System.Drawing.Rectangle(0, 0, $Size, $Size)
    $lock = $bitmap.LockBits($pixelRect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $stride = $lock.Stride
    $pixels = New-Object byte[] ($stride * $Size)
    [System.Runtime.InteropServices.Marshal]::Copy($lock.Scan0, $pixels, 0, $pixels.Length)
    $bitmap.UnlockBits($lock)

    $result = New-Object System.IO.MemoryStream
    $dib = New-Object System.IO.BinaryWriter($result)
    $dib.Write([uint32]40)
    $dib.Write([int32]$Size)
    $dib.Write([int32]($Size * 2))
    $dib.Write([uint16]1)
    $dib.Write([uint16]32)
    $dib.Write([uint32]0)
    $dib.Write([uint32]($Size * $Size * 4))
    $dib.Write([int32]0)
    $dib.Write([int32]0)
    $dib.Write([uint32]0)
    $dib.Write([uint32]0)
    for ($y = $Size - 1; $y -ge 0; $y--) {
        $dib.Write($pixels, $y * $stride, $Size * 4)
    }
    $maskStride = [int]([math]::Floor(($Size + 31) / 32) * 4)
    $mask = New-Object byte[] ($maskStride * $Size)
    $dib.Write($mask, 0, $mask.Length)
    $dib.Flush()
    $bytes = $result.ToArray()

    $dib.Dispose()
    $result.Dispose()
    $barBrush.Dispose()
    $leftBrush.Dispose()
    $rightBrush.Dispose()
    $bar.Dispose()
    $left.Dispose()
    $right.Dispose()
    $border.Dispose()
    $gradient.Dispose()
    $path.Dispose()
    $graphics.Dispose()
    $bitmap.Dispose()
    return $bytes
}

$sizes = @(16, 24, 32, 48, 64, 128, 256)
$images = @()
foreach ($size in $sizes) {
    $images += ,@($size, (New-IconData -Size $size))
}

$stream = New-Object System.IO.MemoryStream
$writer = New-Object System.IO.BinaryWriter($stream)
$writer.Write([uint16]0)
$writer.Write([uint16]1)
$writer.Write([uint16]$images.Count)
$offset = 6 + (16 * $images.Count)
foreach ($entry in $images) {
    $size = $entry[0]
    $data = $entry[1]
    $writer.Write([byte]$(if ($size -ge 256) { 0 } else { $size }))
    $writer.Write([byte]$(if ($size -ge 256) { 0 } else { $size }))
    $writer.Write([byte]0)
    $writer.Write([byte]0)
    $writer.Write([uint16]1)
    $writer.Write([uint16]32)
    $writer.Write([uint32]$data.Length)
    $writer.Write([uint32]$offset)
    $offset += $data.Length
}
foreach ($entry in $images) {
    $writer.Write([byte[]]$entry[1])
}
$writer.Flush()
[System.IO.File]::WriteAllBytes($target, $stream.ToArray())
$writer.Dispose()
$stream.Dispose()
Write-Host "wrote $target"
