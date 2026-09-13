#!/usr/bin/env pwsh
# The invitation card is legible in the appearance the APP is in, not the one the desktop is in.
#
# The card is built in code, so its colours are resolved once per draw rather than tracked by the
# framework, and the obvious way to resolve them is wrong: `Application.Current.Resources` answers
# for the theme the *application* is in, which is the desktop's and is fixed before any window
# exists, while the appearance setting is applied to the content root. Pin the app to light on a
# dark desktop and every label, the organiser row, the description and the attendee tally came back
# in near-white on a light card (docs/client-traps.md; `Services/ThemePalette.cs`).
#
# WHY NOTHING ELSE SEES IT. The card renders perfectly: correct text, correct layout, correct
# language. UIA reports the same tree either way, because a foreground brush is not an automation
# property, so every assertion in the suites beside this one passes over an unreadable card. A
# screenshot passes too, which is how it reached a store capture set: the size floor only rejects a
# blank frame. Appearance.Tests samples the CAPTION, which is XAML and was always right, and that is
# the trap, a green appearance suite over a broken card.
#
# THE RUN IS PINNED TO THE OPPOSITE of whatever this machine is set to, exactly as
# Appearance.Tests.ps1 is and for the same reason: the bug only exists where the two disagree, so a
# suite that took the desktop's own scheme would pass everywhere while measuring nothing.
#
# Colour is a PIXEL rule. What is asserted is not a value but a RELATION, that each line of text on
# the card contrasts with the card behind it, and on the side the theme demands: ink darker than its
# ground in light, lighter in dark. That is the property a reader actually needs, it needs no
# Fluent constant repeated here, and it survives a palette change. The original failure sat at a
# difference of 4 with the sign inverted, so the margin below is not a near thing.
#
# NOTHING HERE MATCHES A LOCALISED STRING. The card is reached through its preview Expander, found
# by ClassName, and the lines are read as Text nodes rather than by content, so this suite says the
# same thing in every language the catalog ships.

Add-Type -AssemblyName System.Drawing

# The English subject of the showcase's seeded invitation. Safe to name, and only here: the
# showcase dataset pins Locale = 'en' (dataset.ps1), so this run's seed is the English one.
$InvitationSubject = '*Invitation*'

# A glyph's core has to differ from its ground by at least this much luma (0..255). Body text at
# this size renders a solid core, so the gap between ink and paper is well over a hundred when it
# is right; anything under this is either a brush from the wrong theme or text that is not there.
$MinContrast = 40

function Test-DesktopDarkMode {
  $key = 'HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize'
  $value = (Get-ItemProperty -Path $key -Name AppsUseLightTheme -ErrorAction SilentlyContinue).AppsUseLightTheme
  return $value -eq 0
}

# Evaluated at load time: the runner reads $Suite.Env before it launches anything.
$DesktopDark = Test-DesktopDarkMode
$Opposite = if ($DesktopDark) { 'light' } else { 'dark' }
$WantDarkApp = -not $DesktopDark

<#
.SYNOPSIS
The invitation card's StackPanel, reached from the preview Expander it holds.
.DESCRIPTION
The Expander is the one language-free handle on the card (InvitationPreview.Tests.ps1 uses it for
the same reason), and the card's stack is its parent: every line this suite measures is a sibling
of it.
#>
function Get-InvitationStack {
  Invoke-UiaElement (Get-MailRowByTitle $InvitationSubject)
  $cond = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ClassNameProperty, 'Microsoft.UI.Xaml.Controls.Expander')
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($watch.Elapsed.TotalSeconds -lt 30) {
    $found = @((Get-MailcalWindow).FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond))
    if ($found.Count -gt 1) {
      throw "expected one Expander in the reading pane, found $($found.Count). Something else is open over it, and the lines measured below would be that surface's, which is a PASS for the wrong element rather than a failure."
    }
    if ($found.Count -eq 1) {
      return @{
        Stack    = [System.Windows.Automation.TreeWalker]::RawViewWalker.GetParent($found[0])
        Expander = $found[0]
      }
    }
    Start-Sleep -Milliseconds 300
  }
  throw "the showcase invitation showed no card. The showcase primes the calendar at boot so the preview opens expanded (boot/inmemory.rs); if that stopped happening the card is correctly withheld and says so, which is a different failure from this one."
}

<#
.SYNOPSIS
The lines of the card itself: its Text nodes, minus the preview's own.
.DESCRIPTION
The preview is excluded deliberately. Its event blocks are light text on a saturated fill in BOTH
themes, so they answer the contrast question with the opposite sign and would make the rule below
mean nothing.
#>
function Get-CardLines {
  param([Parameter(Mandatory)] [hashtable] $Card)
  $skip = $Card.Expander.Current.BoundingRectangle
  Get-UiaTree $Card.Stack | Where-Object {
    $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text -and
    $_.Current.Name -and
    $_.Current.Name.Trim() -and
    -not $_.Current.IsOffscreen -and
    (Test-DrawnRect $_.Current.BoundingRectangle) -and
    -not $skip.Contains($_.Current.BoundingRectangle.X, $_.Current.BoundingRectangle.Y)
  }
}

