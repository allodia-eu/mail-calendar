#!/usr/bin/env pwsh
# The name an account sends its mail under, on both surfaces that show it: the field on the
# account's page in Settings → Accounts, and the composer's From (docs/sending.md, "The sender's name").
#
# Why it is here and not in `Mailcal.Tests`: that assembly is plain net10.0 and links no WinUI, and
# every rule below is a WinUI one. The field is built in code-behind from `AccountSyncChoice`, and
# it commits on LOSING FOCUS rather than on a keystroke, so a page that draws the box and wires
# nothing to it, or wires it to the wrong account, renders exactly like a working one; the From
# label is a DisplayMemberPath, and one pointed at the wrong property renders perfectly and
# misnames every account. `cargo test` proves the core stores and composes what it is handed;
# nothing else proves a client ever hands it over or draws what comes back.
#
# The dataset is the harness as two accounts (`stalwart-multi`), each given the name the server
# holds for it, and Stalwart advertises no sender identities, so the name is ours alone and every
# page offers the field. Not the showcase: Settings → Accounts is drawn from `accounts_snapshot`,
# which lists the accounts the core registered, and the in-memory showcase registers none.
#
# KNOWN GAPS, and why each is one:
#
#   * THE READ-ONLY CARD. `sender_name_editable` is false only where a provider holds the name and
#     the account holder cannot change it, which today means a Microsoft mailbox reading its
#     organisation's directory. No dataset here can produce one: the harness is Stalwart. So the branch that states the name instead of
#     offering a field is asserted only in its ABSENCE (case 1), and its presence needs a real
#     Microsoft account, by hand.
#   * THE STEP AFTER A CONNECT. The dialog that asks for the name is raised by
#     `MailboxModel.SenderNamePrompt`, which only an add-account route sets, so reaching it means
#     really connecting an account, which a shared suite may not do. Staging the prompt directly
#     would fake the surface rather than the input: the assertion would go on passing after the
#     wiring between the add and the ask was cut, which is the class of bug this suite exists for.
#     Driven by hand on the harness instead.
#
# The catalog's own words, hardcoded rather than read back from the app: a test that asks the app
# what it says and then checks it says that cannot fail.
$SenderNameHeading = 'Your name'
$SenderNameManaged = 'Your organisation sets this name, so it cannot be changed here.'
$DepthLabel = 'Download'

<#
.SYNOPSIS
The "your name" text box on the open account page.
.DESCRIPTION
Typed to Edit because the heading above the box carries the same string, so a bare -Name sweep
returns the inert TextBlock as well (uia.ps1 trap 2) and would pass for a page that drew a label
and no field at all.
#>
function Get-SenderNameBoxes {
  @(Find-UiaElements -Name $SenderNameHeading -Type 'Edit' -Root (Get-SettingsDialog))
}

<#
.SYNOPSIS
Scroll the open account page until its two ordered rows are rendered, and return their Tops.
.DESCRIPTION
The ladder rather than a computed position, because what fits is a function of the window and the
display scale. Measuring a row that is off screen would compare a real Top against an infinity:
uia.ps1's rendered-bounds rule exists because that comparison PASSES.
#>
function Get-SenderNamePageOrder {
  $scroller = Get-UiaTree -Root (Get-SettingsDialog) | Where-Object {
    try { $_.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern).Current.VerticallyScrollable }
    catch { $false }
  } | Select-Object -First 1
  if (-not $scroller) { throw 'the account page has no vertical scroller' }
  $pattern = $scroller.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern)
  foreach ($percent in 0, 25, 50, 75, 100) {
    $pattern.SetScrollPercent(-1, $percent)
    Wait-UiaQuiet -CapMs 900 -FloorMs 400
    $dialog = Get-SettingsDialog
    $rows = [ordered] @{
      "the '$SenderNameHeading' field" = @(Find-UiaElements -Name $SenderNameHeading -Type 'Edit' -Root $dialog)[0]
      "the '$DepthLabel' picker" = @(Find-UiaElements -Name $DepthLabel -Type 'ComboBox' -Root $dialog)[0]
    }
    $missing = @($rows.Keys | Where-Object { -not $rows[$_] })
    if ($missing) { throw "the account page has no $($missing -join ', and no ') at all" }
    $tops = [ordered] @{}
    foreach ($what in $rows.Keys) {
      $rect = $rows[$what].Current.BoundingRectangle
      if ([double]::IsInfinity($rect.X) -or $rect.Height -le 0) { $tops = $null; break }
      $tops[$what] = $rect.Top
    }
    if ($tops) { return $tops }
  }
  throw 'no scroll position renders the whole of the account page, so its row order cannot be measured'
}

