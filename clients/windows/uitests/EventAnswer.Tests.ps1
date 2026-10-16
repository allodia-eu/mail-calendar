# Answering a meeting from its event in the calendar (docs/calendar.md §4, "Answering from the
# calendar"): Accept, Maybe and Decline on the invitee's own copy, the detail staying open after an
# answer, and Decline asking first.
#
# WHY THE HARNESS, AS TWO ACCOUNTS. Alice organises a meeting and invites Bob, both signed in to this
# app (stalwart-multi), so Bob's copy reaches his calendar here and his answer reaches Alice's mailbox,
# which is read over JMAP (meetings.ps1). The assertion is that mail: an answer stored and told to
# nobody looks the same from Bob's side.
#
# LANGUAGE. Copy is matched against every catalog locale; the "Tentative:" / "Declined:" prefixes are
# the server's.
#
# STATE. Each run organises its own meeting under a title with the time in it, so the seeded
# invitations other suites read keep their answers, and removes it and the mail it caused at the end.

. (Join-Path $PSScriptRoot 'meetings.ps1')

$Title = "Answer $(Get-Date -Format 'yyyyMMdd-HHmmss')"
$Alice = 'alice@test.local'
$Bob = 'bob@test.local'

function Get-AnswerLine {
  (Find-UiaElement -AutomationId 'EventAnswerLine').Current.Name
}

function Get-AnswerButton {
  param([Parameter(Mandatory)] [string] $Id)
  Find-UiaElement -AutomationId $Id -Type Button
}

<#
.SYNOPSIS
Wait until the answer line reads one of $Values; returns what it read last.
#>
function Wait-AnswerLine {
  param([Parameter(Mandatory)] [string[]] $Values, [int] $TimeoutSec = 45)
  $watch = [Diagnostics.Stopwatch]::StartNew()
  $line = $null
  while ($watch.Elapsed.TotalSeconds -lt $TimeoutSec) {
    $line = Get-AnswerLine
    if ($Values -contains $line) { return $line }
    Start-Sleep -Milliseconds 500
  }
  throw "the answer line never read one of '$($Values -join "', '")' (last: '$line')"
}

