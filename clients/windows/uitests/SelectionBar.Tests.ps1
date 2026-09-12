#!/usr/bin/env pwsh
# The mail actions bar over the two panes (docs/list-selection.md, rule 5): one row of buttons that
# stands whether or not anything is selected. The rule here is that it reads as one row, so every
# button in it has the same height and sits on the same two edges.
#
# THE REGRESSION. Clear selection is the one button carrying a glyph and no label, and the default
# button style centres its content rather than stretching it, so it came up a few pixels shorter
# than the six labelled buttons beside it and the row looked ragged.
#
# WHY THIS SUITE AND NOT `Mailcal.Tests`. A rendered height is exactly what a plain net10.0
# assembly cannot see: the markup compiles, the bindings are correct, and the button is the wrong
# size only once WinUI has measured it.
#
# Dataset is `showcase`: the bar stands with nothing selected, so no mail action is dispatched and
# no selection has to be made to measure it.

# Every button in the bar, in the order the markup lays them out. Named rather than swept off the
# container, so a button that stops being rendered at all fails here instead of quietly leaving the
# comparison to its remaining siblings.
$BarButtons = @(
  'SelectionMarkRead'
  'SelectionFlag'
  'SelectionArchive'
  'SelectionDelete'
  'SelectionDeletePermanently'
  'SelectionSelectAll'
  'SelectionClear'
)

# Physical pixels, as BoundingRectangle reports them, and slack for layout rounding rather than for
# a design difference: the labelled buttons agree with each other exactly, and the defect this suite
# exists for was several device-independent pixels, which only grows on a scaled display.
$EdgeTolerancePx = 1.5

<#
.SYNOPSIS
The rendered rectangle of each button in the actions bar, keyed by automation id.
#>
function Get-BarButtonBounds {
  # Waited on the first BUTTON, not on #SelectionBar. That id is on a `Border`, and UI Automation's
  # control view carries no Border at all, so waiting for it could only ever run its timeout out and
  # answer $null: thirty seconds a case, discarded on the next line, while the case went on to pass
  # on elements it found by other means. A wait nobody reads the result of is not a wait, so this
  # one waits for something that exists and THROWS when it does not arrive.
  if (-not (Wait-UiaElement -Type 'Button' -AutomationId $BarButtons[0] -TimeoutSec 30)) {
    throw "the actions bar never drew its #$($BarButtons[0]) button, so there is no row to measure"
  }
  $bounds = [ordered] @{}
  foreach ($id in $BarButtons) {
    $button = Find-UiaElement -Type 'Button' -AutomationId $id
    $bounds[$id] = Get-RenderedBounds -Element $button -What "the actions bar's #$id button"
  }
  $bounds
}

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'every button in the actions bar is the same height'
      Body = {
        $bounds = Get-BarButtonBounds
        $heights = $bounds.Keys | ForEach-Object { "$_=$([math]::Round($bounds[$_].Height, 1))" }
        $tallest = ($bounds.Values | ForEach-Object { $_.Height } | Measure-Object -Maximum).Maximum
        $shortest = ($bounds.Values | ForEach-Object { $_.Height } | Measure-Object -Minimum).Minimum
        Assert-True (($tallest - $shortest) -le $EdgeTolerancePx) `
          "the actions bar's buttons must all be the same height, the row reads as ragged otherwise. Heights: $($heights -join ' | ')"
      }
    },
    @{
      Name = 'every button in the actions bar sits on the same top and bottom edges'
      Body = {
        # Equal heights alone would also be satisfied by a row of buttons stepping down the bar.
        # These two comparisons are what say it is one row.
        $bounds = Get-BarButtonBounds
        $tops = $bounds.Values | ForEach-Object { $_.Top }
        $bottoms = $bounds.Values | ForEach-Object { $_.Bottom }
        $topSpread = (($tops | Measure-Object -Maximum).Maximum) - (($tops | Measure-Object -Minimum).Minimum)
        $bottomSpread = (($bottoms | Measure-Object -Maximum).Maximum) - (($bottoms | Measure-Object -Minimum).Minimum)
        $edges = $bounds.Keys | ForEach-Object { "$_=$([math]::Round($bounds[$_].Top, 1))..$([math]::Round($bounds[$_].Bottom, 1))" }
        Assert-True ($topSpread -le $EdgeTolerancePx) `
          "the actions bar's buttons must share a top edge. Edges: $($edges -join ' | ')"
        Assert-True ($bottomSpread -le $EdgeTolerancePx) `
          "the actions bar's buttons must share a bottom edge. Edges: $($edges -join ' | ')"
      }
    }
  )
}
