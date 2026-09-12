#!/usr/bin/env pwsh
# The reading pane's action row and the overflow menu at the end of it (docs/reading-actions.md):
# the overflow sits last, it is the same control as the buttons beside it AT BOTH of the row's
# widths, and the export behind it writes the message that arrived.
#
# THE REGRESSION. The overflow is the one control in the row carrying a glyph and no label, and the
# default button style centres its content rather than stretching it, so it came up at the glyph's
# height while its six labelled neighbours stood at their title's line box: 54px against 64px on a
# 200% display, vertically centred, on a row the contract says a reader should not be able to tell
# apart except by its glyph. It is the same defect `SelectionBar.Tests.ps1` guards in the actions
# bar, and it has the same fix (`VerticalAlignment="Stretch"`).
#
# WHY BOTH WIDTHS. The row collapses to icon-only on a narrow pane, and there every button is the
# glyph's height, so the overflow matches BY ACCIDENT and a single-width assertion passes over the
# defect. The labelled width is the one that shows it, and it is reached here by collapsing the
# navigation pane. A display too narrow to reach it FAILS rather than skipping: this suite would
# otherwise report green having measured the icon-only row twice.
#
# WHY THE HARNESS. The last case asserts on WRITTEN BYTES, and the showcase engine performs no real
# mail read, so the source it would hand over is fiction. `01-plain.eml` from the seeder is the
# comparison, CRLF-normalised because the server normalises line endings on delivery, so these are
# the server's bytes rather than the seeder's.
#
# WHY THIS SUITE AND NOT `Mailcal.Tests`. A rendered height and an unwired Click handler are both
# invisible to a plain net10.0 assembly: the markup compiles, the binding is correct, and the
# control is the wrong size (or reaches nothing) only once WinUI has measured and a person presses
# it. The name derivation and the byte fidelity are already unit-tested in `mailcal-app`; what is
# only provable here is that this client's menu item reaches them.

# The row in the order the markup lays it out, named rather than swept off the container so a
# button that stops rendering at all fails here instead of leaving the comparison to its siblings.
$RowButtons = @(
  'ReadingReply'
  'ReadingReplyAll'
  'ReadingForward'
  'ReadingArchive'
  'ReadingDelete'
  'ReadingOverflow'
)
# Physical pixels, as BoundingRectangle reports them. Slack for layout rounding, not for a design
# difference: the labelled buttons agree with each other exactly, and the defect was 10px.
$RowEdgeTolerancePx = 1.5
$RowPlainSubject = 'Harness baseline message'
# The seeder's copy of the message the export is compared against.
$RowSeedFixture = Join-Path $PSScriptRoot '..\..\..\docker\stalwart\seed\mail\01-plain.eml'

<#
.SYNOPSIS
Open the plain seeded message and return the rendered rectangle of each action-row button.
#>
function Get-RowButtonBounds {
  Invoke-UiaElement (Get-MailRowByTitle $RowPlainSubject)
  $null = Wait-UiaElement -AutomationId 'ReadingOverflow' -TimeoutSec 30
  $bounds = [ordered] @{}
  foreach ($id in $RowButtons) {
    $button = Find-UiaElement -AutomationId $id
    $bounds[$id] = Get-RenderedBounds -Element $button -What "the action row's #$id button"
  }
  $bounds
}

<#
.SYNOPSIS
$true while the row is drawing its labels, i.e. the wide variant.
.DESCRIPTION
Read the state off a LABEL rather than off a button width: a width threshold is a constant that
moves with the locale, and every label in this row is longer in Dutch than in English. A collapsed
label leaves the automation tree entirely, so its absence is the signal.
#>
function Test-RowLabelled {
  $label = Find-UiaElement -AutomationId 'ReplyLabel'
  if (-not $label) { return $false }
  $rect = $label.Current.BoundingRectangle
  (-not [double]::IsInfinity($rect.X)) -and $rect.Width -gt 0
}

