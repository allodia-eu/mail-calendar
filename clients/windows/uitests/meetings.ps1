# Helpers for the suites that organise meetings against the harness as two accounts
# (MAILCAL_DEV_ACCOUNT=stalwart-multi: Alice and Bob in one app): MeetingInvitees.Tests.ps1.
# Dot-sourced by a suite file; defines functions and nothing else.
#
# A meeting's real outcome is mail in somebody else's mailbox, which no surface of this app shows
# for an account it is not signed in to, so Wait-SchedulingMail reads it from the harness server
# over JMAP, as test-linux-ui.sh's wait_for_scheduling_mail does.

<#
.SYNOPSIS
Put the shell on the calendar's agenda list, and return the list.
.DESCRIPTION
The agenda rather than the grid because the grid is a drawn canvas whose events have no Invoke
pattern; the agenda's ItemClick runs the same OnOpenEvent.
#>
function Show-MeetingAgenda {
  $existing = Find-UiaElement -AutomationId 'CalendarAgenda'
  if ($existing) { return $existing }
  if (-not (Find-UiaElement -AutomationId 'CalendarViewMenu' -Type Button)) {
    Invoke-UiaElement (Find-UiaElement -AutomationId 'NavCalendar') -SettleMs 1500
    if (-not (Wait-UiaElement -AutomationId 'CalendarViewMenu' -TimeoutSec 30)) { throw 'the calendar did not come up' }
  }
  Invoke-UiaElement (Find-UiaElement -AutomationId 'CalendarViewMenu' -Type Button) -SettleMs 900
  # Scoped to the flyout by its class name: a desktop-wide MenuItem sweep collects every window's
  # Alt+Space menu too.
  $flyout = Find-UiaElements -Type 'Menu' -Root ([System.Windows.Automation.AutomationElement]::RootElement) |
    Where-Object { $_.Current.ClassName -eq 'MenuFlyout' } | Select-Object -First 1
  if (-not $flyout) { throw 'the view flyout did not open' }
  $item = Find-UiaElement -AutomationId 'CalendarViewAgenda' -Type 'MenuItem' -Root $flyout
  if (-not $item) { throw 'the view flyout offers no agenda' }
  Invoke-UiaElement $item -SettleMs 1500
  $agenda = Find-UiaElement -AutomationId 'CalendarAgenda'
  if (-not $agenda) { throw 'the agenda list did not come up' }
  return $agenda
}

<#
.SYNOPSIS
Every Text node's name under $Root.
#>
function Get-MeetingTexts {
  param([Parameter(Mandatory)] [object] $Root)
  foreach ($node in Get-UiaTree $Root) {
    if ($node.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text -and $node.Current.Name) {
      $node.Current.Name
    }
  }
}

<#
.SYNOPSIS
The open ContentDialog, as an element, never the whole window.
.DESCRIPTION
Identified by the CloseButton every ContentDialog carries, not as "the first Popup": the shell keeps
an empty Popup that sorts ahead of it, under which every "does not contain" assertion would pass.
#>
function Get-MeetingDialog {
  param([int] $TimeoutSec = 15)
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($watch.Elapsed.TotalSeconds -lt $TimeoutSec) {
    $popup = Get-UiaTree | Where-Object {
      $_.Current.ClassName -eq 'Popup' -and
      $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Window -and
      (Find-UiaElement -AutomationId 'CloseButton' -Type Button -Root $_)
    } | Select-Object -First 1
    if ($popup) { return $popup }
    Start-Sleep -Milliseconds 250
  }
  throw 'no dialog opened'
}

<#
.SYNOPSIS
Close every open dialog, bounded, so a case starts from the calendar.
#>
function Close-MeetingDialogs {
  for ($i = 0; $i -lt 4; $i++) {
    $button = Find-UiaElement -AutomationId 'CloseButton' -Type Button
    if (-not $button) { return }
    Invoke-UiaElement $button -SettleMs 800
  }
}

