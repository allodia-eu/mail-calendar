#!/usr/bin/env pwsh
# The name an account sends its mail under, on both surfaces that show it: the field on the
# Settings → Accounts card, and the composer's From (docs/sending.md, "The sender's name").
#
# Why it is here and not in `Mailcal.Tests`: that assembly is plain net10.0 and links no WinUI, and
# every rule below is a WinUI one. The field is built in code-behind from `AccountSyncChoice`, and
# it commits on LOSING FOCUS rather than on a keystroke, so a card that draws the box and wires
# nothing to it, or wires it to the wrong account, renders exactly like a working one; the From
# label is a DisplayMemberPath, and one pointed at the wrong property renders perfectly and
# misnames every account. `cargo test` proves the core stores and composes what it is handed;
# nothing else proves a client ever hands it over or draws what comes back.
#
# The dataset is `showcase`: two accounts, each seeded with the name its own mail addresses, and
# its provider advertises no sender identities, which is the IMAP shape, the name is ours alone
# and every card is editable.
#
# KNOWN GAPS, and why each is one:
#
#   * THE READ-ONLY CARD. `sender_name_editable` is false only where a provider holds the name and
#     the account holder cannot change it, which today means a Microsoft mailbox reading its
#     organisation's directory. No dataset here can produce one: the showcase provider has no
#     identities at all and the harness is Stalwart. So the branch that states the name instead of
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
Every "your name" text box on the Accounts panel, in card order.
.DESCRIPTION
Typed to Edit because the heading above each box carries the same string, so a bare -Name sweep
returns the inert TextBlock as well (uia.ps1 trap 2) and would pass for a card that drew a label
and no field at all.

Tree order, NOT screen order: the panel is one StackPanel and scrolls, so the cards past the fold
have no usable rectangle to sort on, and sorting by an infinity is how the first draft of this file
silently examined one card out of two. Where the rule is genuinely about the eye, case 2 brings the
card into view and measures it there.
#>
function Get-SenderNameBoxes {
  @(Find-UiaElements -Name $SenderNameHeading -Type 'Edit' -Root (Get-SettingsDialog))
}

<#
.SYNOPSIS
The Accounts panel's scroller.
#>
function Get-AccountsScroller {
  $scroller = Get-UiaTree -Root (Get-SettingsDialog) | Where-Object {
    try { $_.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern).Current.VerticallyScrollable }
    catch { $false }
  } | Select-Object -First 1
  if (-not $scroller) { throw 'the Accounts panel has no vertical scroller' }
  $scroller
}

<#
.SYNOPSIS
Scroll the Accounts panel until card $Index has all three of its ordered rows rendered, and return
their Tops; throws naming the card when no scroll position shows the whole of it.
.DESCRIPTION
The ladder rather than a computed position, because how many cards fit is a function of the window,
the display scale and how many rows each card has. Every failure below is about which card a row
belongs to, so measuring a card that is only half on screen would compare a real Top against an
infinity: uia.ps1's rendered-bounds rule exists because that comparison PASSES.
#>
function Get-CardOrder {
  param([Parameter(Mandatory)] [int] $Index)
  $scroller = Get-AccountsScroller
  $pattern = $scroller.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern)
  foreach ($percent in 0, 25, 50, 75, 100) {
    $pattern.SetScrollPercent(-1, $percent)
    Wait-UiaQuiet -CapMs 900 -FloorMs 400
    $dialog = Get-SettingsDialog
    $rows = [ordered] @{
      'the address that names the card' =
      @(Find-UiaElements -Type 'Text' -Root $dialog | Where-Object { $_.Current.Name -match '@' })[$Index]
      "the '$SenderNameHeading' field" = @(Find-UiaElements -Name $SenderNameHeading -Type 'Edit' -Root $dialog)[$Index]
      "the '$DepthLabel' picker" = @(Find-UiaElements -Name $DepthLabel -Type 'ComboBox' -Root $dialog)[$Index]
    }
    $missing = @($rows.Keys | Where-Object { -not $rows[$_] })
    if ($missing) {
      # Absent and merely scrolled away are different answers, and scrolling for the second is
      # what would otherwise report the first as "could not be measured".
      throw "account card $Index has no $($missing -join ', and no ') at all"
    }
    $tops = [ordered] @{}
    foreach ($what in $rows.Keys) {
      $rect = $rows[$what].Current.BoundingRectangle
      if ([double]::IsInfinity($rect.X) -or $rect.Height -le 0) { $tops = $null; break }
      $tops[$what] = $rect.Top
    }
    if ($tops) { return $tops }
  }
  throw "no scroll position renders the whole of account card $Index, so its row order cannot be measured"
}

<#
.SYNOPSIS
Type $Text into $Box and then move focus off it, which is what commits the edit.
.DESCRIPTION
Both halves matter. ValuePattern.SetValue does NOT focus the box, so a box that never had focus
never raises LostFocus and the edit is never sent: a test that only set the value would report a
broken wiring on a card that works. And the blur has to land somewhere real, so focus goes to a
picker on the same card.
#>
function Set-SenderName {
  param([Parameter(Mandatory)] [object] $Box, [Parameter(Mandatory)] [AllowEmptyString()] [string] $Text)
  $Box.SetFocus()
  Start-Sleep -Milliseconds 200
  Set-UiaText -Element $Box -Text $Text
  $elsewhere = Find-UiaElement -Name $DepthLabel -Type 'ComboBox' -Root (Get-SettingsDialog)
  if (-not $elsewhere) { throw "the account card has no '$DepthLabel' picker to move focus to" }
  $elsewhere.SetFocus()
  Wait-UiaQuiet -CapMs 1500 -FloorMs 500
}

