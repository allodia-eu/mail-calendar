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

<#
.SYNOPSIS
Opens the form, types $Address and continues, and waits for the card detection found.
#>
function Open-AccountCard {
  param([Parameter(Mandatory)] [string] $Address)
  $box = Open-AddAccount
  Set-UiaText -Element $box -Text $Address
  Invoke-UiaElement (Find-UiaElement -AutomationId 'ContinueButton' -Type 'Button')
  if (-not (Wait-UiaElement -AutomationId 'ConnectButton' -Type 'Button' -TimeoutSec 15)) {
    throw "continuing with $Address reached no card"
  }
  Wait-UiaQuiet -CapMs 1200 -FloorMs 300
}

# Whether the element with $Id is drawn on screen. Collapsed WinUI elements leave the UIA tree, and
# a drawn one scrolled out of view is still drawn.
function Test-UiaShown {
  param([Parameter(Mandatory)] [string] $Id)
  $null -ne (Find-UiaElement -AutomationId $Id)
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
    @{
      Name = 'the found card offers mail, the calendar and contacts as detection found them'
      Body = {
        # The showcase's scripted detection: northwind.example publishes its mail and a calendar
        # (autodetect_showcase.rs), and contacts are looked for at the calendar's server.
        Open-AccountCard -Address 'eva@northwind.example'
        try {
          foreach ($use in 'Mail', 'Calendar', 'Contacts') {
            $box = Find-UiaElement -AutomationId "SetupUse$use"
            Assert-True ($null -ne $box -and (Get-UiaToggle $box)) (
              "docs/account-autodetect.md rule 8: a use whose server was found starts on ($use)")
          }
          Assert-True ($null -eq (Find-UiaElement -AutomationId 'SetupUseCalendarServer')) (
            'a calendar that was found is named by its host, not asked for')
          Assert-True (Test-UiaShown 'SetupMailServerIMAP') 'the mail server that was found is named too'
          foreach ($field in 'ImapHost', 'ImapPort', 'SmtpHost', 'Username') {
            Assert-True (-not (Test-UiaShown $field)) (
              "detected servers are shown to confirm, not to retype ($field): the manual form edits them")
          }
          Set-UiaToggle (Find-UiaElement -AutomationId 'SetupUseMail')
          Assert-True (-not (Test-UiaShown 'SetupMailServerIMAP')) (
            'the mail servers follow the mail toggle, so an account not used for mail shows none')
          Set-UiaToggle (Find-UiaElement -AutomationId 'SetupUseCalendar')
          $contacts = Find-UiaElement -AutomationId 'SetupUseContacts'
          Assert-True ((Get-UiaToggle $contacts) -and -not $contacts.Current.IsEnabled) (
            'the last use on is held on: an account used for nothing is not one')
          Set-UiaToggle (Find-UiaElement -AutomationId 'SetupUseMail') -On
          Assert-True (Test-UiaShown 'SetupMailServerIMAP') 'switching mail back on brings its servers back'
          Invoke-UiaElement (Find-UiaElement -AutomationId 'SetupManualButton' -Type 'Button')
          Assert-True ($null -ne (Wait-UiaElement -AutomationId 'ImapHost' -Type 'Edit' -TimeoutSec 5)) (
            '"Set up manually" from the found card offers its servers to edit')
        }
        finally { Close-AddAccount }
      }
    }
    @{
      Name = 'a calendar the domain does not publish starts off, and switched on asks for its server'
      Body = {
        # oldschool.example publishes its mail and no calendar.
        Open-AccountCard -Address 'eva@oldschool.example'
        try {
          $calendar = Find-UiaElement -AutomationId 'SetupUseCalendar'
          Assert-True ($null -ne $calendar -and -not (Get-UiaToggle $calendar)) (
            'rule 8: a use whose server was not found starts off')
          Assert-True (-not (Test-UiaShown 'SetupUseCalendarServer')) 'and asks for nothing while off'
          Set-UiaToggle $calendar -On
          Assert-True (Test-UiaShown 'SetupUseCalendarServer') 'switching it on asks for the server'
        }
        finally { Close-AddAccount }
      }
    }
    @{
      Name = 'a domain with a calendar and address book and no mail gets a card of its own'
      Body = {
        # cloud.example publishes CalDAV and CardDAV and no mail server.
        Open-AccountCard -Address 'eva@cloud.example'
        try {
          Assert-True ($null -eq (Find-UiaElement -AutomationId 'SetupUseMail')) (
            'docs/account-autodetect.md rule 8: the calendar-and-contacts route offers no mail')
          foreach ($use in 'Calendar', 'Contacts') {
            $box = Find-UiaElement -AutomationId "SetupUse$use"
            Assert-True ($null -ne $box -and (Get-UiaToggle $box)) "both were found, so both start on ($use)"
          }
          Assert-True (-not (Test-UiaShown 'ImapHost')) 'and the card draws no mail server'
          Assert-True (-not (Test-UiaShown 'DavLogin')) (
            'the typed address is the login, so the card does not ask for one')
          Assert-True (Test-UiaShown 'DavPassword') 'it asks for the password alone'
        }
        finally { Close-AddAccount }
      }
    }
  )
}