<#
.SYNOPSIS
Type $Text into $Box and then move focus off it, which is what commits the edit.
.DESCRIPTION
Both halves matter. ValuePattern.SetValue does NOT focus the box, so a box that never had focus
never raises LostFocus and the edit is never sent: a test that only set the value would report a
broken wiring on a page that works. And the blur has to land somewhere real, so focus goes to a
picker on the same page.
#>
function Set-SenderName {
  param([Parameter(Mandatory)] [object] $Box, [Parameter(Mandatory)] [AllowEmptyString()] [string] $Text)
  $Box.SetFocus()
  Start-Sleep -Milliseconds 200
  Set-UiaText -Element $Box -Text $Text
  $elsewhere = Find-UiaElement -Name $DepthLabel -Type 'ComboBox' -Root (Get-SettingsDialog)
  if (-not $elsewhere) { throw "the account page has no '$DepthLabel' picker to move focus to" }
  $elsewhere.SetFocus()
  Wait-UiaQuiet -CapMs 1500 -FloorMs 500
}

<#
.SYNOPSIS
Open the composer and return its From picker, closing Settings on the way if it is up.
#>
function Get-ComposerFrom {
  $settings = Find-SettingsDialog
  if ($settings) {
    Invoke-UiaElement (Find-UiaElement -AutomationId 'CloseButton' -Type 'Button' -Root $settings) -SettleMs 1200
  }
  if (-not (Find-UiaElement -AutomationId 'FromBox' -Type 'ComboBox')) {
    $compose = Find-UiaElement -AutomationId 'ComposeButton' -Type 'Button'
    if (-not $compose) { throw 'no Compose button to open the composer with' }
    Invoke-UiaElement $compose -SettleMs 2500
  }
  $from = Wait-UiaElement -AutomationId 'FromBox' -TimeoutSec 15
  if (-not $from) { throw 'the composer opened without a From picker (#FromBox)' }
  $from
}

