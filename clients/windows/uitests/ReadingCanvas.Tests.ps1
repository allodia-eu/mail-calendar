#!/usr/bin/env pwsh
# The reading pane's body area is the PAGE a message is drawn on, and it is that page in a dark
# window too (docs/sync-progress.md). Two things have to be true of it, and neither can be seen
# without a rendered window in a pinned appearance:
#
#   * the page is painted. `BodyArea.Background` is assigned in the code-behind from the core's
#     `message_canvas`, the class of wiring that compiles, renders and does nothing. Left unset,
#     the area falls through to the pane's own background: white in a light window, so a
#     light-mode developer sees nothing wrong, and the dark panel that was the defect in a dark
#     one.
#   * the ink on it is resolved FOR the page. `RequestedTheme="Light"` is what does that. Remove
#     it and the dark theme's own label colours draw white on white, which is not a flicker but a
#     body area that is simply blank, and blank reads as "this message has no text".
#
# `Mailcal.Tests` cannot link WinUI, so it can see neither a brush nor a resolved foreground.
#
# COLOUR IS A PIXEL RULE, UIA cannot see it, so the page is sampled off the screen the same way
# Appearance.Tests.ps1 samples the caption. The sampler is this file's own: it measures a
# different surface, and reading a body area means insetting from a ScrollViewer's own chrome.
#
# THE RUN IS PINNED TO DARK, which is the whole point: in a light window every assertion below
# passes over a body area that was never painted at all. The first case is what makes the pin
# mean something, the window's own chrome has to be dark while the page in it is white. Together
# they are the contract: the page is the core's colour, the chrome around it stays themed.
#
# Dataset is `harness`: the assertion needs a message whose body is text/plain, so that what is on
# the page is WinUI's own drawing rather than a WebView2 document painting its own white. The
# showcase seeds HTML bodies only, which would hide a body area that is still a hole.
#
# A PLAIN-TEXT BODY IS THE OBSERVABLE, not the state the defect showed on. The flicker was in the
# gap before a body lands, and the spinner, the load error and "no content" sit on the same page;
# none of the four is reachable against a real server, which prefetches bodies and serves them. But
# all five states are the one `BodyArea`, painted once in the constructor and themed once in the
# XAML, so a plain-text body is where those two can be read off the screen. The other four were
# read by hand in dark, against a build that forced each state.

Add-Type -AssemblyName System.Drawing

# The caption band Appearance.Tests.ps1 and TitleBar.Tests.ps1 sample, and for the same reason: it
# is the one surface that spans the app's whole theme and carries no text of its own.
$CanvasCaptionXFraction = 0.55
$CanvasCaptionWidthPx = 120
$CanvasCaptionTop = 12
$CanvasCaptionHeight = 8

# Nothing here is near the midpoint: a dark caption renders around 20-30 and the page is #ffffff.
$CanvasMidLuminance = 128
# The floor for "this is the page". The light theme's own panel brushes sit in the 240s and the
# dark theme's in the 30s and 40s, so anything above this is the page and nothing else is close.
$CanvasPageFloor = 200
# The ceiling for "there is ink on it". Light-theme body text is near-black; white-on-white leaves
# the darkest pixel in the sample as light as the page itself.
$CanvasInkCeiling = 100

# A message whose body is text/plain, from docker/stalwart/seed/mail/01-plain.eml.
$CanvasPlainSubject = 'Harness baseline message'

<#
.SYNOPSIS
The median, lightest and darkest luminance (0..255) of a band of screen pixels.
#>
function Get-BandLuminance {
  param([int] $X, [int] $Y, [int] $Width, [int] $Height)
  $bmp = New-Object System.Drawing.Bitmap $Width, $Height
  try {
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    try { $g.CopyFromScreen($X, $Y, 0, 0, $bmp.Size) } finally { $g.Dispose() }
    $lums = New-Object 'System.Collections.Generic.List[double]'
    for ($px = 0; $px -lt $Width; $px++) {
      for ($py = 0; $py -lt $Height; $py++) {
        $c = $bmp.GetPixel($px, $py)
        # Rec. 601 luma, what a human eye weights, so "light or dark" means what it says.
        $lums.Add((0.299 * $c.R) + (0.587 * $c.G) + (0.114 * $c.B))
      }
    }
    $sorted = $lums | Sort-Object
    [pscustomobject]@{
      Median = $sorted[[int]($sorted.Count / 2)]
      Darkest = $sorted[0]
      Lightest = $sorted[-1]
    }
  }
  finally { $bmp.Dispose() }
}