<#
.SYNOPSIS
Open the detail of the agenda row titled $Title whose calendar belongs to $Account; returns the dialog.
.DESCRIPTION
With two accounts on one harness, a meeting between them is two agenda rows with one title: the
organiser's copy and the invitee's. The detail names its calendar with the account's address, which
is how the right copy is told apart. Bob's copy reaches his calendar on his next calendar sync, so a
missing row presses Sync once and looks again.
#>
function Open-MeetingDetail {
  param([Parameter(Mandatory)] [string] $Title, [Parameter(Mandatory)] [string] $Account)
  Close-MeetingDialogs
  for ($attempt = 0; $attempt -lt 3; $attempt++) {
    $agenda = Show-MeetingAgenda
    $rows = @(Find-UiaElements -Type 'ListItem' -Root $agenda |
        Where-Object { @(Get-MeetingTexts $_) -contains $Title })
    foreach ($row in $rows) {
      Invoke-UiaElement $row -SettleMs 1500
      $dialog = Get-MeetingDialog
      if (@(Get-MeetingTexts $dialog) -match [regex]::Escape("($Account)")) { return $dialog }
      Close-MeetingDialogs
    }
    $sync = Find-UiaElement -AutomationId 'CalendarRefresh' -Type Button
    if (-not $sync) { throw 'the calendar offers no Sync button' }
    Invoke-UiaElement $sync -SettleMs 8000
  }
  throw "no copy of '$Title' in $Account's calendar"
}

<#
.SYNOPSIS
Scroll the open editor's form to its end, where the invitee section is.
#>
function Show-EditorEnd {
  param([Parameter(Mandatory)] [object] $Dialog)
  $scroller = Get-UiaTree $Dialog | Where-Object {
    $_.Current.ClassName -eq 'ScrollViewer' -and
    ($_.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName }) -match 'ScrollPattern' -and
    $_.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern).Current.VerticallyScrollable
  } | Select-Object -First 1
  if ($scroller) {
    $scroller.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern).SetScrollPercent(-1, 100)
    Start-Sleep -Milliseconds 800
  }
}

<#
.SYNOPSIS
An element under $Root by an AutomationId that carries an address (EventInviteeOptional:<email>).
.DESCRIPTION
Through Get-UiaTree, the recursive children walk, because a FindAll over a dialog's descendants
under-walks WinUI's tree.
#>
function Find-ById {
  param([Parameter(Mandatory)] [string] $Id, [object] $Root)
  $tree = if ($Root) { Get-UiaTree $Root } else { Get-UiaTree }
  $tree | Where-Object { $_.Current.AutomationId -eq $Id } | Select-Object -First 1
}

<#
.SYNOPSIS
Every catalog locale's value for one message key, so a case asserts the right message in whatever
language the harness run is in.
#>
function Get-MeetingCatalogValues {
  param([Parameter(Mandatory)] [string] $Key)
  $values = @()
  foreach ($file in Get-ChildItem -Path (Join-Path $PSScriptRoot '../../../messages') -Filter '*.json') {
    $json = Get-Content -LiteralPath $file.FullName -Raw -Encoding utf8 | ConvertFrom-Json
    if ($json.PSObject.Properties.Name -contains $Key) { $values += $json.$Key }
  }
  if (-not $values) { throw "no catalog locale defines '$Key'" }
  $values
}