<#
.SYNOPSIS
Whether a BoundingRectangle names real pixels on a screen.
.DESCRIPTION
An element that is not laid out reports Rect.Empty, whose edges are infinite, and casting one of
those to an int throws somewhere far from the cause. A collapsed line is not this suite's business
either way: it has no ink to read.
#>
function Test-DrawnRect {
  param([Parameter(Mandatory)] [object] $Rect)
  foreach ($v in $Rect.X, $Rect.Y, $Rect.Width, $Rect.Height) {
    if ([double]::IsNaN($v) -or [double]::IsInfinity($v)) { return $false }
  }
  return ($Rect.Width -ge 4) -and ($Rect.Height -ge 4)
}

<#
.SYNOPSIS
The ink/ground luma pair for one line of text, read off the screen.
.DESCRIPTION
Most pixels in a text rectangle are ground, so the median IS the ground; the ink is whichever
extreme lies further from it. Returns both, plus the signed difference (ink - ground), so the
caller can assert the SIDE as well as the size.
#>
function Get-LineInk {
  param([Parameter(Mandatory)] [object] $Line)
  $r = $Line.Current.BoundingRectangle
  if (-not (Test-DrawnRect $r)) { return $null }
  $w = [int] $r.Width
  $h = [int] $r.Height

  $bmp = New-Object System.Drawing.Bitmap $w, $h
  try {
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    try { $g.CopyFromScreen([int] $r.X, [int] $r.Y, 0, 0, $bmp.Size) } finally { $g.Dispose() }
    $lums = for ($px = 0; $px -lt $w; $px++) {
      for ($py = 0; $py -lt $h; $py++) {
        $c = $bmp.GetPixel($px, $py)
        # Rec. 601 luma, what a human eye weights, so "lighter" and "darker" mean what they say.
        (0.299 * $c.R) + (0.587 * $c.G) + (0.114 * $c.B)
      }
    }
    $sorted = @($lums | Sort-Object)
    $ground = $sorted[[int]($sorted.Count / 2)]
    $low = $sorted[0]
    $high = $sorted[$sorted.Count - 1]
    $ink = if (($ground - $low) -ge ($high - $ground)) { $low } else { $high }
    return [pscustomobject]@{ Ink = $ink; Ground = $ground; Delta = $ink - $ground }
  }
  finally { $bmp.Dispose() }
}

$Suite = @{
  Dataset = 'showcase'
  # The whole point: pin the run to the scheme this machine is NOT in, so the card is drawn from a
  # theme the application-level resources do not hold.
  Env     = @{ MAILCAL_APPEARANCE = $Opposite }
  # A relaunch, because this suite shares its launch with Appearance.Tests, which runs first and
  # whose job is to MOVE the appearance: its last case hands the window back to the desktop and
  # leaves Settings open over the reading pane. Inheriting that costs this suite both halves of
  # what it measures, the window is no longer pinned away from the application's theme, and the
  # lines it finds are the sidebar's. Neither reads as a broken precondition: the first makes the
  # suite vacuous and the second makes it green against the wrong element.
  Prepare = { Start-Dataset -Dataset 'showcase' }
  Cases   = @(
    @{
      Name = 'every line of the invitation card has ink against its own ground'
      Body = {
        $card = Get-InvitationStack
        $lines = @(Get-CardLines $card)
        Assert-True ($lines.Count -ge 4) (
          "the card should carry at least the title and the organiser/when/where rows, found $($lines.Count) line(s); " +
          'if the card stopped being built this suite is measuring the wrong element')
        foreach ($line in $lines) {
          $read = Get-LineInk $line
          if (-not $read) { continue }
          Assert-True ([Math]::Abs($read.Delta) -ge $MinContrast) (
            "'$($line.Current.Name)' is drawn at luma $($read.Ink) on a ground of $($read.Ground), " +
            "a difference of $([Math]::Abs($read.Delta)) against a floor of $MinContrast. " +
            'That is a colour resolved from the wrong theme: the line is there, correctly worded ' +
            'and correctly placed, and cannot be read')
        }
      }
    },
    @{
      Name = 'and the ink is on the side this appearance demands, not the desktop''s'
      Body = {
        $card = Get-InvitationStack
        $want = if ($WantDarkApp) { 'lighter' } else { 'darker' }
        foreach ($line in @(Get-CardLines $card)) {
          $read = Get-LineInk $line
          if (-not $read) { continue }
          $isLighter = $read.Delta -gt 0
          Assert-True ($isLighter -eq $WantDarkApp) (
            "this run is pinned to '$Opposite' while the desktop is in " +
            "$(if ($DesktopDark) { 'dark' } else { 'light' }) mode, so card ink must be $want than its " +
            "ground; '$($line.Current.Name)' came back the other way (ink $($read.Ink), ground $($read.Ground)). " +
            'That is the application theme leaking into a code-built surface')
        }
      }
    }
  )
}
