# The question a calendar paused for a server on another site asks (docs/account-autodetect.md
# rule 14), on both of the surfaces that ask it: the calendar's own line, whose Review opens a
# dialog, and the account's page in Settings, which asks inline because Settings is itself a dialog.
#
# WHY THE SHOWCASE. MAILCAL_SHOWCASE_SERVER_CHANGE (debug builds, read in the core) pauses the first
# account's calendar for a move from example.com to example.net, so everything from the
# connectivity snapshot to the question is real, and CI runs it. A showcase account has no servers
# to change, so the core refuses the allow, and that refusal being said is what proves Allow
# reached it. The harness suite (Mailcal.UiTests ServerChangeTests) drives the allow that moves an
# account; the calendar's line is out of its reach, and this is where it is asserted.
#
# Neither key a person might press to get past the dialog allows anything: Escape is its close
# response and Enter its default, and both are "Not now". The twins are
# clients/apple/UITests/ServerChangeTests.swift and the server-change leg of
# scripts/dev/test-linux-ui.sh.

$ServerChangeCatalog = Get-Content (Join-Path $PSScriptRoot '../../../messages/en.json') -Raw | ConvertFrom-Json
$ServerChangePaused = $ServerChangeCatalog.settings_account_use_server_changed_calendar
$ServerChangeTitle = $ServerChangeCatalog.server_changed_title_calendar
$ServerChangeReview = $ServerChangeCatalog.server_changed_review
$ServerChangeBeforeLine = $ServerChangeCatalog.server_changed_before.Replace('{site}', 'example.com')
$ServerChangeNowLine = $ServerChangeCatalog.server_changed_now.Replace('{site}', 'example.net')
$ServerChangeRefused = $ServerChangeCatalog.server_changed_failed

<#
.SYNOPSIS
The calendar's line for the paused calendar, with the calendar on screen.
#>
function Get-ServerChangeLine {
  Invoke-UiaElement (Find-UiaElement -AutomationId 'NavCalendar') -SettleMs 1500
  $line = Wait-UiaElement -AutomationId 'CalendarOutageBanner' -TimeoutSec 30
  if (-not $line) {
    throw 'the calendar draws no line for its paused use. Is this a DEBUG build? MAILCAL_SHOWCASE_SERVER_CHANGE is compiled out of Release'
  }
  $line
}

<#
.SYNOPSIS
Whether any element under $Root is named $Text, or, with -Within, has a name containing it.
#>
function Test-ServerChangeSays {
  param([object] $Root, [string] $Text, [switch] $Within)
  @(Find-UiaElements -Root $Root | Where-Object {
      if ($Within) { $_.Current.Name.Contains($Text) } else { $_.Current.Name -eq $Text }
    }).Count -gt 0
}

<#
.SYNOPSIS
Presses Review on the calendar's line and returns the dialog it opens, checked for both sites.
#>
function Open-ServerChangePrompt {
  $line = Get-ServerChangeLine
  $review = Find-UiaElement -AutomationId 'CalendarOutageAction' -Root $line
  if (-not $review) { throw 'the paused line has no action button (#CalendarOutageAction)' }
  Assert-Equal $ServerChangeReview $review.Current.Name 'a paused calendar offers Review, not Account settings'
  Invoke-UiaElement $review -SettleMs 1500
  $prompt = Wait-UiaElement -AutomationId 'ServerChangePrompt' -TimeoutSec 20
  if (-not $prompt) { throw 'Review opened no question (#ServerChangePrompt)' }
  Assert-True (Test-ServerChangeSays -Root $prompt -Text $ServerChangeTitle) 'the dialog is titled for the calendar'
  Assert-Equal $ServerChangeBeforeLine (Find-UiaElement -AutomationId 'ServerChangeBefore' -Root $prompt).Current.Name 'the dialog names the site the calendar used'
  Assert-Equal $ServerChangeNowLine (Find-UiaElement -AutomationId 'ServerChangeNow' -Root $prompt).Current.Name 'the dialog names the site it was found on'
  $prompt
}