<#
.SYNOPSIS
Try to put the row into its labelled or its icon-only variant. Returns whether it got there.
.DESCRIPTION
The lever is the navigation-pane toggle, which is all this suite has: the list|reading divider is a
pointer gesture, and UIA drives patterns, not pointers. Which variant a given pane width produces
depends on the display and the locale, so this reports rather than asserts, and flips back when it
cannot deliver: leaving the pane collapsed would cost the next suite a relaunch.
#>
function Set-RowVariant {
  param([Parameter(Mandatory)] [bool] $Labelled)
  if ((Test-RowLabelled) -eq $Labelled) { return $true }
  $toggle = Find-UiaElement -AutomationId 'PART_PaneToggleButton' -Type Button
  if (-not $toggle) { throw "no navigation-pane toggle in the caption, so neither of the row's widths can be selected" }
  Invoke-UiaElement $toggle
  Start-Sleep -Milliseconds 1500
  if ((Test-RowLabelled) -eq $Labelled) { return $true }
  Invoke-UiaElement $toggle
  Start-Sleep -Milliseconds 1500
  $false
}

<#
.SYNOPSIS
Assert the row reads as one row: one height, one top edge, one bottom edge.
#>
function Assert-RowIsOneRow {
  param([Parameter(Mandatory)] [object] $Bounds, [Parameter(Mandatory)] [string] $Width)
  $heights = $Bounds.Keys | ForEach-Object { "$_=$([math]::Round($Bounds[$_].Height, 1))" }
  $edges = $Bounds.Keys | ForEach-Object { "$_=$([math]::Round($Bounds[$_].Top, 1))..$([math]::Round($Bounds[$_].Bottom, 1))" }
  $tallest = ($Bounds.Values | ForEach-Object { $_.Height } | Measure-Object -Maximum).Maximum
  $shortest = ($Bounds.Values | ForEach-Object { $_.Height } | Measure-Object -Minimum).Minimum
  Assert-True (($tallest - $shortest) -le $RowEdgeTolerancePx) `
    "at the $Width width the action row's controls must all be the same height: the contract is that a reader cannot tell the overflow from its neighbours except by its glyph. Heights: $($heights -join ' | ')"
  # Equal heights alone would also be satisfied by a row of buttons stepping down the pane. These
  # two comparisons are what say it is one row.
  $tops = $Bounds.Values | ForEach-Object { $_.Top }
  $bottoms = $Bounds.Values | ForEach-Object { $_.Bottom }
  $topSpread = (($tops | Measure-Object -Maximum).Maximum) - (($tops | Measure-Object -Minimum).Minimum)
  $bottomSpread = (($bottoms | Measure-Object -Maximum).Maximum) - (($bottoms | Measure-Object -Minimum).Minimum)
  Assert-True ($topSpread -le $RowEdgeTolerancePx) `
    "at the $Width width the action row must share a top edge. Edges: $($edges -join ' | ')"
  Assert-True ($bottomSpread -le $RowEdgeTolerancePx) `
    "at the $Width width the action row must share a bottom edge. Edges: $($edges -join ' | ')"
}

$Suite = @{
  Dataset = 'harness'
  Cases   = @(
    @{
      Name = 'the overflow is last of all, after both mailbox actions'
      Body = {
        $bounds = Get-RowButtonBounds
        $overflow = $bounds['ReadingOverflow'].X
        foreach ($id in $RowButtons | Where-Object { $_ -ne 'ReadingOverflow' }) {
          Assert-True ($bounds[$id].X -lt $overflow) `
            "#$id is drawn at $($bounds[$id].X) and the overflow at $overflow. The overflow is the END of the row on every platform, which is what makes it findable without a label (docs/reading-actions.md)"
        }
      }
    },
    @{
      Name = 'the row is one row at the icon-only width'
      Body = {
        $null = Get-RowButtonBounds
        $reached = Set-RowVariant -Labelled $false
        try {
          Assert-True $reached `
            'the reading pane never reached its icon-only width, even with the navigation pane open, so this case measured the labelled row and proved nothing about the narrow one'
          Assert-RowIsOneRow -Bounds (Get-RowButtonBounds) -Width 'icon-only'
        } finally {
          $null = Set-RowVariant -Labelled $false
        }
      }
    },
    @{
      Name = 'the row is one row at the labelled width too, which is where a centred glyph shows'
      Body = {
        $null = Get-RowButtonBounds
        $reached = Set-RowVariant -Labelled $true
        try {
          # Loudly, not as a skip: measuring the icon-only row a second time is the false pass this
          # case exists to prevent, and it would report green having seen nothing.
          Assert-True $reached `
            'the reading pane never reached its labelled width, even with the navigation pane collapsed, so this case measured the icon-only row again and proved nothing about the defect it guards. Widen the window, or run it on a larger display'
          Assert-RowIsOneRow -Bounds (Get-RowButtonBounds) -Width 'labelled'
        } finally {
          $null = Set-RowVariant -Labelled $false
        }
      }
    },
    @{
      Name = 'the menu behind it offers the export, under a spoken name'
      Body = {
        $null = Get-RowButtonBounds
        $overflow = Find-UiaElement -AutomationId 'ReadingOverflow'
        # A spoken name, not WHICH one: the catalog string is localised, and this suite runs against
        # the harness under the host's own language. That it is not the automation id is the point,
        # a WinUI button derives no Name from an icon-only Content, and the fallback reads as the
        # id to a screen reader.
        Assert-True ($overflow.Current.Name -and $overflow.Current.Name -ne 'ReadingOverflow') `
          "the overflow carries no label at any width, so its accessible name is the only name a screen reader has; it announced '$($overflow.Current.Name)'"
        $overflow.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Expand()
        try {
          $item = Wait-UiaElement -AutomationId 'ReadingExportEml' -TimeoutSec 10
          Assert-True ($null -ne $item) 'expanding the overflow must offer the .eml export; the menu ships with that one item'
        } finally {
          $overflow.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Collapse()
          Start-Sleep -Milliseconds 500
        }
      }
    },
    @{
      Name = 'exporting writes the bytes that were delivered, at the path the picker was given'
      Body = {
        $null = Get-RowButtonBounds
        $destination = Join-Path $env:TEMP ("mailcal-uitest-export-" + [guid]::NewGuid().ToString('N') + ".eml")
        try {
          $overflow = Find-UiaElement -AutomationId 'ReadingOverflow'
          $overflow.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Expand()
          $item = Wait-UiaElement -AutomationId 'ReadingExportEml' -TimeoutSec 10
          if (-not $item) { throw 'the overflow offered no export item, so there was nothing to press' }
          $item.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
          $suggested = ''
          Save-ThroughFilePicker -Destination $destination -SuggestedName ([ref] $suggested)
          # The pre-filled name is the only evidence of WHICH string the client handed the core.
          # Passing the wrong field compiles, renders, saves the right bytes, and files them under
          # a name nobody can find again, so nothing else here would notice.
          Assert-Equal "$RowPlainSubject.eml" $suggested `
            'the picker must open on the name the core derived from the subject this pane displays (docs/reading-actions.md)'

          # Bytes, never existence, and not read the instant the dialog closes. The picker ITSELF
          # creates the file, so Test-Path is satisfied by a zero-length one, and the source is
          # fetched and copied in on a background thread afterwards. Wait for a length that holds
          # still, which is also the only way a half-written file would be caught rather than read.
          $settled = 0
          $previous = -1
          for ($i = 0; $i -lt 60; $i++) {
            $length = if (Test-Path $destination) { (Get-Item $destination).Length } else { -1 }
            if ($length -gt 0 -and $length -eq $previous) {
              $settled++
              if ($settled -ge 3) { break }
            } else {
              $settled = 0
            }
            $previous = $length
            Start-Sleep -Milliseconds 400
          }
          Assert-GreaterThan 0 (Get-Item $destination -ErrorAction SilentlyContinue).Length `
            "the export left nothing but the empty file the picker created at $destination. The core's own suite covers the bytes; what only this suite can see is whether this client's menu item reaches the core at all"
          $written = [System.IO.File]::ReadAllText($destination) -replace "`r`n", "`n"
          $delivered = [System.IO.File]::ReadAllText((Resolve-Path $RowSeedFixture)) -replace "`r`n", "`n"
          Assert-Equal $delivered $written `
            'the exported file must be the message that arrived, not a rebuild of the reading view (docs/reading-actions.md). Compared with line endings normalised: the server normalises them on delivery, so the export carries CRLF where the seeder wrote LF'
        } finally {
          Remove-Item $destination -ErrorAction SilentlyContinue
        }
      }
    }
  )
}
