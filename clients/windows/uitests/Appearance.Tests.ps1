#!/usr/bin/env pwsh
# The app-level light/dark setting (docs/settings.md → General). Three things have to be true and
# none of them can be seen without a rendered window:
#
#   * the choice REACHES the window. The core persists it and hands it over, but painting it is a
#     `RequestedTheme` assignment on the content root, the class of wiring that compiles, renders,
#     and does nothing, which is exactly what this suite exists for.
#   * picking one repaints AT ONCE. The core signals only `Surface::Settings` for the appearance
#     (it computes nothing from it), so unlike every other display setting there is no snapshot
#     reload to carry the change, `SettingsDialog` calls `MainWindow.ApplyAppearance` itself. Cut
#     that one line and the picker still moves, still persists, and the window stays as it was.
#   * "Use system setting" really hands the window BACK. The failure here is one-directional and
#     silent: `ElementTheme.Default` is what makes it work, and pinning the theme any other way
#     (`Application.RequestedTheme`, say) leaves System unreachable until the next launch.
#
# THE RUN IS PINNED TO THE OPPOSITE of whatever this machine is set to, via MAILCAL_APPEARANCE,
# the same override the showcase script uses to photograph both themes. That is what makes every
# assertion below mean something on any host: if the suite simply asserted "dark", it would pass on
# a dark-mode box while doing nothing at all, since the app would have looked that way regardless.
#
# Colour is a PIXEL rule, UIA cannot see it, so the caption is sampled the same way
# TitleBar.Tests.ps1 samples it, and for the same reason: the caption is the one surface that spans
# the app's whole theme and has no text of its own in the sampled band.
#
# Dataset is `showcase`: nothing here dispatches a mail action, and it has no store, so a pick made
# below is never written to the developer's own preferences.

Add-Type -AssemblyName System.Drawing

# Where to sample the caption, a band of empty drag region to the right of the title and left of
# the caption buttons, inside the 48px caption at any scale factor. Kept identical to
# TitleBar.Tests.ps1 so the two files cannot disagree about what "the caption" is.
$SampleXFraction = 0.55
$SampleWidthPx = 120
$SampleTop = 12
$SampleHeight = 8

# Nothing here is near the midpoint: dark Mica renders around 20-30 and light around 170-240, so a
# window on the wrong side is off by a hundred and more, not by rounding.
$MidLuminance = 128

function Test-DesktopDarkMode {
  $key = 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize'
  $value = (Get-ItemProperty -Path $key -Name AppsUseLightTheme -ErrorAction SilentlyContinue).AppsUseLightTheme
  return $value -eq 0
}

# The scheme this run pins the app to: whichever one the desktop is NOT in. Evaluated at load time,
# because the runner reads $Suite.Env before it launches anything.
$DesktopDark = Test-DesktopDarkMode
$Opposite = if ($DesktopDark) { 'light' } else { 'dark' }

<#
.SYNOPSIS
The median luminance (0..255) of a band of screen pixels inside the running window's caption.
#>
function Get-CaptionLuminance {
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
        # Rec. 601 luma, what a human eye weights, so "is this light or dark" means what it says.
        (0.299 * $c.R) + (0.587 * $c.G) + (0.114 * $c.B)
      }
    }
    ($lums | Sort-Object)[[int]($lums.Count / 2)]
  }
  finally { $bmp.Dispose() }
}

function Assert-WindowIsDark {
  param([bool] $Dark, [string] $Because)
  $lum = Get-CaptionLuminance
  $side = if ($lum -lt $MidLuminance) { 'dark' } else { 'light' }
  $want = if ($Dark) { 'dark' } else { 'light' }
  Assert-True (($lum -lt $MidLuminance) -eq $Dark) `
    "the window sampled at luminance $lum, i.e. $side, but must be $want, $Because"
}

<#
.SYNOPSIS
The median luminance of an empty band in the Settings dialog's title row.
#>
function Get-SettingsLuminance {
  # Find-, not Get-: this asserts on the dialog's OWN theme, so it must read the one already
  # open rather than open a fresh one (uia.ps1).
  $dialog = Find-SettingsDialog
  if (-not $dialog) { throw 'Settings is not open' }
  $r = $dialog.Current.BoundingRectangle
  $width = 100
  $height = 8
  $x = [int]($r.X + ($r.Width * 0.62))
  $y = [int]($r.Y + 18)

  $bmp = New-Object System.Drawing.Bitmap $width, $height
  try {
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    try { $g.CopyFromScreen($x, $y, 0, 0, $bmp.Size) } finally { $g.Dispose() }
    $lums = for ($px = 0; $px -lt $width; $px++) {
      for ($py = 0; $py -lt $height; $py++) {
        $c = $bmp.GetPixel($px, $py)
        (0.299 * $c.R) + (0.587 * $c.G) + (0.114 * $c.B)
      }
    }
    ($lums | Sort-Object)[[int]($lums.Count / 2)]
  }
  finally { $bmp.Dispose() }
}

function Assert-SettingsIsDark {
  param([bool] $Dark, [string] $Because)
  $lum = Get-SettingsLuminance
  $side = if ($lum -lt $MidLuminance) { 'dark' } else { 'light' }
  $want = if ($Dark) { 'dark' } else { 'light' }
  Assert-True (($lum -lt $MidLuminance) -eq $Dark) `
    "Settings sampled at luminance $lum, i.e. $side, but must be $want, $Because"
}

