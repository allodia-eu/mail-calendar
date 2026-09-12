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
# And the caption is where the search field lives, which is the rest of this file (docs/search.md,
# "Where the field lives"). That is a placement rule, so it is geometry: a field merely PRESENT in
# the caption, drawn hard against one edge, satisfies every lookup and none of the rule. New Mail
# is NOT here, deliberately, it is at the head of the actions bar under the caption
# (SelectionBar.Tests.ps1), where a collapsed folder pane cannot take it off screen.
#
# Dataset is `showcase`: these are rules about markup and rendering, no mail action is dispatched,
# and the pinned window frame keeps the caption's geometry predictable for the pixel sample.

# Where to sample the caption: the gap between the search field and the caption buttons, which is
# empty drag region in every locale, so no glyph can land in the sample and drag the median. Y is
# inside the 48px caption at any scale factor.
#
# Measured from the field rather than taken as a fraction of the window, because the field is
# centred and its width follows the window: a fixed fraction is over bare caption at one size and
# over the field at another, and a sample taken over a control reads that control's fill rather
# than the caption's. The light/dark rule below would then be asserting the wrong surface.
$SampleWidthPx = 40
$SampleGapPx = 24
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
  $field = Get-RenderedBounds -Element (Get-CaptionSearchField) -What 'the search field'
  $x = [int]($field.Right + $SampleGapPx)
  $y = [int]($r.Y + $SampleTop)
  if (($x + $SampleWidthPx) -gt $r.Right) {
    throw "no bare caption to sample: the search field ends at $($field.Right) and the window at $($r.Right)"
  }

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
  $item = Find-UiaElement -Name 'All Accounts' -Type 'ListItem'
  if (-not $item) { throw 'the sidebar has no "All Accounts" entry, the shell is not showing' }
  $item.Current.BoundingRectangle.Width
}

# The search field. A SIBLING of the TitleBar control rather than its content (MainWindow.xaml says
# why), so it is looked up on the window and not inside the caption control.
function Get-CaptionSearchField {
  $field = Find-UiaElement -AutomationId 'SearchBox'
  if (-not $field) { throw 'the window''s top row carries no search field (docs/search.md)' }
  $field
}

$Suite = @{
  Dataset = 'showcase'
  # Follow the desktop, whatever this developer's stored appearance is, see the header.
  Env     = @{ MAILCAL_APPEARANCE = 'system' }
  Cases   = @(
    @{
      Name = 'the caption is the app''s own TitleBar, carrying the app name'
      Body = {
        # The name THIS build was given, never the branded literal. The app's name is injected
        # (docs/branding.md), so a hardcoded 'Allodia Mail & Calendar' asserts against a build only
        # the brand owner makes: CI and every fork are unbranded and their caption reads 'MailCal'.
        # It failed there exactly once, naming the caption rather than the branding, which is a full
        # CI run spent on a test that was wrong rather than an app that was.
        $expected = Get-BrandAppTitle
        $bar = Get-AppTitleBar
        $title = @(Get-UiaTree $bar | Where-Object { $_.Current.Name -eq $expected })
        Assert-GreaterThan 0 $title.Count (
          "the caption must name the app ('$expected'), it is what the user reads to tell one " +
          'window from another in Alt-Tab and on the taskbar preview')
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
        $buttons = @(Find-UiaElements -Type 'Button' -Root $bar | ForEach-Object { $_.Current.AutomationId })
        Assert-Equal @('PART_PaneToggleButton') $buttons (
          'the caption control carries exactly one button: the pane toggle. The system''s ' +
          'minimize / maximize / close live outside it, and the search field beside it is not a ' +
          "button, so anything else here is a stray affordance. Found: $($buttons -join ' | ')")
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
        # By id, not "the first button in the caption": New Mail is in there too now, and a
        # positional lookup would silently start toggling the composer open instead.
        $toggle = Find-UiaElement -Type 'Button' -Root (Get-AppTitleBar) -AutomationId 'PART_PaneToggleButton'
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
    },
    @{
      Name = 'the search field is centred over the WINDOW, not over a pane'
      Body = {
        # docs/search.md: the default scope is every account and every folder, so a field drawn
        # over the message list was a control whose reach was wider than the column it sat on.
        # Centred over the window is the honest placement, and it is the whole of the rule: a
        # field merely present in the caption would pass a lookup and fail the reader.
        $window = (Get-MailcalWindow).Current.BoundingRectangle
        $field = Get-RenderedBounds -Element (Get-CaptionSearchField) -What 'the search field'
        $fieldCentre = $field.Left + ($field.Width / 2)
        $windowCentre = $window.X + ($window.Width / 2)
        # Tight, because this is centred on the glass and not on what something else leaves over:
        # the field is a sibling of the caption control rather than its content, for exactly that
        # reason. The TitleBar's own centre slot is off by tens of pixels, which is what this
        # number is set to catch.
        Assert-True ([Math]::Abs($fieldCentre - $windowCentre) -le 2) `
          "the search field's centre ($fieldCentre) must be the window's ($windowCentre): it belongs to the window, not to the message list or to the room the caption buttons leave"

        # And the message list no longer carries one of its own: two fields for one query is worse
        # than either placement.
        $list = Find-UiaElement -AutomationId 'RowsList'
        Assert-True ($null -eq (Find-UiaElement -Root $list -Type 'Edit')) `
          'the message list must not draw a search field of its own as well'
      }
    }
  )
}
