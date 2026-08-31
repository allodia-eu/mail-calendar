#!/usr/bin/env pwsh
# The window's caption, which was drawn by the SYSTEM until 2026-08-03, and therefore did not read
# the app's theme: on a dark-mode desktop the app came up dark with a pale strip across the top of
# it. Nothing in this repo could see that. `Mailcal.Tests` links no WinUI; `cargo test` sees no
# client at all; and the caption is the one part of the window a screenshot reviewer's eye slides
# straight past, because every OTHER window on the desktop has one too.
#
# So the rules here are the ones that would have been red on that build:
#
#   * the caption is drawn by the APP (a WinUI TitleBar control in the content tree), not by the
#     system, the structural fact everything else depends on;
#   * its background is on the same side of the light/dark divide as the desktop's own app mode.
#     This is the defect itself, and it is a PIXEL rule: UIA cannot see colour, so this samples the
#     rendered caption. Read the mode from the registry rather than pinning "dark", so the suite
#     asserts the same invariant on a light-mode box instead of inverting into a false failure.
#     The run is pinned to MAILCAL_APPEARANCE=system, because the app now HAS a light/dark setting
#     of its own (docs/settings.md -> General) and a developer who has set it would otherwise fail
#     this case for the right reason under the wrong name. Appearance.Tests.ps1 owns that setting;
#     this file is about the caption following whatever the app resolved to.
#   * the pane toggle moved INTO the caption and still works. A forwarded event handler
#     (PaneToggleRequested -> Nav.IsPaneOpen) is exactly the kind of wiring that compiles, renders,
#     and does nothing, the class of bug this whole suite exists for.
#
# Dataset is `showcase`: these are rules about markup and rendering, no mail action is dispatched,
# and the pinned window frame keeps the caption's geometry predictable for the pixel sample.

# Where to sample the caption. A fraction of the window's width, deliberately to the RIGHT of the
# icon/title and to the LEFT of the caption buttons, that span is empty drag region in every
# locale, so no glyph can land in the sample and drag the median. Y is inside the 48px caption at
# any scale factor.
$SampleXFraction = 0.55
$SampleWidthPx = 120
$SampleTop = 12
$SampleHeight = 8

# The midpoint of the 0..255 luminance range. Nothing here is near it: dark Mica renders around 30
# and light Mica around 240, so a bar on the wrong side is off by two hundred, not by rounding.
$MidLuminance = 128

<#
.SYNOPSIS
The median luminance (0..255) of a band of screen pixels inside the running window's caption.
.DESCRIPTION
A screen grab rather than PrintWindow: PrintWindow asks the app to re-render, and the system caption
buttons are not the app's to draw, so a printed frame is not what the user is looking at. The median
(not the mean) so that a stray antialiased pixel cannot move the answer.
#>
function Get-CaptionLuminance {
  Add-Type -AssemblyName System.Drawing
  $window = Get-MailcalWindow
  if (-not $window) { throw 'no Mailcal window, the dataset should have launched one' }
  $r = $window.Current.BoundingRectangle
  $x = [int]($r.X + ($r.Width * $SampleXFraction))
  $y = [int]($r.Y + $SampleTop)

  $bmp = New-Object System.Drawing.Bitmap $SampleWidthPx, $SampleHeight
  try {
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    try { $g.CopyFromScreen($x, $y, 0, 0, $bmp.Size) } finally { $g.Dispose() }
    $lums = for ($px = 0; $px -lt $SampleWidthPx; $px++) {
      for ($py = 0; $py -lt $SampleHeight; $py++) {
        $c = $bmp.GetPixel($px, $py)
        # Rec. 601 luma, the same weighting a human eye applies, so "is this bar light or dark"
        # means what it says rather than "is its blue channel high".
        (0.299 * $c.R) + (0.587 * $c.G) + (0.114 * $c.B)
      }
    }
    ($lums | Sort-Object)[[int]($lums.Count / 2)]
  }
  finally { $bmp.Dispose() }
}

<#
.SYNOPSIS
$true when the desktop is in dark app mode.
.DESCRIPTION
AppsUseLightTheme is the value Windows itself flips on its light/dark schedule, and the one WinUI
resolves ActualTheme from when an app states no preference, which is what MAILCAL_APPEARANCE=system
pins this run to. Absent means light: that is the documented default when the key has never been
written.
#>
function Test-DesktopDarkMode {
  $key = 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize'
  $value = (Get-ItemProperty -Path $key -Name AppsUseLightTheme -ErrorAction SilentlyContinue).AppsUseLightTheme
  return $value -eq 0
}

