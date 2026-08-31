#!/usr/bin/env pwsh
# The frame geometry behind screenshot.ps1: finding the unpainted margin a window capture carries,
# cropping it off, and reporting any black edge that survives. Dot-sourced by screenshot.ps1 (which
# owns the window handles and the shutter) and by screenshot-frame.tests.ps1 (which owns none of
# that and can therefore run in CI).
#
# It lives in its own file precisely because it is the part that was wrong: a capture framed in an
# unpainted black L shipped into the committed store screenshot set, and nothing could have caught
# it, the capture path needs a running WinUI app, a display, and a shutter, so it was verified by
# eye or not at all. Everything here takes a Bitmap and returns numbers, so the shapes that fooled
# us (a margin that is an L rather than a rectangle; a margin wider than any real window frame) are
# ordinary test cases.
#
# Defining functions only: no side effects on dot-source.

# Whether every pixel of one edge line is pure black. Sampled every 4th pixel: what we are looking
# for is a solid unpainted band, not a stray dark run, and a full scan of four 2900-px lines
# through GetPixel is slow enough to notice.
function Test-BlackLine([System.Drawing.Bitmap] $b, [bool] $horizontal, [int] $at) {
  $n = if ($horizontal) { $b.Width } else { $b.Height }
  for ($i = 0; $i -lt $n; $i += 4) {
    $c = if ($horizontal) { $b.GetPixel($i, $at) } else { $b.GetPixel($at, $i) }
    if ($c.R -ne 0 -or $c.G -ne 0 -or $c.B -ne 0) { return $false }
  }
  return $true
}

# How many black pixels deep the margin runs on one side, giving up at $cap.
#
# Measured along the bitmap's centre lines, not down whole edge lines. The caption is drawn across
# the *full* window rect, the top ~58 px at 200%, invisible side border included, while the client
# area below it starts one inset in, so the left and right bands are an L, not a rectangle. A
# whole-column test walks straight into the caption and reports no margin at all, which is exactly
# the bug that produced a black-framed store screenshot set.
function Measure-BlackMargin([System.Drawing.Bitmap] $b, [string] $side, [int] $cap) {
  $midX = [int]($b.Width / 2); $midY = [int]($b.Height / 2)
  $n = 0
  while ($n -lt $cap) {
    $c = switch ($side) {
      'left' { $b.GetPixel($n, $midY) }
      'right' { $b.GetPixel($b.Width - 1 - $n, $midY) }
      'top' { $b.GetPixel($midX, $n) }
      'bottom' { $b.GetPixel($midX, $b.Height - 1 - $n) }
      default { throw "unknown side '$side'" }
    }
    if ($c.R -ne 0 -or $c.G -ne 0 -or $c.B -ne 0) { break }
    $n++
  }
  return $n
}

# Whether the bitmap's first row is the window border rather than anything the app painted.
#
# PrintWindow does not render that row: it comes back a uniform, fully saturated line, pure white
# under a light theme, pure black under a dark one, where on screen the border is the same grey in
# both. Recognise it only when it reads BLACK and the crop turns theme-dependent: the row goes under
# a dark theme and stays under a light one, so two captures of one screen land a pixel apart and
# only the dark one is short of its store slot.
#
# Read between the side margins, never from the corner. The row stops at the invisible resize
# border rather than spanning the window rect, so pixel (0,0) is unpainted margin in BOTH themes,
# anchoring there compares the whole row against black, finds white at the first sampled pixel
# inside the frame, and reports no border row at all under a light theme.
#
# Recognised rather than assumed, so a host that does render the row loses nothing to this. The row
# beneath has to DIFFER, which is what keeps a uniformly white or uniformly black bitmap, a blank
# frame, from having its first row taken for a border and quietly cropped.
function Test-BorderRow([System.Drawing.Bitmap] $b, [int] $from, [int] $to) {
  if ($b.Height -lt 2 -or ($to - $from) -lt 2) { return $false }
  $edge = $b.GetPixel([int](($from + $to) / 2), 0)
  if ($edge.R -ne $edge.G -or $edge.G -ne $edge.B) { return $false }
  if ($edge.R -ne 0 -and $edge.R -ne 255) { return $false }
  # One pass, two questions: is the row uniform across the span, and does the row under it differ
  # anywhere. Sampled every 4th pixel, the same stride Test-BlackLine walks and for the same reason.
  $differs = $false
  for ($x = $from; $x -lt $to; $x += 4) {
    $c = $b.GetPixel($x, 0)
    if ($c.R -ne $edge.R -or $c.G -ne $edge.G -or $c.B -ne $edge.B) { return $false }
    $under = $b.GetPixel($x, 1)
    if ($under.R -ne $edge.R -or $under.G -ne $edge.G -or $under.B -ne $edge.B) { $differs = $true }
  }
  return $differs
}

