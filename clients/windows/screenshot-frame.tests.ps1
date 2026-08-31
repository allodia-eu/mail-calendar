#!/usr/bin/env pwsh
# Tests for screenshot-frame.ps1, the crop that keeps the invisible window border out of a store
# screenshot. Run by build-and-run.ps1 (so the `windows` CI job gates them) and standalone:
#
#   ./screenshot-frame.tests.ps1
#
# Plain assertions rather than Pester: the repo has no PowerShell test harness, and the Windows
# runner's preinstalled Pester version is not something this gate should depend on. Exit code is
# the contract, 0 all passed, 1 something failed.
#
# The fixtures are synthetic captures. The one that matters is New-CaptureBitmap's default shape,
# which reproduces what PrintWindow actually hands back: the caption painted across the *full*
# window rect, and the client area below it inset by the invisible resize border, so the unpainted
# margin is an L, not a rectangle. Measuring it down whole edge columns finds nothing, which is how
# a black-framed set reached showcase-screenshots/windows/ in the first place.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
. (Join-Path $PSScriptRoot 'screenshot-frame.ps1')

$script:Failed = 0
$script:Passed = 0

function Assert-Equal($expected, $actual, [string] $what) {
  if ("$expected" -eq "$actual") {
    $script:Passed++
  }
  else {
    $script:Failed++
    Write-Host "  FAIL $what, expected <$expected>, got <$actual>" -ForegroundColor Red
  }
}

function Assert-Empty($actual, [string] $what) {
  Assert-Equal '' ($actual -join '/') $what
}

# A capture the shape PrintWindow really produces. $inset px of unpainted (black) border down the
# left, the right and the bottom; the caption band across the full width of the top, because DWM
# draws it over the invisible border too; white client area in between.
function New-CaptureBitmap([int] $w, [int] $h, [int] $inset, [int] $caption = 20) {
  $b = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($b)
  $g.Clear([System.Drawing.Color]::Black)
  $g.FillRectangle([System.Drawing.Brushes]::Gainsboro, 0, 0, $w, $caption)
  $g.FillRectangle([System.Drawing.Brushes]::White,
    $inset, $caption, ($w - (2 * $inset)), ($h - $caption - $inset))
  $g.Dispose()
  return $b
}

# The same capture as above, plus the window's own top border row, the row PrintWindow renders as
# pure white under a light theme and pure black under a dark one, whatever colour the border really
# is. $chrome is the caption colour, i.e. the theme.
#
# The border row stops at the invisible resize border rather than spanning the window rect, so the
# two top corners are unpainted in both themes. That is the shape a real capture has, and the shape
# that catches a border-row test anchored on pixel (0,0) instead of on the frame between the
# margins, which reads the corner, compares the row against black, and finds no border under a
# light theme.
function New-ThemedCaptureBitmap([int] $w, [int] $h, [int] $inset, [System.Drawing.Color] $chrome,
  [System.Drawing.Color] $borderRow, [int] $caption = 20) {
  $b = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($b)
  $g.Clear([System.Drawing.Color]::Black)
  $g.FillRectangle((New-Object System.Drawing.SolidBrush $chrome),
    $inset, 0, ($w - (2 * $inset)), ($h - $inset))
  $g.FillRectangle((New-Object System.Drawing.SolidBrush $borderRow),
    $inset, 0, ($w - (2 * $inset)), 1)
  $g.Dispose()
  return $b
}

function New-SolidBitmap([int] $w, [int] $h, [System.Drawing.Color] $color) {
  $b = New-Object System.Drawing.Bitmap($w, $h)
  $g = [System.Drawing.Graphics]::FromImage($b)
  $g.Clear($color)
  $g.Dispose()
  return $b
}

Write-Host '==> screenshot-frame' -ForegroundColor Cyan

# 1. The regression. A 13 px L-shaped margin is found on all three sides, and the top, where the
#    caption reaches the window's own edge, is correctly left alone.
$b = New-CaptureBitmap 200 200 13
$m = Get-UnpaintedMargin $b 13 13
Assert-Equal 13 $m.left 'L-shaped margin: left'
Assert-Equal 13 $m.right 'L-shaped margin: right'
Assert-Equal 13 $m.bottom 'L-shaped margin: bottom'
Assert-Equal 0 $m.top 'L-shaped margin: top (caption reaches the edge)'