<#
.SYNOPSIS
Leave the Accounts panel and come back, so its controls are rebuilt from the CORE.
.DESCRIPTION
This is the whole point of the read-back cases. `ShowCategory` calls `GetSyncSettings()` afresh
each time, so what a box shows after this round trip is the core's answer and not the string that
was typed into it, which is the difference between "the client kept a copy" and "the value landed".
#>
function Reset-AccountsPanel {
  Open-SettingsCategory -Name 'General' | Out-Null
  Open-SettingsCategory -Name 'Accounts' | Out-Null
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
  Dataset = 'showcase'
  Prepare = {
    Open-SettingsCategory -Name 'Accounts' | Out-Null
    Wait-UiaQuiet -CapMs 1500 -FloorMs 500
  }
  Cases   = @(
    @{
      Name = 'every account card offers the field, because no provider here holds the name'
      Body = {
        $boxes = Get-SenderNameBoxes
        # One card per account, and the depth picker is a row every card has, so counting those
        # states the rule ("every card") rather than pinning today's number of seeded accounts.
        $cards = @(Find-UiaElements -Name $DepthLabel -Type 'ComboBox' -Root (Get-SettingsDialog)).Count
        Assert-True ($cards -gt 0) (
          'the Accounts panel drew no account card at all, so nothing below proves anything')
        Assert-Equal $cards $boxes.Count (
          'docs/sending.md rule 2: the field is offered wherever the name is the account ' +
          "holder's, and this dataset's provider keeps no name of its own, so every one of the " +
          "$cards cards has a box")
        foreach ($box in $boxes) {
          Assert-True $box.Current.IsEnabled (
            'a box that is drawn and disabled reads as a bug; where the name cannot be changed ' +
            'the card states the name instead and draws no box')
        }
        $managed = @(Find-UiaElements -Name $SenderNameManaged -Type 'Text' -Root (Get-SettingsDialog))
        Assert-Equal 0 $managed.Count (
          'the organisation-sets-this note belongs only where sender_name_editable is false. A ' +
          'client that showed it here would be deciding editability from something other than ' +
          'the capability (docs/sending.md rule 2)')
      }
    }
    @{
      Name = "the name sits under its own account's address, above what that account stores"
      Body = {
        $cards = @(Find-UiaElements -Name $DepthLabel -Type 'ComboBox' -Root (Get-SettingsDialog)).Count
        for ($i = 0; $i -lt $cards; $i++) {
          # docs/settings.md puts the name on the Accounts card, and the card puts it above the
          # questions about how much of the account this device keeps: it is the one control there
          # whose effect a stranger sees. The panel is a flat stack with no box around either card,
          # so the address above is the only thing saying which account a row is about, and a field
          # that drifted out of its card would read as belonging to a different mailbox while
          # rendering perfectly.
          $tops = Get-CardOrder -Index $i
          $ordered = @($tops.Keys)
          for ($row = 1; $row -lt $ordered.Count; $row++) {
            Assert-True ($tops[$ordered[$row - 1]] -lt $tops[$ordered[$row]]) (
              "on account card $i, $($ordered[$row - 1]) comes above $($ordered[$row]) " +
              "($($tops[$ordered[$row - 1]]) vs $($tops[$ordered[$row]]))")
          }
        }
      }
    }
    @{
      Name = 'every account starts under the name its own seeded mail addresses'
      Body = {
        # The showcase seeds this at boot by calling the use case, so nothing in the seed data
        # says whether it happened; without it both cards open on their first-run empty state and
        # the From picker below has nothing but addresses to show.
        foreach ($box in Get-SenderNameBoxes) {
          Assert-True ((Get-UiaText $box).Trim().Length -gt 0) (
            'the showcase dataset gives each account the name its own mail already addresses ' +
            '(boot/inmemory.rs); an empty card here means the seeding never reached the core')
        }
      }
    }
    @{
      Name = 'a typed name reaches the core, and the card reads it back from there'
      Body = {
        Reset-AccountsPanel
        $boxes = Get-SenderNameBoxes
        Assert-True ($boxes.Count -ge 2) (
          'this case needs two cards to state which account the name landed on; the showcase ' +
          "dataset seeds two accounts and this run has $($boxes.Count)")
        # Remembered rather than assumed, and put back at the end: suites share one launch, and
        # the case after this one reads what the seed set.
        $was = @(Get-UiaText $boxes[0]; Get-UiaText $boxes[1])
        Set-SenderName -Box $boxes[0] -Text 'Ada Lovelace'
        Reset-AccountsPanel
        $after = Get-SenderNameBoxes
        Assert-Equal 'Ada Lovelace' (Get-UiaText $after[0]) (
          'the card is rebuilt from SyncSettings() every time it is shown, so this is the value ' +
          'the CORE holds. The old name here means the edit never left the client: the box ' +
          'commits on LostFocus, and nothing else in the client watches it')
        Assert-Equal $was[1] (Get-UiaText $after[1]) (
          'the name is per account (docs/sending.md), so it lands on the card it was typed on ' +
          'and on no other. The second card moving too means the setter was handed the wrong id')
        Set-SenderName -Box (Get-SenderNameBoxes)[0] -Text $was[0]
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
          "the showcase seeds two accounts, so the picker offers both. It offers: $($items -join ' | ')")
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
        Reset-AccountsPanel
        $boxes = Get-SenderNameBoxes
        Assert-True ($boxes.Count -ge 1) (
          'no account card offers the field, so there is nothing here to clear')
        $was = Get-UiaText $boxes[0]
        Assert-True ($was.Length -gt 0) (
          'this case needs a name to delete; the seeded card should carry one')
        Set-SenderName -Box $boxes[0] -Text ''
        Reset-AccountsPanel
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