$Suite = @{
  Dataset = 'harness'
  Env     = @{ MAILCAL_DEV_ACCOUNT = 'stalwart-multi' }
  Prepare = {
    Open-SettingsCategory -Name 'Accounts' | Out-Null
    Wait-UiaQuiet -CapMs 1500 -FloorMs 500
  }
  Cases   = @(
    @{
      Name = 'every account page offers the field, because no provider here holds the name'
      Body = {
        foreach ($i in 0, 1) {
          Open-SettingsAccount -Index $i | Out-Null
          $boxes = Get-SenderNameBoxes
          Assert-Equal 1 $boxes.Count (
            'docs/sending.md rule 2: the field is offered wherever the name is the account ' +
            "holder's, and this dataset's provider keeps no name of its own, so account $i's page has a box")
          Assert-True $boxes[0].Current.IsEnabled (
            'a box that is drawn and disabled reads as a bug; where the name cannot be changed ' +
            'the page states the name instead and draws no box')
          Assert-True ((Get-UiaText $boxes[0]).Trim().Length -gt 0) (
            'the harness gives each account the name the server holds for it ' +
            '(MailboxModel.DevAccount.cs); an empty field means that never reached the core')
          $managed = @(Find-UiaElements -Name $SenderNameManaged -Type 'Text' -Root (Get-SettingsDialog))
          Assert-Equal 0 $managed.Count (
            'the organisation-sets-this note belongs only where sender_name_editable is false. A ' +
            'client that showed it here would be deciding editability from something other than ' +
            'the capability (docs/sending.md rule 2)')
        }
      }
    }
    @{
      Name = 'the name sits above what the account stores'
      Body = {
        Open-SettingsAccount -Index 0 | Out-Null
        # The name is the one control on the page whose effect a stranger sees, so it comes above
        # the questions about how much of the account this device keeps (docs/settings.md).
        $tops = Get-SenderNamePageOrder
        $ordered = @($tops.Keys)
        for ($row = 1; $row -lt $ordered.Count; $row++) {
          Assert-True ($tops[$ordered[$row - 1]] -lt $tops[$ordered[$row]]) (
            "$($ordered[$row - 1]) comes above $($ordered[$row]) " +
            "($($tops[$ordered[$row - 1]]) vs $($tops[$ordered[$row]]))")
        }
      }
    }
    @{
      Name = 'a typed name reaches the core, and the page reads it back from there'
      Body = {
        # Remembered rather than assumed, and put back at the end: suites share one launch, and
        # the case after this one reads what the seed set.
        Open-SettingsAccount -Index 1 | Out-Null
        $other = Get-UiaText (Get-SenderNameBoxes)[0]
        Open-SettingsAccount -Index 0 | Out-Null
        $was = Get-UiaText (Get-SenderNameBoxes)[0]
        Set-SenderName -Box (Get-SenderNameBoxes)[0] -Text 'Ada Lovelace'
        Open-SettingsAccount -Index 0 | Out-Null
        Assert-Equal 'Ada Lovelace' (Get-UiaText (Get-SenderNameBoxes)[0]) (
          'the page is rebuilt from SyncSettings() every time it is shown, so this is the value ' +
          'the CORE holds. The old name here means the edit never left the client: the box ' +
          'commits on LostFocus, and nothing else in the client watches it')
        Open-SettingsAccount -Index 1 | Out-Null
        Assert-Equal $other (Get-UiaText (Get-SenderNameBoxes)[0]) (
          'the name is per account (docs/sending.md), so it lands on the page it was typed on ' +
          'and on no other. The second account moving too means the setter was handed the wrong id')
        Open-SettingsAccount -Index 0 | Out-Null
        Set-SenderName -Box (Get-SenderNameBoxes)[0] -Text $was
      }
    }
    @{
      Name = "the composer's From reads as the recipient will read it, name and address"
      Body = {
        # docs/sending.md rule 7. The From is where the sender picks who a message comes from, so
        # it shows the whole of what arrives rather than half of it. Both seeded accounts carry a
        # name here, so a picker still showing bare addresses means the label never reached it.
        $from = Get-ComposerFrom
        $selection = @(
          $from.GetCurrentPattern([System.Windows.Automation.SelectionPattern]::Pattern).Current.GetSelection() |
            ForEach-Object { $_.Current.Name })
        Assert-Equal 1 $selection.Count 'the From picker opens on exactly one account'
        Assert-True ($selection[0] -match '^.+ <[^<>@]+@[^<>]+>$') (
          "the From label is `Name <address>`, which is the shape the recipient's own client " +
          "shows. It reads: $($selection[0])")
        # Every item, not only the selected one: a picker labelled from the right property in one
        # place and the wrong one in the other renders perfectly and misnames every other account.
        $from.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Expand()
        Wait-UiaQuiet -CapMs 1200 -FloorMs 400
        $items = @(Find-UiaElements -Type 'ListItem' -Root $from | ForEach-Object { $_.Current.Name })
        Assert-True ($items.Count -ge 2) (
          "the harness connects two accounts, so the picker offers both. It offers: $($items -join ' | ')")
        foreach ($item in $items) {
          Assert-True ($item -match '^.+ <[^<>@]+@[^<>]+>$') (
            "every account in the picker carries its name, not only the selected one. This one " +
            "reads: $item")
        }
        $from.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Collapse()
        Wait-UiaQuiet -CapMs 900 -FloorMs 300
        # Leave the composer closed, so the runner can hand this launch to the next suite.
        $discard = Find-UiaElements -Type 'Button' |
          Where-Object { $_.Current.AutomationId -in @('DiscardButton', 'CancelButton') } |
          Select-Object -First 1
        if ($discard) { Invoke-UiaElement $discard -SettleMs 1500 }
      }
    }
    @{
      Name = 'clearing the field is an answer, not a no-op: the account goes back to a bare address'
      Body = {
        Open-SettingsAccount -Index 0 | Out-Null
        $boxes = Get-SenderNameBoxes
        Assert-True ($boxes.Count -ge 1) (
          'the account page offers no field, so there is nothing here to clear')
        $was = Get-UiaText $boxes[0]
        Assert-True ($was.Length -gt 0) (
          'this case needs a name to delete; the seeded page should carry one')
        Set-SenderName -Box $boxes[0] -Text ''
        Open-SettingsAccount -Index 0 | Out-Null
        Assert-Equal '' (Get-UiaText (Get-SenderNameBoxes)[0]) (
          'docs/sending.md rule 3: an empty name is the first-run state and a real answer. A ' +
          'client that treated empty as "nothing to do" would leave the old name on the wire ' +
          'after the user deleted it')
        # Put the seed back, since suites share one launch and the next one may read it.
        Set-SenderName -Box (Get-SenderNameBoxes)[0] -Text $was
      }
    }
  )
}