# 2. ...and the whole-column measurement that shipped the bug finds nothing, because the caption
#    spans the border. This is the assertion that would have failed before the fix.
Assert-Equal $false (Test-BlackLine $b $false 0) 'a whole-column test misses the L, hence centre lines'

# 3. Cropping it leaves exactly the painted frame, with no black edge left to report.
$cropped = Remove-UnpaintedMargin $b $m
Assert-Equal 174 $cropped.Width 'cropped width  (200 - 2*13)'
Assert-Equal 187 $cropped.Height 'cropped height (200 - 13)'
Assert-Empty (Get-BlackEdges $cropped) 'no black edge survives the crop'
$cropped.Dispose(); $b.Dispose()

# 4. A capture with nothing to crop is returned untouched, the same instance, so screenshot.ps1's
#    ownership check (dispose the original only if it got a different bitmap) holds.
$b = New-SolidBitmap 60 40 ([System.Drawing.Color]::White)
$m = Get-UnpaintedMargin $b 13 13
Assert-Equal 0 ($m.left + $m.right + $m.top + $m.bottom) 'clean capture: nothing to crop'
Assert-Equal $true ([object]::ReferenceEquals((Remove-UnpaintedMargin $b $m), $b)) 'clean capture: same instance back'
Assert-Empty (Get-BlackEdges $b) 'clean capture: no black edge'
$b.Dispose()

# 5. A blank frame, the failure the byte floor in showcase.sh cannot see, is NOT quietly cropped
#    away to nothing. The cap stops the crop, and the black edge survives to fail the run.
$b = New-SolidBitmap 60 40 ([System.Drawing.Color]::Black)
$m = Get-UnpaintedMargin $b 13 13
Assert-Equal 13 $m.left 'blank frame: crop stops at the cap'
$cropped = Remove-UnpaintedMargin $b $m
Assert-Equal 'top/bottom/left/right' ((Get-BlackEdges $cropped) -join '/') 'blank frame: every edge reported'
$cropped.Dispose(); $b.Dispose()

# 6. A margin wider than any real window frame is a broken capture, not a border: the crop is
#    capped and what is left still reports black, so the asset is never written.
$b = New-CaptureBitmap 200 200 25
$m = Get-UnpaintedMargin $b 13 13
Assert-Equal 13 $m.left 'over-wide margin: capped at the frame inset'
$cropped = Remove-UnpaintedMargin $b $m
Assert-Equal 'bottom/left/right' ((Get-BlackEdges $cropped) -join '/') 'over-wide margin: still reported after cropping'
$cropped.Dispose(); $b.Dispose()

# 7. A margin narrower than the cap is cropped exactly, not to the cap, a 100% display has an
#    8 px border where a 200% one has 13.
$b = New-CaptureBitmap 200 200 5
$m = Get-UnpaintedMargin $b 13 13
Assert-Equal 5 $m.left 'narrow margin: cropped to what is there, not to the cap'
Assert-Equal 5 $m.bottom 'narrow margin: bottom'
$b.Dispose()

# 8. Dark theme is not a margin. WinUI's dark surface is #202020, and treating "nearly black" as
#    unpainted would eat a real edge off every dark-mode capture.
$b = New-SolidBitmap 60 40 ([System.Drawing.Color]::FromArgb(0x20, 0x20, 0x20))
$m = Get-UnpaintedMargin $b 13 13
Assert-Equal 0 ($m.left + $m.right + $m.top + $m.bottom) 'dark theme (#202020) is content, not margin'
Assert-Empty (Get-BlackEdges $b) 'dark theme: no black edge reported'
$b.Dispose()

# 9. A single black edge is named on its own, so a failure message points at the right side.
$b = New-SolidBitmap 60 40 ([System.Drawing.Color]::White)
$g = [System.Drawing.Graphics]::FromImage($b)
$g.FillRectangle([System.Drawing.Brushes]::Black, 0, 39, 60, 1)
$g.Dispose()
Assert-Equal 'bottom' ((Get-BlackEdges $b) -join '/') 'a single black edge is named'
$b.Dispose()

