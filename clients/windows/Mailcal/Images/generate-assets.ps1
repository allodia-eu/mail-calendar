#!/usr/bin/env pwsh
# Derives EVERY Windows launcher asset from the one brand source icon:
#   - the MSIX tile/store PNGs the Package.appxmanifest references (Store/packaged build), and
#   - app.ico, the multi-resolution icon embedded in the unpackaged exe (csproj <ApplicationIcon>)
#     that Windows shows on the taskbar and title bar for the `dotnet run` dev loop.
# Which source that is comes from brand.py, so this agrees with the other three generators without a
# second copy of the rule (docs/branding.md). Pass -Source to override it.
# Windows-only (uses System.Drawing). Committed outputs mean CI/clean checkouts need not run it, but
# package.ps1 regenerates them from source if they're missing.
param([string]$Source)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$dir = $PSScriptRoot
$repo = (Resolve-Path (Join-Path $dir '../../../..')).Path
if ($Source) {
  $src = $Source
} else {
  # python3 first, then python: the Store's Python stub is named `python`, and a box that has both
  # answers to whichever it has. Same order as package.ps1.
  $python = @('python3', 'python') |
    ForEach-Object { Get-Command $_ -ErrorAction SilentlyContinue } |
    Select-Object -First 1
  if (-not $python) { throw "need python3 (or python) on PATH to resolve the brand icon source" }
  $src = & $python.Source (Join-Path $repo 'scripts/dev/brand.py') --icon-source
  if ($LASTEXITCODE -ne 0) { throw "brand.py could not resolve the icon source" }
}
if (-not (Test-Path $src)) { throw "source icon not found: $src" }
Write-Host "==> Source icon: $src"
# Not `$source`: PowerShell matches variable names case-insensitively, so that name IS the -Source
# parameter above, and its [string] constraint would coerce the loaded image to the text
# "System.Drawing.Bitmap", every DrawImage below then fails on a type nothing in this file assigned.
$sourceImage = [System.Drawing.Image]::FromFile($src)

# Draw the source into a $w x $h bitmap at max quality. Square targets get the (square) source
# scaled to fill; non-square targets (wide tile, splash) get it centred on a transparent canvas so
# the art is never distorted by stretching.
function New-Scaled([int]$w, [int]$h) {
  $bmp = [System.Drawing.Bitmap]::new($w, $h, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  try {
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode   = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.SmoothingMode     = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)
    $side = [Math]::Min($w, $h)               # the square art fills the shorter side...
    $x = [int](($w - $side) / 2)              # ...and is centred on the longer one.
    $y = [int](($h - $side) / 2)
    $g.DrawImage($sourceImage, [System.Drawing.Rectangle]::new($x, $y, $side, $side))
  }
  finally { $g.Dispose() }
  return $bmp
}

# The manifest's tiles + splash + store logo. Square tiles fill; Wide/Splash centre (see above).
$assets = [ordered]@{
  'Square44x44Logo.png'   = @(44, 44)
  'Square71x71Logo.png'   = @(71, 71)
  'Square150x150Logo.png' = @(150, 150)
  'Square310x310Logo.png' = @(310, 310)
  'Wide310x150Logo.png'   = @(310, 150)
  'SplashScreen.png'      = @(620, 300)
  'StoreLogo.png'         = @(50, 50)
}

foreach ($name in $assets.Keys) {
  $w, $h = $assets[$name]
  $bmp = New-Scaled $w $h
  $bmp.Save((Join-Path $dir $name), [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
  Write-Host "wrote $name ($w x $h)"
}

# app.ico, a multi-resolution icon carrying every size Windows asks for (taskbar, title bar, Alt-Tab,
# jump list, high-DPI). Each frame is stored PNG-compressed, which modern Windows reads for all sizes
# and which keeps the 256px frame small. Built by hand because System.Drawing.Icon can't author a
# multi-frame .ico from bitmaps.
$icoSizes = @(16, 20, 24, 32, 40, 48, 64, 128, 256)
$frames = foreach ($s in $icoSizes) {
  $bmp = New-Scaled $s $s
  $ms = [System.IO.MemoryStream]::new()
  $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
  $bmp.Dispose()
  [pscustomobject]@{ Size = $s; Bytes = $ms.ToArray() }
}

$icoPath = Join-Path $dir 'app.ico'
$fs = [System.IO.File]::Create($icoPath)
$bw = [System.IO.BinaryWriter]::new($fs)
try {
  # ICONDIR: reserved=0, type=1 (icon), image count.
  $bw.Write([uint16]0); $bw.Write([uint16]1); $bw.Write([uint16]$frames.Count)
  # Image data starts after the 6-byte ICONDIR + one 16-byte ICONDIRENTRY per frame.
  $offset = 6 + (16 * $frames.Count)
  foreach ($f in $frames) {
    $dim = if ($f.Size -ge 256) { 0 } else { $f.Size }   # 0 in the byte means 256.
    $bw.Write([byte]$dim)           # width
    $bw.Write([byte]$dim)           # height
    $bw.Write([byte]0)              # palette count (0 = no palette)
    $bw.Write([byte]0)              # reserved
    $bw.Write([uint16]1)            # colour planes
    $bw.Write([uint16]32)           # bits per pixel
    $bw.Write([uint32]$f.Bytes.Length)  # bytes of image data
    $bw.Write([uint32]$offset)          # offset of image data
    $offset += $f.Bytes.Length
  }
  foreach ($f in $frames) { $bw.Write($f.Bytes) }
}
finally { $bw.Dispose(); $fs.Dispose() }
Write-Host "wrote app.ico ($($frames.Count) frames: $($icoSizes -join ', '))"

$sourceImage.Dispose()