<#
.SYNOPSIS
Opens Settings → General if it is not already showing, and returns the three appearance radios.
.DESCRIPTION
The NavigationView's settings entry exposes SelectionItem rather than Invoke, which is what
Invoke-UiaElement falls back to, NavigationView raises ItemInvoked for it either way. The dialog is
modal, so once a case has opened it the later ones find it already up.
#>
function Get-AppearanceRadios {
  if (-not (Find-UiaElement -Name 'Use system setting' -Type RadioButton)) {
    $entry = Find-UiaElement -Name 'Settings' -Type ListItem
    if (-not $entry) { throw 'the sidebar has no Settings entry, the shell is not showing' }
    Invoke-UiaElement $entry
  }
  $radios = @{}
  foreach ($name in 'Use system setting', 'Light', 'Dark') {
    $el = Find-UiaElement -Name $name -Type RadioButton
    if (-not $el) {
      throw "Settings → General offers no '$name' appearance choice. All three are the contract " +
        '(docs/settings.md → General): follow the host, or override it either way'
    }
    $radios[$name] = $el
  }
  $radios
}

$Suite = @{
  Dataset = 'showcase'
  # The whole point: pin the run to the scheme this machine is NOT in, so every case below is
  # measuring the app's own decision rather than the desktop's.
  Env     = @{ MAILCAL_APPEARANCE = $Opposite }
  Cases   = @(
    @{
      Name = 'the app paints itself in its own appearance, not the desktop''s'
      Body = {
        Assert-WindowIsDark (-not $DesktopDark) (
          "this run is pinned to '$Opposite' while the desktop is in " +
          "$(if ($DesktopDark) { 'dark' } else { 'light' }) mode. If the two match, the setting is " +
          'not reaching the content root at all and the window is simply inheriting the host')
      }
    },
    @{
      Name = 'Settings → General offers follow-the-system, Light and Dark'
      Body = {
        $radios = Get-AppearanceRadios
        Assert-Equal 3 $radios.Count 'all three appearance choices must be on the General panel'
      }
    },
    @{
      Name = 'picking an appearance repaints the window at once, both ways'
      Body = {
        $radios = Get-AppearanceRadios
        $toDesktop = if ($DesktopDark) { 'Dark' } else { 'Light' }
        $toOpposite = if ($DesktopDark) { 'Light' } else { 'Dark' }

        Invoke-UiaElement $radios[$toDesktop]
        Assert-WindowIsDark $DesktopDark (
          "picking '$toDesktop' left the window as it was. The core signals only Settings for the " +
          'appearance, so nothing reloads a snapshot on the client''s behalf, the dialog has to ' +
          'repaint the window itself, and a missing call there is invisible to every other gate')
        Assert-SettingsIsDark $DesktopDark (
          'the open popup has to follow the content root instead of keeping the theme it copied ' +
          'when it opened')

        Invoke-UiaElement $radios[$toOpposite]
        Assert-WindowIsDark (-not $DesktopDark) (
          "picking '$toOpposite' must move it back, a repaint that only ever runs in one " +
          'direction still passes a single-direction test')
        Assert-SettingsIsDark (-not $DesktopDark) (
          'the popup has to stay synchronized in both directions')
      }
    },
    @{
      Name = '"Use system setting" hands the window back to the desktop'
      Body = {
        $radios = Get-AppearanceRadios
        Invoke-UiaElement $radios['Use system setting']
        Assert-WindowIsDark $DesktopDark (
          'following the host has to be reachable while the app is running. It is why the theme is ' +
          'set on the content ROOT (ElementTheme.Default) rather than pinned on the Application, ' +
          'which can only be set once before any content exists, that would leave this choice ' +
          'unreachable until the next launch, and nothing but a rendered window would say so')
        Assert-SettingsIsDark $DesktopDark (
          'a popup that copied the previous explicit theme would not follow the host')
      }
    }
  )
}