<#
.SYNOPSIS
The caption element, or a throw naming what is missing.
.DESCRIPTION
Never inline `Find-UiaElement -Root $bar` without this. Find-UiaElement falls back to the WHOLE
WINDOW when -Root is $null, so on a build with no caption control a scoped search silently widens to
an unscoped one and finds the NavigationView's own toggle instead, which is how the pane-toggle
case below passed against the very build it was written to reject. A missing caption has to stop the
case, not quietly re-aim it.
#>
function Get-AppTitleBar {
  $bar = Find-UiaElement -AutomationId 'AppTitleBar'
  if (-not $bar) {
    throw 'no #AppTitleBar in the content tree, the window''s caption must be a WinUI TitleBar ' +
      'control. Without it the system draws the caption on a surface that does not read the app''s ' +
      'theme, which is how a dark app shipped with a pale strip across the top of it'
  }
  $bar
}

# The sidebar's first entry, whose width is how the pane's open/collapsed state is observed: an open
# pane lays it out across the full OpenPaneLength, a collapsed one shrinks it to the icon strip.
function Get-PaneItemWidth {
  $item = Find-UiaElement -Name 'All Inboxes' -Type 'ListItem'
  if (-not $item) { throw 'the sidebar has no "All Inboxes" entry, the shell is not showing' }
  $item.Current.BoundingRectangle.Width
}

$Suite = @{
  Dataset = 'showcase'
  # Follow the desktop, whatever this developer's stored appearance is, see the header.
  Env     = @{ MAILCAL_APPEARANCE = 'system' }
  Cases   = @(
    @{
      Name = 'the caption is the app''s own TitleBar, carrying the app name'
      Body = {
        $bar = Get-AppTitleBar
        $title = @(Get-UiaTree $bar |
          Where-Object { $_.Current.Name -eq 'Allodia Mail & Calendar' })
        Assert-GreaterThan 0 $title.Count (
          'the caption must name the app, it is what the user reads to tell one window from ' +
          'another in Alt-Tab and on the taskbar preview')
      }
    },
    @{
      Name = 'the caption is drawn in the desktop''s own light/dark mode'
      Body = {
        $dark = Test-DesktopDarkMode
        $lum = Get-CaptionLuminance
        $mode = if ($dark) { 'dark' } else { 'light' }
        if ($dark) {
          Assert-True ($lum -lt $MidLuminance) (
            "the desktop is in $mode mode but the caption sampled at luminance $lum, i.e. it is " +
            'light. This is the regression this file was written for: a system-drawn caption keeps ' +
            'its own theme, so the app renders dark with a pale bar above it')
        }
        else {
          Assert-True ($lum -gt $MidLuminance) (
            "the desktop is in $mode mode but the caption sampled at luminance $lum, i.e. it is " +
            'dark. The caption must follow the desktop''s app mode in BOTH directions')
        }
      }
    },
    @{
      Name = 'the pane toggle lives in the caption, and nowhere else'
      Body = {
        $bar = Get-AppTitleBar
        $buttons = @(Find-UiaElements -Type 'Button' -Root $bar)
        Assert-Equal 1 $buttons.Count (
          'the caption carries exactly one button: the pane toggle. The system''s minimize / ' +
          'maximize / close live outside the control, so a second one here means a stray affordance')
        # The NavigationView's own toggle is hidden (IsPaneToggleButtonVisible="False"), which is
        # the Fluent guidance once a custom title bar exists. Two hamburgers is the failure mode.
        $navToggle = Find-UiaElement -AutomationId 'TogglePaneButton'
        Assert-True ($null -eq $navToggle) (
          'the NavigationView must not draw its own pane toggle as well, the caption''s is the ' +
          'one the user sees, and a second one below it reads as a different control')
      }
    },
    @{
      Name = 'the caption''s pane toggle collapses and reopens the sidebar'
      Body = {
        $bar = Get-AppTitleBar
        $toggle = Find-UiaElement -Type 'Button' -Root $bar
        Assert-True ($null -ne $toggle) 'the caption must carry a pane toggle'

        $open = Get-PaneItemWidth
        Invoke-UiaElement $toggle
        $collapsed = Get-PaneItemWidth
        Assert-True ($collapsed -lt $open) (
          "the sidebar entry measured $collapsed px collapsed against $open px open, the toggle " +
          'moved out of the NavigationView into the caption, so its PaneToggleRequested handler is ' +
          'now the only thing that closes the pane. A handler that is never wired compiles, ' +
          'renders, and does nothing')

        Invoke-UiaElement $toggle
        $reopened = Get-PaneItemWidth
        Assert-Equal $open $reopened (
          'toggling twice must return the sidebar to where it started, the toggle flips a state, ' +
          'it does not set one')
      }
    }
  )
}
