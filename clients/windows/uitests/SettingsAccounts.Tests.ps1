#!/usr/bin/env pwsh
# Settings → Accounts drawn from the core's one snapshot (docs/accounts.md rules 11 to 13): every
# account listed, mail or not; a link suggested where the calendar server schedules as the mailbox's
# address, and linked only once the person presses it; a use switched off only after it is
# confirmed; and a removal that names the accounts losing their link.
#
# Why it is here and not in `Mailcal.Tests`: the rules themselves are pinned there
# (AccountSettingsRulesTests), but whether the page draws them, and whether a press reaches the core
# and comes back as the next snapshot, is WinUI and the core together. A picker wired to nothing
# renders exactly like one that links.
#
# The dataset is the harness as `stalwart-linked`: alice's mailbox used for mail alone, beside
# alice's and bob's calendar and contacts as accounts of their own. Stalwart lists alice's address
# in her calendar principal's `calendar-user-address-set`, so her calendar is the one suggested for
# her mailbox and bob's is offered without being suggested. That mode connects only its canned
# accounts, so whatever a case links or switches is gone at the next launch, and no other suite
# shares it.

$SettingsAccountsMailbox = 'alice@test.local'

<#
.SYNOPSIS
The account rows on the Settings → Accounts list, drawn afresh.
.DESCRIPTION
Through another category first: selecting Accounts while an account's page is open leaves the page
on screen, and a sweep of it finds no rows at all.
#>
function Get-SettingsAccountRows {
  Open-SettingsCategory -Name 'General' | Out-Null
  @(Find-UiaElements -Type 'Button' -Root (Open-SettingsCategory -Name 'Accounts') |
      Where-Object { $_.Current.Name -match '@' })
}

$Suite = @{
  Dataset = 'harness'
  Env     = @{ MAILCAL_DEV_ACCOUNT = 'stalwart-linked' }
  Cases   = @(
    @{
      Name = 'every account is listed, the two without a mailbox included'
      Body = {
        $rows = Get-SettingsAccountRows
        $names = @($rows | ForEach-Object { $_.Current.Name })
        Assert-Equal 3 $rows.Count (
          'docs/accounts.md rule 11: Settings lists every account from accounts_snapshot. The ' +
          "mail-only list has one row here, the snapshot three. Listed: $($names -join ' | ')")
        $calendars = @($rows | Where-Object { $_.Current.HelpText -match 'CalDAV' })
        Assert-Equal 2 $calendars.Count (
          'the two accounts used for calendar and contacts alone are named by their kind. Rows: ' +
          (($rows | ForEach-Object { $_.Current.HelpText }) -join ' | '))
      }
    }
    @{
      Name = "the mailbox's own calendar is suggested, and linked only when the person presses it"
      Body = {
        $dialog = Open-SettingsAccount -Address $SettingsAccountsMailbox -Index 0
        $picker = Find-UiaElement -AutomationId 'AccountLink_Calendar' -Root $dialog
        Assert-True ($null -ne $picker) 'a mailbox without a calendar offers to link one'
        $selected = @($picker.GetCurrentPattern([System.Windows.Automation.SelectionPattern]::Pattern).Current.GetSelection())
        Assert-Equal 'None' ($selected | Select-Object -First 1).Current.Name (
          'docs/accounts.md rule 12: a candidate is suggested, never linked, so the picker still ' +
          'says nothing is linked')
        # The calendar server is asked once the page first reads the snapshot, so the suggestion
        # arrives a moment after the page does.
        $link = Wait-UiaElement -AutomationId 'AccountLinkSuggested_Calendar' -TimeoutSec 15
        Assert-True ($null -ne $link) (
          "the calendar server schedules as the mailbox's address, so its calendar is suggested " +
          'with a button that links it')
        Invoke-UiaElement $link -SettleMs 2500
        $dialog = Get-SettingsDialog
        $picker = Find-UiaElement -AutomationId 'AccountLink_Calendar' -Root $dialog
        $selected = @($picker.GetCurrentPattern([System.Windows.Automation.SelectionPattern]::Pattern).Current.GetSelection())
        Assert-Equal $SettingsAccountsMailbox ($selected | Select-Object -First 1).Current.Name 'pressing Link picks the suggested calendar'
        Assert-True ($null -eq (Find-UiaElement -AutomationId 'AccountLinkSuggested_Calendar' -Root $dialog)) (
          'a link already made is not suggested again: the page was drawn from the next snapshot')
        # The core's answer, from the other end: naming a calendar from the mailbox links the
        # mailbox to the calendar too, and only the next snapshot can say so.
        $rows = @(Get-SettingsAccountRows | ForEach-Object { $_.Current.HelpText })
        Assert-True (@($rows | Where-Object { $_ -match "Mail: $SettingsAccountsMailbox" }).Count -eq 1) (
          "one calendar account names the mailbox it sends through. Rows: $($rows -join ' || ')")
      }
    }
    @{
      Name = 'switching a use off is asked first, and cancelling puts the switch back'
      Body = {
        $dialog = Open-SettingsAccount -Address $SettingsAccountsMailbox -Index 1
        $contacts = Find-UiaElement -AutomationId 'AccountUse_Contacts' -Root $dialog
        Assert-True (Get-UiaToggle $contacts) 'the calendar account starts used for its contacts'
        Set-UiaToggle $contacts
        $off = Find-UiaElement -Name 'Switch off' -Type 'Button' -Root (Get-SettingsDialog)
        Assert-True ($null -ne $off) (
          'docs/accounts.md rule 13: switching a use off deletes what the device holds of it, so ' +
          'it is confirmed first')
        Invoke-UiaElement (Find-UiaElement -Name 'Cancel' -Type 'Button' -Root (Get-SettingsDialog))
        $contacts = Find-UiaElement -AutomationId 'AccountUse_Contacts' -Root (Get-SettingsDialog)
        Assert-True (Get-UiaToggle $contacts) 'cancelling leaves the use on'
      }
    }
    @{
      Name = 'the last use is held on, and says why'
      Body = {
        $dialog = Open-SettingsAccount -Address $SettingsAccountsMailbox -Index 0
        $mail = Find-UiaElement -AutomationId 'AccountUse_Mail' -Root $dialog
        Assert-True (Get-UiaToggle $mail) 'the mailbox is used for mail'
        Assert-True (-not $mail.Current.IsEnabled) (
          'docs/accounts.md rule 13: the last of mail, calendar and contacts cannot be switched ' +
          'off, so its switch is not offered')
      }
    }
    @{
      Name = "removing an account names the accounts that lose their link, and Cancel keeps it"
      Body = {
        # Linked by the case above: the mailbox now names alice's calendar account.
        $dialog = Open-SettingsAccount -Address $SettingsAccountsMailbox -Index 1
        Invoke-UiaElement (Find-UiaElement -AutomationId 'AccountRemove' -Root $dialog)
        $message = Find-UiaElement -AutomationId 'AccountRemoveMessage' -Root (Get-SettingsDialog)
        Assert-True ($message.Current.Name -match "unlinked from $SettingsAccountsMailbox") (
          'docs/accounts.md rule 12: the confirmation names the accounts that lose their link. ' +
          "It reads: $($message.Current.Name)")
        Invoke-UiaElement (Find-UiaElement -Name 'Cancel' -Type 'Button' -Root (Get-SettingsDialog))
        Assert-Equal 3 (Get-SettingsAccountRows).Count 'cancelling removes nothing'
      }
    }
  )
}