<#
.SYNOPSIS
The median luminance of the running window's caption.
#>
function Get-CanvasCaptionLuminance {
  $window = Get-MailcalWindow
  if (-not $window) { throw 'no Mailcal window, the dataset should have launched one' }
  $r = $window.Current.BoundingRectangle
  (Get-BandLuminance ([int]($r.X + ($r.Width * $CanvasCaptionXFraction))) ([int]($r.Y + $CanvasCaptionTop)) `
      $CanvasCaptionWidthPx $CanvasCaptionHeight).Median
}

<#
.SYNOPSIS
Opens the plain-text fixture and returns the rendered bounds of the control that draws it.
.DESCRIPTION
PlainScroller, not the body row's Grid: a bare layout panel gets no automation peer, so BodyArea
itself is unreachable and only what is drawn inside it can be measured (uia.ps1).
#>
function Open-PlainBody {
  Invoke-UiaElement (Get-MailRowByTitle $CanvasPlainSubject)
  $body = Wait-UiaElement -AutomationId 'PlainScroller' -TimeoutSec 30
  if (-not $body) {
    throw "no PlainScroller within 30s, the reading pane never drew '$CanvasPlainSubject' as text"
  }
  Get-RenderedBounds -Element $body -What "the plain-text body of '$CanvasPlainSubject'"
}

$Suite = @{
  Dataset = 'harness'
  Env     = @{ MAILCAL_APPEARANCE = 'dark' }
  Cases   = @(
    @{
      Name = 'the window really is dark, which is what makes the rest of this suite an assertion'
      Body = {
        $lum = Get-CanvasCaptionLuminance
        Assert-True ($lum -lt $CanvasMidLuminance) (
          "the caption sampled at luminance $lum, i.e. light, but this run is pinned to dark with " +
          'MAILCAL_APPEARANCE. In a light window the page and the chrome are both white and every ' +
          'case below passes over a body area that was never painted')
      }
    },
    @{
      Name = 'the body area is the page the core names, not the dark theme''s panel'
      Body = {
        $r = Open-PlainBody
        # Inset well past the ScrollViewer's own edge chrome, and below the first lines of text, so
        # this band is the page and nothing but the page.
        $inset = ConvertTo-UiaPixels 40
        $band = Get-BandLuminance ([int]($r.X + $inset)) ([int]($r.Y + $r.Height - $inset)) `
          ([int][Math]::Min(240, $r.Width - (2 * $inset))) 24
        Assert-True ($band.Median -gt $CanvasPageFloor) (
          "the body area sampled at luminance $($band.Median) behind a plain-text body in a dark " +
          'window, so it is the pane''s own background showing through. The area is the page a ' +
          'message is drawn on, in both themes (docs/sync-progress.md): the core hands the colour ' +
          'over as message_canvas and ReadingView''s constructor paints it onto BodyArea. Nothing ' +
          'else draws it, the ScrollViewer has no background of its own')
      }
    },
    @{
      Name = 'the ink on that page is resolved for the page, not for the theme around it'
      Body = {
        $r = Open-PlainBody
        # The body's first lines: the ScrollViewer pads 16, and the text starts at its top left.
        $pad = ConvertTo-UiaPixels 16
        $band = Get-BandLuminance ([int]($r.X + $pad)) ([int]($r.Y + $pad)) `
          ([int][Math]::Min(400, $r.Width - (2 * $pad))) ([int](ConvertTo-UiaPixels 44))
        Assert-True ($band.Darkest -lt $CanvasInkCeiling) (
          "the darkest pixel over the message's own first lines is $($band.Darkest), so there is " +
          'no ink on the page: the text resolved the DARK theme''s label colour and is drawing ' +
          'white on white. RequestedTheme="Light" on BodyArea is what pairs with the page, and ' +
          'nothing headless can see a resolved foreground')
      }
    }
  )
}
