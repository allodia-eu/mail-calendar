# "The organiser wasn't told" when the answer came from the calendar: the question is put inside the
# open event detail, because the window's own bar sits under the dialog's overlay where nobody can
# reach it (docs/calendar.md, "Answering from the calendar"; docs/invitations.md).
#
# THE INPUT IS FAKED, THE SURFACE IS NOT. No harness server reports a failed iTIP delivery, so
# MAILCAL_FAKE_REPLY_DELIVERY substitutes the verdict in the core, as InvitationReplyPrompt.Tests.ps1
# does, and on the same CalDAV half, the only route that asks. This suite shares that suite's launch.
#
# ITS OWN INVITATION. The seeded ones belong to other suites ("Quarterly planning" must stay
# unanswered, "Vendor pitch" is declined and so off the calendar), so Prepare writes a meeting Bob
# organises straight into Alice's calendar collection, the way docker/stalwart/seed-calendar-week.sh
# seeds "Quarterly planning", and the last case removes it.

. (Join-Path $PSScriptRoot 'meetings.ps1')

$Stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$Title = "Calendar answer $Stamp"
$Uid = "windows-answer-$Stamp"
$Organizer = 'bob@test.local'
$EventUrl = "http://127.0.0.1:28080/dav/cal/alice%40test.local/default/$Uid.ics"
$DavHeaders = @{
  Authorization = 'Basic ' + [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes('alice@test.local:harness-alice-pw'))
}
# AppPaths.PrefsDir for MAILCAL_DEV_ACCOUNT=stalwart-imap, where a remembered answer is kept.
$PrefsFile = Join-Path $env:LOCALAPPDATA 'Allodia/MailCalendar/dev-imap/preferences.toml'

<#
.SYNOPSIS
Forget the account's standing answer, so the core asks rather than answering for the user.
#>
function Clear-RememberedReply {
  if (-not (Test-Path -LiteralPath $PrefsFile)) { return }
  $kept = @()
  $inTable = $false
  foreach ($line in Get-Content -LiteralPath $PrefsFile) {
    if ($line -match '^\s*\[') { $inTable = $line -match '^\s*\[invitation_reply_fallback\]\s*$' }
    if (-not $inTable) { $kept += $line }
  }
  Set-Content -LiteralPath $PrefsFile -Value $kept
}

<#
.SYNOPSIS
The detail's own reply bar while it is open and showing, or $null.
#>
function Get-DetailReplyBar {
  param([Parameter(Mandatory)] [object] $Dialog)
  $bar = Find-ById 'EventReplyPrompt' $Dialog
  if ($bar -and -not $bar.Current.IsOffscreen) { return $bar }
  $null
}

$Suite = @{
  Dataset = 'harness'
  Env     = @{
    MAILCAL_DEV_ACCOUNT         = 'stalwart-imap'
    MAILCAL_FAKE_REPLY_DELIVERY = 'failed:5.2'
  }
  Prepare = {
    Clear-RememberedReply
    $day = (Get-Date).AddDays(1).ToString('yyyyMMdd')
    $body = @(
      'BEGIN:VCALENDAR', 'VERSION:2.0', 'PRODID:-//Allodia//Windows UI suite//EN', 'BEGIN:VEVENT',
      "UID:$Uid@test.local", 'DTSTAMP:20260101T000000Z',
      "DTSTART;TZID=Europe/Amsterdam:${day}T140000", "DTEND;TZID=Europe/Amsterdam:${day}T150000",
      "SUMMARY:$Title",
      "ORGANIZER;CN=Bob Tester:mailto:$Organizer",
      "ATTENDEE;CN=Bob Tester;ROLE=CHAIR;PARTSTAT=ACCEPTED;RSVP=FALSE:mailto:$Organizer",
      'ATTENDEE;CN=Alice Tester;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;RSVP=TRUE:mailto:alice@test.local',
      'END:VEVENT', 'END:VCALENDAR', ''
    ) -join "`r`n"
    Invoke-RestMethod -Uri $EventUrl -Method Put -Headers $DavHeaders `
      -ContentType 'text/calendar; charset=utf-8' -Body ([Text.Encoding]::UTF8.GetBytes($body)) | Out-Null
  }
  Cases   = @(
    @{
      Name = 'a reported failure puts the question inside the open detail, naming the organiser'
      Body = {
        $dialog = Open-MeetingDetail -Title $Title -Account 'alice@test.local'
        Invoke-UiaElement (Find-UiaElement -AutomationId 'EventAnswerAccept' -Type Button -Root $dialog) -SettleMs 500
        $watch = [Diagnostics.Stopwatch]::StartNew()
        $bar = $null
        while (-not $bar -and $watch.Elapsed.TotalSeconds -lt 60) {
          Start-Sleep -Milliseconds 500
          $bar = Get-DetailReplyBar $dialog
        }
        if (-not $bar) { throw 'the server reported the answer undelivered and the open detail asked nothing' }
        $texts = @(Get-MeetingTexts $bar)
        $expected = Get-MeetingCatalogValues 'invitation_reply_undelivered_body' |
          ForEach-Object { $_.Replace('{summary}', $Title).Replace('{organizer}', $Organizer) }
        Assert-True ([bool]($texts | Where-Object { $expected -contains $_ })) `
          "the question names the meeting and who would be emailed, it shows: $($texts -join ' | ')"
      }
    },
    @{
      Name = 'answering it from the detail closes it'
      Body = {
        $dialog = Get-MeetingDialog
        $bar = Get-DetailReplyBar $dialog
        if (-not $bar) { throw 'the question is not on screen to answer' }
        $dismiss = Get-UiaTree $bar | Where-Object {
          $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Button -and
          (Get-MeetingCatalogValues 'invitation_reply_undelivered_dismiss') -contains $_.Current.Name
        } | Select-Object -First 1
        Invoke-UiaElement $dismiss -SettleMs 1500
        $watch = [Diagnostics.Stopwatch]::StartNew()
        while ((Get-DetailReplyBar $dialog) -and $watch.Elapsed.TotalSeconds -lt 30) { Start-Sleep -Milliseconds 300 }
        Assert-Equal $null (Get-DetailReplyBar $dialog) 'the core clears the question as it is answered, and the detail follows it'
        Assert-True ((Get-MeetingCatalogValues 'invitation_response_accepted') -contains (Find-UiaElement -AutomationId 'EventAnswerLine' -Root $dialog).Current.Name) `
          'the answer itself was stored; only telling the organiser failed'

        Close-MeetingDialogs
        Clear-RememberedReply
        Invoke-RestMethod -Uri $EventUrl -Method Delete -Headers $DavHeaders | Out-Null
        foreach ($who in 'alice@test.local', $Organizer) { Remove-SchedulingMail -Address $who -Title $Title }
      }
    }
  )
}