# The unpainted margin on all four sides, each capped at the window frame's inset, $insetX for the
# left and right, $insetY for the top and bottom. The cap is what keeps this from eating content:
# black beyond a plausible window frame is not a margin, it is a broken capture, and Get-BlackEdges
# below is what says so.
#
# The top is the one side a RESTORED window is not inflated on, so the only thing there above the
# app's paint is the single border row: the walk finds it under a dark theme, where it is genuinely
# black, and Test-BorderRow finds it under a light one, where it is white. A maximised window is
# inflated at the top as well, and that band is wider than a row, so the walk keeps it.
function Get-UnpaintedMargin([System.Drawing.Bitmap] $b, [int] $insetX, [int] $insetY) {
  $left = Measure-BlackMargin $b 'left' $insetX
  $right = Measure-BlackMargin $b 'right' $insetX
  $top = Measure-BlackMargin $b 'top' $insetY
  if ($top -eq 0 -and (Test-BorderRow $b $left ($b.Width - $right))) { $top = 1 }
  return @{
    left   = $left
    right  = $right
    top    = $top
    bottom = Measure-BlackMargin $b 'bottom' $insetY
  }
}

# The bitmap with $margin cropped off. Returns the *same instance* when there is nothing to crop, so
# a caller disposes the original only when it got a different one back.
function Remove-UnpaintedMargin([System.Drawing.Bitmap] $b, [hashtable] $margin) {
  if (($margin.left + $margin.right + $margin.top + $margin.bottom) -eq 0) { return $b }
  $crop = New-Object System.Drawing.Rectangle(
    $margin.left, $margin.top,
    ($b.Width - $margin.left - $margin.right), ($b.Height - $margin.top - $margin.bottom))
  return $b.Clone($crop, $b.PixelFormat)
}

# Which edges still read as unpainted, by name, so a failure can say which. Two probes, because
# neither alone is enough:
#
#   * the centre pixel, the same one Measure-BlackMargin walks. After a crop this is black only if
#     the crop hit its cap, i.e. the margin was wider than any window frame, a blank capture, or a
#     window that moved mid-shot. This is the probe that fails.
#   * the whole edge line, which catches a uniformly unpainted edge the centre probe stepped over.
#
# The whole-line probe cannot stand on its own here: the caption is drawn across the full window
# rect, so an uncropped left or right margin is an L and its edge column is *not* entirely black.
# That is why the real 13 px border was only ever visible as a black bottom edge.
function Get-BlackEdges([System.Drawing.Bitmap] $b) {
  $midX = [int]($b.Width / 2); $midY = [int]($b.Height / 2)
  $black = {
    param($c)
    $c.R -eq 0 -and $c.G -eq 0 -and $c.B -eq 0
  }
  return @(
    @{ n = 'top'; hit = ((& $black $b.GetPixel($midX, 0)) -or (Test-BlackLine $b $true 0)) }
    @{ n = 'bottom'; hit = ((& $black $b.GetPixel($midX, $b.Height - 1)) -or (Test-BlackLine $b $true ($b.Height - 1))) }
    @{ n = 'left'; hit = ((& $black $b.GetPixel(0, $midY)) -or (Test-BlackLine $b $false 0)) }
    @{ n = 'right'; hit = ((& $black $b.GetPixel($b.Width - 1, $midY)) -or (Test-BlackLine $b $false ($b.Width - 1))) }
  ) | Where-Object { $_.hit } | ForEach-Object { $_.n }
}