$Suite = @{
  Dataset = 'harness'
  Env     = @{ MAILCAL_DEV_ACCOUNT = 'stalwart-multi' }
  Cases   = @(
    @{
      Name = 'the organiser is offered no answer to their own meeting'
      Body = {
        Close-MeetingDialogs
        $null = Show-MeetingAgenda
        Invoke-UiaElement (Find-UiaElement -AutomationId 'CalendarNewEvent' -Type Button) -SettleMs 1500
        $editor = Get-MeetingDialog
        Show-EditorEnd $editor
        Invoke-WinappClick (Find-UiaElement -AutomationId 'EventInviteeField' -Root $editor)
        Send-WinappKeys $Bob -Text
        Send-WinappKeys enter
        Start-Sleep -Milliseconds 800
        Set-UiaText (Find-UiaElement -AutomationId 'EventTitleBox' -Root $editor) $Title
        Invoke-UiaElement (Find-UiaElement -AutomationId 'PrimaryButton' -Type Button -Root $editor) -SettleMs 1500
        Wait-SchedulingMail -Address $Bob -Prefix 'Invitation:' -Title $Title

        $detail = Open-MeetingDetail -Title $Title -Account $Alice
        Assert-Equal $null (Find-UiaElement -AutomationId 'EventAnswerLine' -Root $detail) `
          'Alice called the meeting; she attends by definition and has nothing to answer'
        Close-MeetingDialogs
      }
    },
    @{
      Name = "the invitee's detail opens on its default button, not on an answer"
      Body = {
        $null = Open-MeetingDetail -Title $Title -Account $Bob
        Assert-True ((Get-MeetingCatalogValues 'invitation_response_needs_action') -contains (Get-AnswerLine)) `
          'Bob has not answered, and the detail says so'
        # A ContentDialog focuses the first focusable control of its content, which is Accept; Enter
        # would then accept a meeting where it should press the default button.
        Start-Sleep -Milliseconds 500
        $focused = [System.Windows.Automation.AutomationElement]::FocusedElement
        Assert-Equal 'PrimaryButton' $focused.Current.AutomationId 'the initial focus is the default button'
        Close-MeetingDialogs
      }
    },
    @{
      Name = 'Maybe answers at once, and the detail stays open showing it'
      Body = {
        $null = Open-MeetingDetail -Title $Title -Account $Bob
        Invoke-UiaElement (Get-AnswerButton 'EventAnswerTentative') -SettleMs 500
        $null = Wait-AnswerLine (Get-MeetingCatalogValues 'invitation_response_tentative')
        Assert-True ($null -ne (Find-UiaElement -AutomationId 'CloseButton' -Type Button)) 'the detail stays open after Maybe'
        Assert-True (-not (Get-AnswerButton 'EventAnswerTentative').Current.IsEnabled) 'the answer given is inert'
        Assert-True ((Get-AnswerButton 'EventAnswerAccept').Current.IsEnabled) 'the others can still change it'
        Wait-SchedulingMail -Address $Alice -Prefix 'Tentative:' -Title $Title
      }
    },
    @{
      Name = 'Decline asks first, and backing out sends nothing'
      Body = {
        $null = Open-MeetingDetail -Title $Title -Account $Bob
        Invoke-UiaElement (Get-AnswerButton 'EventAnswerDecline') -SettleMs 1500
        $confirm = Find-UiaElement -AutomationId 'EventDeclineConfirm'
        if (-not $confirm) { throw 'Decline sent at once, without asking' }
        Invoke-UiaElement (Find-UiaElement -AutomationId 'CloseButton' -Type Button -Root $confirm) -SettleMs 2000
        # Backing out reopens the detail rather than leaving the user on the calendar.
        Assert-True ((Get-MeetingCatalogValues 'invitation_response_tentative') -contains (Get-AnswerLine)) `
          'cancelling the question returns to the detail, answer unchanged'
        Start-Sleep -Seconds 3
        Assert-True (Test-NoSchedulingMail -Address $Alice -Prefix 'Declined:' -Title $Title) 'a cancelled decline tells nobody'
      }
    },
    @{
      Name = 'a confirmed decline tells the organiser and takes the meeting off the calendar'
      Body = {
        $null = Open-MeetingDetail -Title $Title -Account $Bob
        Invoke-UiaElement (Get-AnswerButton 'EventAnswerDecline') -SettleMs 1500
        $confirm = Find-UiaElement -AutomationId 'EventDeclineConfirm'
        Invoke-UiaElement (Find-UiaElement -AutomationId 'PrimaryButton' -Type Button -Root $confirm) -SettleMs 2000
        Assert-Equal $null (Find-UiaElement -AutomationId 'CloseButton' -Type Button) 'the detail closes with the meeting'
        Wait-SchedulingMail -Address $Alice -Prefix 'Declined:' -Title $Title
        $agenda = Show-MeetingAgenda
        $copies = @(Find-UiaElements -Type 'ListItem' -Root $agenda | Where-Object { @(Get-MeetingTexts $_) -contains $Title })
        Assert-Equal 1 $copies.Count "only Alice's copy is left; a declined meeting is hidden from Bob's calendar"

        # Alice's copy goes too, so the next run starts from a clean calendar.
        $null = Open-MeetingDetail -Title $Title -Account $Alice
        Invoke-UiaElement (Find-UiaElement -AutomationId 'SecondaryButton' -Type Button) -SettleMs 1500
        Invoke-UiaElement (Find-UiaElement -AutomationId 'PrimaryButton' -Type Button) -SettleMs 1500
        foreach ($who in $Alice, $Bob) { Remove-SchedulingMail -Address $who -Title $Title }
      }
    }
  )
}