<#
.SYNOPSIS
Answers the open dialog with $Key and asserts it closed with the calendar still paused.
#>
function Assert-ServerChangeKeyLeavesItPaused {
  param([string] $Key)
  Send-WinappKeys $Key
  Assert-True (Wait-UiaGone -AutomationId 'ServerChangePrompt' -TimeoutSec 20) "$Key answers the question"
  $line = Get-ServerChangeLine
  Assert-True (Test-ServerChangeSays -Root $line -Text $ServerChangePaused -Within) "and after $Key the calendar is still paused: $Key is Not now, never Allow"
  Assert-Equal $ServerChangeReview (Find-UiaElement -AutomationId 'CalendarOutageAction' -Root $line).Current.Name 'and Review is offered again'
}

$Suite = @{
  Dataset = 'showcase'
  Env     = @{ MAILCAL_SHOWCASE_SERVER_CHANGE = '1' }
  Cases   = @(
    @{
      Name = 'the calendar says it is paused, and Review asks naming both sites'
      Body = {
        $line = Get-ServerChangeLine
        Assert-True (Test-ServerChangeSays -Root $line -Text $ServerChangePaused -Within) 'the calendar line says the calendar is paused for a new server'
        $null = Open-ServerChangePrompt
        Assert-ServerChangeKeyLeavesItPaused -Key 'esc'
      }
    },
    @{
      Name = 'Enter is Not now too, so a stray Return allows nothing'
      Body = {
        $null = Open-ServerChangePrompt
        Assert-ServerChangeKeyLeavesItPaused -Key 'enter'
      }
    },
    @{
      Name = 'the account page asks inline, and a refused Allow says so and keeps asking'
      Body = {
        $dialog = Open-SettingsAccount -Index 0
        try {
          Assert-True (Test-ServerChangeSays -Root $dialog -Text $ServerChangePaused) 'the calendar switch on the account page says it is paused'
          $review = Find-UiaElement -AutomationId 'AccountUseServerChangeReview_Calendar' -Root $dialog
          if (-not $review) { throw 'the account page offers no Review under the calendar (#AccountUseServerChangeReview_Calendar)' }
          Invoke-UiaElement $review -SettleMs 1500
          # The question is a layout panel, which has no automation peer: it is found by its parts.
          $dialog = Get-SettingsDialog
          $allow = Find-UiaElement -AutomationId 'ServerChangeAllow' -Type 'Button' -Root $dialog
          if (-not $allow) { throw 'Review drew no question on the account page (#ServerChangeAllow)' }
          Assert-Equal $ServerChangeBeforeLine (Find-UiaElement -AutomationId 'ServerChangeBefore' -Root $dialog).Current.Name 'the inline question names the site the calendar used'
          Assert-Equal $ServerChangeNowLine (Find-UiaElement -AutomationId 'ServerChangeNow' -Root $dialog).Current.Name 'and the site it was found on'
          Invoke-UiaElement $allow -SettleMs 2000
          $said = $false
          for ($i = 0; $i -lt 40 -and -not $said; $i++) {
            $errorBar = Find-UiaElement -AutomationId 'ServerChangeError' -Root (Get-SettingsDialog)
            $said = $errorBar -and (Test-ServerChangeSays -Root $errorBar -Text $ServerChangeRefused)
            if (-not $said) { Start-Sleep -Milliseconds 250 }
          }
          Assert-True $said 'a refused Allow says nothing was changed, where it was asked'
          $dialog = Get-SettingsDialog
          Assert-True ($null -ne (Find-UiaElement -AutomationId 'ServerChangeAllow' -Type 'Button' -Root $dialog)) 'and the question stays open to be answered again'
          Assert-True (Test-ServerChangeSays -Root $dialog -Text $ServerChangePaused) 'and the calendar is still paused'
        }
        finally {
          if ($settings = Find-SettingsDialog) {
            Invoke-UiaElement (Find-UiaElement -AutomationId 'CloseButton' -Type 'Button' -Root $settings) -SettleMs 1200
          }
        }
      }
    }
  )
}
