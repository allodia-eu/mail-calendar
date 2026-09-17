#!/usr/bin/env pwsh
# What the message list draws under the pointer, which is nothing.
#
# A mouse moving down a mailbox passes over every row on its way to the one it wants. Anything the
# list puts on screen for that is noise: the reader is not asking a question, and a label that
# follows the pointer down the list obscures the very rows being scanned.
#
# THE REGRESSION. The list carries keyboard accelerators for Delete, Back and Escape
# (docs/list-selection.md, rule 9), and `KeyboardAcceleratorPlacementMode` defaults to `Auto`, which
# tells the framework to raise a tooltip naming the accelerator's KEY over whatever the pointer is
# resting on. So every row the mouse crossed got a bare "Delete" floating above it. It is the
# framework's word for the key rather than any string of ours, so no catalog check could see it, it
# matches nothing on the row's own menu (which calls that action "Move to Trash"), and it appears in
# every locale.
#
# WHY THIS SUITE AND NOT `Mailcal.Tests`. A tooltip the framework raises is not in the markup at
# all: nothing is bound, nothing is assigned, and the only way to know it is there is to put a
# pointer on a row in a running window and look. That is also why the pointer is moved by injection
# rather than by warping the cursor; uia.ps1's Move-UiaPointerOnto says what a warp measures
# instead.
#
# Dataset is `showcase`: rows are all this needs, and hovering one dispatches nothing.

# The tooltip's own delay before it appears, with room to spare. Waiting longer costs this suite a
# second; waiting less asserts that nothing had appeared YET, which is not the rule.
$HoverSettleMs = 1800

<#
.SYNOPSIS
A message row well down the list, so the pointer has rows to cross on its way to it.
#>
function Get-HoverTargetRow {
  $rows = @(Get-MailRows)
  if ($rows.Count -lt 4) {
    throw "the list holds $($rows.Count) row(s); this suite needs a few to move the pointer across"
  }
  $rows | Select-Object -Skip 3 -First 1
}

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'hovering a message row says nothing'
      Body = {
        $row = Get-HoverTargetRow
        $at = Move-UiaPointerOnto -Element $row -What 'a message row'
        Start-Sleep -Milliseconds $HoverSettleMs
        $tips = @(Get-UiaToolTips | ForEach-Object { $_.Current.Name })
        Assert-Equal 0 $tips.Count (
          "the pointer resting on a message row at $($at[0]),$($at[1]) raised: $($tips -join ' | '). " +
          'A pointer passing over a message is not asking a question, and a label that follows it ' +
          'down the list covers the rows being scanned. The framework raises one of its own for a ' +
          'keyboard accelerator unless the list says not to')
      }
    },
    @{
      Name = 'and the keys the list is silent about still work'
      Body = {
        # The other half of the same line: hiding the accelerator's tooltip must not take the
        # accelerator with it. Escape, because clearing a selection is the one of the three that
        # costs no message. The row is selected through the automation pattern rather than by a
        # click, which would also open it in the pane.
        $row = Get-HoverTargetRow
        Invoke-UiaElement $row
        $list = Find-UiaElement -AutomationId 'RowsList'
        $selection = $list.GetCurrentPattern([System.Windows.Automation.SelectionPattern]::Pattern)
        Assert-Equal 1 $selection.Current.GetSelection().Length 'a row is selected to clear'

        [System.Windows.Forms.SendKeys]::SendWait('{ESC}')
        Wait-UiaQuiet -CapMs 1200
        Assert-Equal 0 $selection.Current.GetSelection().Length (
          'Escape must still clear the selection. KeyboardAcceleratorPlacementMode decides whether ' +
          'the framework ANNOUNCES an accelerator, never whether it fires, and a change that ' +
          'silenced the list by removing the accelerators would pass the case above')
      }
    }
  )
}