# 10. Both probes earn their place. An edge that is black everywhere *except* the centre pixel is
#     still an unpainted edge, the centre probe alone would wave it through.
$b = New-SolidBitmap 60 40 ([System.Drawing.Color]::White)
$g = [System.Drawing.Graphics]::FromImage($b)
$g.FillRectangle([System.Drawing.Brushes]::Black, 0, 39, 60, 1)
$g.FillRectangle([System.Drawing.Brushes]::White, 30, 39, 1, 1)
$g.Dispose()
Assert-Equal 'bottom' ((Get-BlackEdges $b) -join '/') 'the whole-line probe catches what the centre probe steps over'
$b.Dispose()

# 11. The border row goes in BOTH themes, so one screen cannot come out two different sizes.
#     This is the defect: measured only as a black margin the row is cropped under a dark theme and
#     kept under a light one, and the dark capture reaches the store a pixel short of its slot,
#     past the byte floor, past the Store's own pixel bounds, past every eye that saw it.
$light = New-ThemedCaptureBitmap 200 200 13 ([System.Drawing.Color]::FromArgb(0xF3, 0xF3, 0xF3)) ([System.Drawing.Color]::White)
$dark = New-ThemedCaptureBitmap 200 200 13 ([System.Drawing.Color]::FromArgb(0x20, 0x20, 0x20)) ([System.Drawing.Color]::Black)
$ml = Get-UnpaintedMargin $light 13 13
$md = Get-UnpaintedMargin $dark 13 13
Assert-Equal 1 $ml.top 'border row: cropped under a light theme, where it is white'
Assert-Equal 1 $md.top 'border row: cropped under a dark theme, where it is black'
$cl = Remove-UnpaintedMargin $light $ml
$cd = Remove-UnpaintedMargin $dark $md
Assert-Equal $cl.Height $cd.Height 'border row: both themes crop to the SAME height'
Assert-Equal 186 $cl.Height 'border row: 200 - 1 (border) - 13 (resize border)'
Assert-Empty (Get-BlackEdges $cl) 'border row: no black edge survives the light crop'
Assert-Empty (Get-BlackEdges $cd) 'border row: no black edge survives the dark crop'
$cl.Dispose(); $cd.Dispose(); $light.Dispose(); $dark.Dispose()

# 12. It is the WINDOW BORDER that is recognised, not "a saturated row". A blank frame is uniform
#     all the way down, so its first row is not a border, otherwise a white-out or a black-out
#     would quietly lose a row and report itself one pixel healthier than it is. (Cases 4 and 5
#     already pin what those bitmaps do; this says why the row underneath has to differ.)
$b = New-SolidBitmap 60 40 ([System.Drawing.Color]::White)
Assert-Equal $false (Test-BorderRow $b) 'a uniformly white frame has no border row'
$b.Dispose()
$b = New-SolidBitmap 60 40 ([System.Drawing.Color]::Black)
Assert-Equal $false (Test-BorderRow $b) 'a uniformly black frame has no border row'
$b.Dispose()

# 13. Nor is chrome that merely reaches the top edge: the caption is a colour, not a saturation.
$b = New-CaptureBitmap 200 200 13
Assert-Equal $false (Test-BorderRow $b) 'a caption at the top edge is not a border row'
Assert-Equal 0 (Get-UnpaintedMargin $b 13 13).top 'a caption at the top edge is still not cropped'
$b.Dispose()

# 14. A maximised window IS inflated at the top, and that band is wider than a row, the walk keeps
#     it rather than the border-row rule capping it at one.
$b = New-SolidBitmap 200 200 ([System.Drawing.Color]::White)
$g = [System.Drawing.Graphics]::FromImage($b)
$g.FillRectangle([System.Drawing.Brushes]::Black, 0, 0, 200, 13)
$g.Dispose()
Assert-Equal 13 (Get-UnpaintedMargin $b 13 13).top 'maximized: the whole top band is the margin'
$b.Dispose()

if ($script:Failed -gt 0) {
  Write-Host "==> screenshot-frame: $($script:Failed) failed, $($script:Passed) passed" -ForegroundColor Red
  exit 1
}
Write-Host "==> screenshot-frame: $($script:Passed) passed" -ForegroundColor Green
