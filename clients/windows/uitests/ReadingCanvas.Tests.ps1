#!/usr/bin/env pwsh
# The reading pane's body area is the PAGE a message is drawn on (docs/sync-progress.md), and in a
# dark window which page that is depends on the reader's choice (docs/reading-dark-mode.md): dark
# with dark messages on, white once the sun/moon button sends messages back to their light page.
# Three things have to be true of it, and none can be seen without a rendered window in a pinned
# appearance:
#
#   * with dark messages on, the page is dark and the ink on it light. The page and the ink are set
#     in ReadingView.Scheme.cs from the core's `message_canvas`, and the theme over the page is
#     the page's, not the window's.
#   * after the button, the page is painted white INSIDE A DARK WINDOW. This is the case the
#     suite was written for: `BodyArea.Background` is wiring that compiles, renders and does
#     nothing, and left unset the area falls through to the pane's own dark background, which is a
#     hole in a white page rather than a page.
#   * on that white page the ink is resolved FOR the page. `BodyArea.RequestedTheme` is what does
#     that; left at the window's, the dark theme's label colours draw white on white.
#
# `Mailcal.Tests` cannot link WinUI, so it can see neither a brush nor a resolved foreground.
#
# COLOUR IS A PIXEL RULE, UIA cannot see it, so the page is sampled off the screen the same way
# Appearance.Tests.ps1 samples the caption. The sampler is this file's own: it measures a
# different surface, and reading a body area means insetting from a ScrollViewer's own chrome.
#
# THE RUN IS PINNED TO DARK, which is the whole point: in a light window there is no button and
# every page is white. The first case is what makes the pin mean something.
#
# Dataset is `harness`: the assertion needs a message whose body is text/plain, so that what is on
# the page is WinUI's own drawing rather than a WebView2 document painting its own. The showcase
# seeds HTML bodies only, which would hide a body area that is still a hole.
#
# THE PREFERENCE PERSISTS, so the case that presses the button presses it again before it ends,
# and the next case, or the next suite, starts with dark messages on.

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
# The dark page's ceiling and its ink's floor: #1e1e1e under #e3e3e3, so a median below the
# midpoint with nothing lighter than this in the band is a dark page with no text on it.
$CanvasLightInkFloor = 170

# A message whose body is text/plain, from docker/stalwart/seed/mail/01-plain.eml.
$CanvasPlainSubject = 'Harness baseline message'

<#
.SYNOPSIS
The median, lightest and darkest luminance (0..255) of a band of screen pixels.
#>
function Get-BandLuminance {
  param([int] $X, [int] $Y, [int] $Width, [int] $Height)
  $bmp = Get-UiaScreenPixels -Element (Get-MailcalWindow) -X $X -Y $Y -Width $Width -Height $Height
  try {
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

<#
.SYNOPSIS
The sun/moon button, waited for: it exists only in a dark window.
#>
function Get-SchemeToggle {
  $toggle = Wait-UiaElement -AutomationId 'ReadingSchemeToggle' -TimeoutSec 15
  if (-not $toggle) {
    throw 'no ReadingSchemeToggle on the action row of a window pinned dark (docs/reading-dark-mode.md, rule 3)'
  }
  $toggle
}

<#
.SYNOPSIS
The page behind the plain-text body, and the ink over its first lines.
#>
function Get-PlainPage {
  $r = Open-PlainBody
  # Inset well past the ScrollViewer's own edge chrome, and below the first lines of text, so the
  # page band is the page and nothing but the page.
  $inset = ConvertTo-UiaPixels 40
  $page = Get-BandLuminance ([int]($r.X + $inset)) ([int]($r.Y + $r.Height - $inset)) `
    ([int][Math]::Min(240, $r.Width - (2 * $inset))) 24
  # The body's first lines: the ScrollViewer pads 16, and the text starts at its top left.
  $pad = ConvertTo-UiaPixels 16
  $ink = Get-BandLuminance ([int]($r.X + $pad)) ([int]($r.Y + $pad)) `
    ([int][Math]::Min(400, $r.Width - (2 * $pad))) ([int](ConvertTo-UiaPixels 44))
  [pscustomobject]@{ Page = $page; Ink = $ink }
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
          'MAILCAL_APPEARANCE. In a light window there is no sun/moon button and every page is ' +
          'white, so every case below would pass having proved nothing')
      }
    },
    @{
      Name = 'with dark messages on, the page is dark and the ink on it light'
      Body = {
        $sample = Get-PlainPage
        Assert-True ($sample.Page.Median -lt $CanvasMidLuminance) (
          "the page behind a plain-text body sampled at $($sample.Page.Median) in a dark window " +
          'with dark messages on. The page follows the scheme (docs/reading-dark-mode.md, rule 5), ' +
          'set in ReadingView.Scheme.cs from message_canvas')
        Assert-True ($sample.Ink.Lightest -gt $CanvasLightInkFloor) (
          "the lightest pixel over the message's first lines is $($sample.Ink.Lightest), so the " +
          'text is dark on a dark page: BodyArea.RequestedTheme is not following the page')
      }
    },
    @{
      Name = 'the button opens the trailing group, before archive'
      Body = {
        $null = Open-PlainBody
        $toggle = Get-RenderedBounds -Element (Get-SchemeToggle) -What 'the sun/moon button'
        $forward = Get-RenderedBounds -Element (Find-UiaElement -AutomationId 'ReadingForward') -What 'Forward'
        $archive = Get-RenderedBounds -Element (Find-UiaElement -AutomationId 'ReadingArchive') -What 'Archive'
        Assert-True ($forward.X -lt $toggle.X -and $toggle.X -lt $archive.X) (
          "Forward at $($forward.X), the button at $($toggle.X), Archive at $($archive.X): the " +
          'button sits immediately before archive and delete (docs/reading-actions.md)')
      }
    },
    @{
      Name = 'the sun sends the message to its light page, painted, with ink resolved for it'
      Body = {
        $null = Open-PlainBody
        $toggle = Get-SchemeToggle
        $sun = $toggle.Current.Name
        Invoke-UiaElement $toggle
        try {
          $sample = Get-PlainPage
          Assert-True ($sample.Page.Median -gt $CanvasPageFloor) (
            "after the sun, the body area sampled at $($sample.Page.Median) in a dark window, so " +
            'it is the pane''s own background showing through: BodyArea.Background is not the ' +
            'core''s light canvas (ReadingView.Scheme.cs)')
          Assert-True ($sample.Ink.Darkest -lt $CanvasInkCeiling) (
            "the darkest pixel over the message's first lines is $($sample.Ink.Darkest), so there " +
            'is no ink on the white page: the text resolved the dark window''s label colour')
          $moon = (Get-SchemeToggle).Current.Name
          Assert-True ($moon -ne $sun) "the button still reads '$sun' after it was pressed"
        } finally {
          # The preference persists: put dark messages back on for whatever runs next.
          Invoke-UiaElement (Get-SchemeToggle)
        }
      }
    }
  )
}
