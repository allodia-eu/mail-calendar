#!/usr/bin/env pwsh
# Adding a second account: the setup form in a dialog over the running app, and its way back
# (docs/account-autodetect.md, rule 12).
#
# Why it is here and not in `Mailcal.Tests`: that assembly links no WinUI, and the form moves
# between two hosts at run time, out of the window and into a dialog and back. A form left in the
# wrong host, or a shell hidden behind a dialog that no longer needs it to be, renders without an
# error anywhere.
#
# The dataset is `showcase`: two accounts, so the add is a later one. Nothing here connects
# anything: the form is opened, stepped through without a network, and cancelled.

$AddAccount = 'Add account…'

function Open-AddAccount {
  $row = Find-UiaElement -Name $AddAccount -Type 'ListItem'
  if (-not $row) { throw "the sidebar has no '$AddAccount' row to open the form with" }
  Invoke-UiaElement $row -SettleMs 2000
  $box = Wait-UiaElement -AutomationId 'DetectEmail' -Type 'Edit' -TimeoutSec 10
  if (-not $box) { throw "pressing '$AddAccount' opened no setup form (#DetectEmail)" }
  $box
}

function Close-AddAccount {
  $cancel = Find-UiaElement -AutomationId 'DetectCancelButton' -Type 'Button'
  if (-not $cancel) { $cancel = Find-UiaElement -AutomationId 'CancelButton' -Type 'Button' }
  if ($cancel) { Invoke-UiaElement $cancel -SettleMs 1500 }
}

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'the form opens over the shell, which stays where it was'
      Body = {
        Open-AddAccount | Out-Null
        try {
          # The shell used to be collapsed for the length of an add, which is what took the whole
          # window for a form a few hundred pixels wide. Behind a dialog it is still drawn.
          Assert-True ($null -ne (Find-UiaElement -AutomationId 'SettingsItem')) (
            'adding an account is a dialog over the running app, so the sidebar stays on screen ' +
            'behind it rather than the form taking over the window')
          $dialog = Get-UiaTree -Root (Get-MailcalWindow) |
            Where-Object { $_.Current.AutomationId -eq 'AddAccountDialog' } | Select-Object -First 1
          Assert-True ($null -ne $dialog) 'the form is hosted in the add-account dialog'
        }
        finally { Close-AddAccount }
      }
    }
    @{
      Name = 'the address step can be cancelled, and cancelling closes the dialog'
      Body = {
        Open-AddAccount | Out-Null
        $cancel = Find-UiaElement -AutomationId 'DetectCancelButton' -Type 'Button'
        Assert-True ($null -ne $cancel) (
          'a later add opens on the address step, and that step needs its own way out')
        Invoke-UiaElement $cancel -SettleMs 1500
        Assert-True (Wait-UiaGone -AutomationId 'DetectEmail' -TimeoutSec 5) (
          'Cancel on the address step must close the dialog')
      }
    }
    @{
      Name = 'the second step goes back to the address, inside the dialog'
      Body = {
        $box = Open-AddAccount
        try {
          $address = 'someone@example.com'
          Set-UiaText -Element $box -Text $address
          Invoke-UiaElement (Find-UiaElement -AutomationId 'ManualButton' -Type 'Button')
          Assert-True ($null -ne (Wait-UiaElement -AutomationId 'ImapHost' -Type 'Edit' -TimeoutSec 5)) (
            '"Set up manually" must open the manual form')
          $back = Find-UiaElement -AutomationId 'BackButton' -Type 'Button'
          Assert-True ($null -ne $back -and $back.Current.IsEnabled) (
            'the manual form has a way back to the address step')
          Invoke-UiaElement $back
          $box = Wait-UiaElement -AutomationId 'DetectEmail' -Type 'Edit' -TimeoutSec 5
          Assert-True ($null -ne $box) 'Back must return to the address step, not close the dialog'
          Assert-Equal $address (Get-UiaText $box) 'going back keeps the address'
        }
        finally { Close-AddAccount }
      }
    }
  )
}