<#
.SYNOPSIS
The subjects in a harness user's mailbox whose message carries a text/calendar part.
#>
function Get-SchedulingSubjects {
  param([Parameter(Mandatory)] [string] $Address)
  $user = $Address.Split('@')[0]
  $pair = "${Address}:harness-$user-pw"
  $headers = @{ Authorization = 'Basic ' + [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($pair)) }
  $session = Invoke-RestMethod -Uri 'http://127.0.0.1:28080/jmap/session' -Headers $headers -TimeoutSec 10
  $account = $session.primaryAccounts.'urn:ietf:params:jmap:mail'
  $body = @{
    using       = @('urn:ietf:params:jmap:core', 'urn:ietf:params:jmap:mail')
    methodCalls = @(
      , @('Email/query', @{ accountId = $account; limit = 100 }, 'q')
      , @('Email/get', @{
          accountId  = $account
          '#ids'     = @{ resultOf = 'q'; name = 'Email/query'; path = '/ids' }
          properties = @('subject', 'bodyStructure')
        }, 'g')
    )
  } | ConvertTo-Json -Depth 12
  $response = Invoke-RestMethod -Uri 'http://127.0.0.1:28080/jmap/' -Method Post -Headers $headers `
    -ContentType 'application/json' -Body $body -TimeoutSec 10
  foreach ($mail in $response.methodResponses[1][1].list) {
    if (($mail.bodyStructure | ConvertTo-Json -Depth 20 -Compress) -match 'text/calendar') { $mail.subject }
  }
}

<#
.SYNOPSIS
Wait until $Address holds a scheduling message whose subject starts with $Prefix and names $Title.
.DESCRIPTION
The prefix ("Invitation:", "Cancelled:", "Tentative:") is the server's own subject line, written
by Stalwart in English whatever language this app runs in.
#>
function Wait-SchedulingMail {
  param(
    [Parameter(Mandatory)] [string] $Address,
    [Parameter(Mandatory)] [string] $Prefix,
    [Parameter(Mandatory)] [string] $Title,
    [int] $TimeoutSec = 60
  )
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($watch.Elapsed.TotalSeconds -lt $TimeoutSec) {
    if (@(Get-SchedulingSubjects $Address) | Where-Object { $_.StartsWith($Prefix) -and $_.Contains($Title) }) { return }
    Start-Sleep -Seconds 2
  }
  throw "no '$Prefix $Title' with a calendar part reached $Address within ${TimeoutSec}s"
}

<#
.SYNOPSIS
Whether no scheduling message with $Prefix and $Title is in $Address's mailbox.
#>
function Test-NoSchedulingMail {
  param([Parameter(Mandatory)] [string] $Address, [Parameter(Mandatory)] [string] $Prefix, [Parameter(Mandatory)] [string] $Title)
  -not (@(Get-SchedulingSubjects $Address) | Where-Object { $_.StartsWith($Prefix) -and $_.Contains($Title) })
}

<#
.SYNOPSIS
Destroy every message in $Address's mailbox whose subject names $Title.
.DESCRIPTION
A meeting's invitations, cancellations and replies are real mail in the seeded mailboxes, and other
harness suites find their rows by sight: a reply left at the top of Alice's inbox pushes a seeded
conversation below the fold and fails a suite that has nothing to do with meetings. So a suite
removes the mail it caused before it finishes.
#>
function Remove-SchedulingMail {
  param([Parameter(Mandatory)] [string] $Address, [Parameter(Mandatory)] [string] $Title)
  $user = $Address.Split('@')[0]
  $pair = "${Address}:harness-$user-pw"
  $headers = @{ Authorization = 'Basic ' + [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($pair)) }
  $session = Invoke-RestMethod -Uri 'http://127.0.0.1:28080/jmap/session' -Headers $headers -TimeoutSec 10
  $account = $session.primaryAccounts.'urn:ietf:params:jmap:mail'
  $body = @{
    using       = @('urn:ietf:params:jmap:core', 'urn:ietf:params:jmap:mail')
    methodCalls = @(
      , @('Email/query', @{ accountId = $account; filter = @{ subject = $Title } }, 'q')
      , @('Email/set', @{
          accountId  = $account
          '#destroy' = @{ resultOf = 'q'; name = 'Email/query'; path = '/ids' }
        }, 's')
    )
  } | ConvertTo-Json -Depth 12
  $null = Invoke-RestMethod -Uri 'http://127.0.0.1:28080/jmap/' -Method Post -Headers $headers `
    -ContentType 'application/json' -Body $body -TimeoutSec 10
}
