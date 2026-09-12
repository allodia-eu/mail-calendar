#!/usr/bin/env pwsh
# The mail actions bar across the window (docs/list-selection.md, rule 5): one row of buttons that
# stands whether or not anything is selected. The rule here is that it reads as one row, so every
# button in it has the same height and sits on the same two edges.
#
# It spans the folder pane as well as the two mail panes, and the two buttons on it that name no
# selection sit at its two ends: New Mail at the head, Sync at the tail. New Mail is there rather
# than in the pane it used to head, because collapsing the pane would take it off screen, and
# writing a message is the one thing that must always be one click away.
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
  # Past a divider from the rest, and not a selection action: New Mail acts on nothing that is
  # picked. Measured with them because the row still has to read as one row.
  'ComposeButton'
  'SelectionMarkRead'
  'SelectionFlag'
  'SelectionArchive'
  'SelectionDelete'
  'SelectionDeletePermanently'
  'SelectionSelectAll'
  'SelectionClear'
  # Past the divider, and not a selection action: Sync acts on the mailbox. It is measured with
  # the rest because the row still has to read as one row.
  'SelectionSync'
)

# Physical pixels, as BoundingRectangle reports them, and slack for layout rounding rather than for
# a design difference: the labelled buttons agree with each other exactly, and the defect this suite
# exists for was several device-independent pixels, which only grows on a scaled display.
$EdgeTolerancePx = 1.5

<#
.SYNOPSIS
The rendered rectangle of each button in the actions bar, keyed by automation id.
#>
<#
.SYNOPSIS
Whether any message row is selected right now.
.DESCRIPTION
The oracle for "is this button following the selection", read off the LIST rather than off another
button bound to the same flag: two controls sharing one broken binding agree perfectly. Which
rows a dataset leaves selected is not this suite's to decide, the reading pane selects the row it
is showing, so the rule is stated against whatever is picked rather than against an empty list.
#>
function Test-RowSelected {
  $pattern = [System.Windows.Automation.SelectionItemPattern]::Pattern
  foreach ($row in Get-MailRows) {
    if ($row.GetCurrentPattern($pattern).Current.IsSelected) { return $true }
  }
  $false
}

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
    },
    @{
      Name = 'New Mail stands at the head of the bar and never follows the selection'
      Body = {
        # docs/list-selection.md, rule 5. Nothing on this row names the selection except the
        # selection's own buttons, so New Mail is live whatever is picked, and it sits ahead of
        # them rather than among them.
        $compose = Find-UiaElement -Type 'Button' -AutomationId 'ComposeButton'
        if (-not $compose) { throw 'the actions bar never drew its #ComposeButton' }
        Assert-True $compose.Current.IsEnabled `
          'New Mail must stay live whatever is selected: it writes a message, it does not act on a selection'
        Assert-Equal (Test-RowSelected) `
          (Find-UiaElement -Type 'Button' -AutomationId 'SelectionArchive').Current.IsEnabled `
          'and Archive beside it must follow the selection, or this proves nothing about the two being different'

        $bounds = Get-BarButtonBounds
        Assert-True ($bounds['ComposeButton'].Right -le $bounds['SelectionMarkRead'].Left) `
          "New Mail (right edge $($bounds['ComposeButton'].Right)) must come before the selection's own buttons (Mark as read starts at $($bounds['SelectionMarkRead'].Left))"
      }
    },
    @{
      Name = 'New Mail survives a collapsed folder pane'
      Body = {
        # Why the button is on this row at all. It used to head the folder pane, and the pane
        # collapses to an icon strip: a New Mail the user cannot see is one they cannot reach,
        # and nothing else in the app writes a message.
        $toggle = Find-UiaElement -Type 'Button' -AutomationId 'PART_PaneToggleButton'
        if (-not $toggle) { throw 'no pane toggle in the caption, so this proves nothing' }

        Invoke-UiaElement $toggle -SettleMs 800
        try {
          $compose = Find-UiaElement -Type 'Button' -AutomationId 'ComposeButton'
          Assert-True (($null -ne $compose) -and (-not $compose.Current.IsOffscreen)) `
            'New Mail must stay on screen with the folder pane collapsed'
        }
        finally {
          # Back to where the suite found it: the app is shared with every suite after this one.
          Invoke-UiaElement $toggle -SettleMs 800
        }
      }
    },
    @{
      Name = 'Sync stands at the end of the bar and never follows the selection'
      Body = {
        # docs/list-selection.md, rule 5. Sync does not name what is picked: it acts on the
        # mailbox, so it is live whatever the selection is, and it sits past the selection's own
        # buttons rather than among them.
        $sync = Find-UiaElement -Type 'Button' -AutomationId 'SelectionSync'
        if (-not $sync) { throw 'the actions bar never drew its #SelectionSync button' }
        Assert-True $sync.Current.IsEnabled `
          'Sync must stay live whatever is selected: it syncs the mailbox, not the selection'
        Assert-Equal (Test-RowSelected) `
          (Find-UiaElement -Type 'Button' -AutomationId 'SelectionArchive').Current.IsEnabled `
          'and Archive beside it must follow the selection, or this proves nothing about the two being different'

        $bounds = Get-BarButtonBounds
        Assert-True ($bounds['SelectionSync'].Left -ge $bounds['SelectionClear'].Right) `
          "Sync (left edge $($bounds['SelectionSync'].Left)) must come after the selection's own buttons (Clear ends at $($bounds['SelectionClear'].Right))"
      }
    }
  )
}
