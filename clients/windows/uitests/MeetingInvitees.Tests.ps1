# Organising a meeting from the event editor (docs/calendar.md §4, "Organising a meeting"): inviting
# people to a new event, as required or optional, changing who is invited, and the cancellations a
# removal and a delete send.
#
# WHY THE HARNESS, AS TWO ACCOUNTS. What an invitation does is mail in the invitee's mailbox, sent by
# the server once the meeting is stored, so the assertion is read from Bob's and Dave's mailboxes over
# JMAP (meetings.ps1), not from anything this app draws. stalwart-multi puts Bob's account in the app
# beside Alice's, so his address completes from the people index as To does.
#
# LANGUAGE. The app runs in the developer's language, so copy is matched against every catalog locale
# (Get-MeetingCatalogValues), and the "Invitation:" / "Cancelled:" prefixes are the server's.
#
# STATE. Each run creates its own meeting under a title with the time in it and deletes it, and the
# mail it caused, at the end, so a rerun never reads the last one's mail.

. (Join-Path $PSScriptRoot 'meetings.ps1')

$Title = "Invitees $(Get-Date -Format 'yyyyMMdd-HHmmss')"
$Alice = 'alice@test.local'
$Bob = 'bob@test.local'
$Dave = 'dave@test.local'

<#
.SYNOPSIS
Open the new-event editor, scrolled to its invitee field; returns the dialog.
#>
function Open-NewEvent {
  Close-MeetingDialogs
  $null = Show-MeetingAgenda
  Invoke-UiaElement (Find-UiaElement -AutomationId 'CalendarNewEvent' -Type Button) -SettleMs 1500
  $dialog = Get-MeetingDialog
  Show-EditorEnd $dialog
  if (-not (Find-UiaElement -AutomationId 'EventInviteeField' -Root $dialog)) {
    throw "the editor offers no invitee field on $Alice's calendar, whose server sends invitations"
  }
  return $dialog
}

<#
.SYNOPSIS
Type into the invitee field with real keystrokes, so the field's own change handling runs.
#>
function Send-InviteeKeys {
  param([Parameter(Mandatory)] [string] $Text, [switch] $Enter)
  $field = Find-UiaElement -AutomationId 'EventInviteeField'
  Invoke-WinappClick $field
  Send-WinappKeys $Text -Text
  if ($Enter) { Send-WinappKeys enter }
  Start-Sleep -Milliseconds 800
}

function Get-FieldValue {
  (Find-UiaElement -AutomationId 'EventInviteeField').GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value
}

$Suite = @{
  Dataset = 'harness'
  Env     = @{ MAILCAL_DEV_ACCOUNT = 'stalwart-multi' }
  Cases   = @(
    @{
      Name = 'the organiser cannot invite themselves'
      Body = {
        $dialog = Open-NewEvent
        Assert-Equal $null (Find-UiaElement -AutomationId 'EventAttendeesHeading' -Root $dialog) `
          'a new event with nobody on it draws no attendee heading; one over nothing says we looked and found none'
        Send-InviteeKeys $Alice -Enter
        $refusal = (Find-UiaElement -AutomationId 'EventInviteeRefusal' -Root $dialog).Current.Name
        Assert-True ((Get-MeetingCatalogValues 'event_invitee_organiser') -contains $refusal) `
          "typing the organiser's own address says why it cannot be added (got '$refusal')"
        Assert-Equal $Alice (Get-FieldValue) 'a refused address stays in the field to be corrected'
        Assert-Equal $null (Find-ById "EventInviteeRemove:$Alice" $dialog) 'and is not on the list'
        Close-MeetingDialogs
      }
    },
    @{
      Name = 'a new meeting invites the people on its list, as required or optional'
      Body = {
        $dialog = Open-NewEvent
        # The title first, so Save is enabled: Enter in the field below must add, not save.
        Set-UiaText (Find-UiaElement -AutomationId 'EventTitleBox' -Root $dialog) $Title
        Show-EditorEnd $dialog
        # Bob from the suggestion list, as To completes him; Dave typed, and made optional.
        Send-InviteeKeys 'bo'
        $list = Wait-UiaElement -AutomationId 'RecipientSuggestions' -TimeoutSec 10
        if (-not $list) { throw 'typing a known name offers no suggestions' }
        $suggestion = Find-UiaElements -Type ListItem -Root $list |
          Where-Object { $_.Current.Name -like "*$Bob*" } | Select-Object -First 1
        if (-not $suggestion) { throw "the suggestions do not offer $Bob" }
        Invoke-WinappClick $suggestion
        Start-Sleep -Milliseconds 800
        Send-InviteeKeys $Dave -Enter
        Assert-True ($null -ne (Find-UiaElement -AutomationId 'PrimaryButton' -Type Button -Root $dialog)) `
          'Enter in the invitee field adds the address; it does not press Save and close the editor'
        foreach ($who in $Bob, $Dave) {
          Assert-True ($null -ne (Find-ById "EventInviteeRemove:$who" $dialog)) "$who is on the list"
        }
        Assert-Equal '' (Get-FieldValue) 'an added address leaves the field'
        Assert-True ($null -ne (Find-UiaElement -AutomationId 'EventAttendeesHeading' -Root $dialog)) `
          'with people on the list, the list has its heading'
        $optional = Find-ById "EventInviteeOptional:$Dave" $dialog
        $optional.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern).Toggle()
        # Enter in the now empty field is the dialog's again: it saves.
        Invoke-WinappClick (Find-UiaElement -AutomationId 'EventInviteeField' -Root $dialog)
        Send-WinappKeys enter
        if (-not (Wait-UiaGone -AutomationId 'EventInviteeField' -TimeoutSec 10)) {
          throw 'Enter in an empty invitee field must reach the default button and save'
        }
        Wait-SchedulingMail -Address $Bob -Prefix 'Invitation:' -Title $Title
        Wait-SchedulingMail -Address $Dave -Prefix 'Invitation:' -Title $Title
      }
    },
    @{
      Name = 'the optional invitee, and only they, is marked in the attendee list'
      Body = {
        $texts = @(Get-MeetingTexts (Open-MeetingDetail -Title $Title -Account $Alice))
        Assert-True ($texts -contains $Dave) 'Dave is on the roster'
        $marked = @($texts | Where-Object { (Get-MeetingCatalogValues 'event_attendee_optional') -contains $_ })
        Assert-Equal 1 $marked.Count 'one optional invitee, one mark; a required one carries none'
        Close-MeetingDialogs
      }
    },
    @{
      Name = 'removing an invitee cancels theirs alone, and the last one stays'
      Body = {
        $null = Open-MeetingDetail -Title $Title -Account $Alice
        Invoke-UiaElement (Find-UiaElement -AutomationId 'PrimaryButton' -Type Button) -SettleMs 1500
        $editor = Get-MeetingDialog
        Show-EditorEnd $editor
        Invoke-UiaElement (Find-ById "EventInviteeRemove:$Dave" $editor) -SettleMs 800
        $last = Find-ById "EventInviteeRemove:$Bob" $editor
        Assert-True (-not $last.Current.IsEnabled) 'a meeting keeps at least one invitee, so the last remove is inert'
        Invoke-UiaElement (Find-UiaElement -AutomationId 'PrimaryButton' -Type Button -Root $editor) -SettleMs 1500
        Wait-SchedulingMail -Address $Dave -Prefix 'Cancelled:' -Title $Title
        Assert-True (Test-NoSchedulingMail -Address $Bob -Prefix 'Cancelled:' -Title $Title) `
          'Bob is still invited, so he is sent no cancellation'
      }
    },
    @{
      Name = 'deleting the meeting tells whoever is still on it'
      Body = {
        $null = Open-MeetingDetail -Title $Title -Account $Alice
        Invoke-UiaElement (Find-UiaElement -AutomationId 'SecondaryButton' -Type Button) -SettleMs 1500
        Invoke-UiaElement (Find-UiaElement -AutomationId 'PrimaryButton' -Type Button) -SettleMs 1500
        Wait-SchedulingMail -Address $Bob -Prefix 'Cancelled:' -Title $Title
        foreach ($who in $Alice, $Bob, $Dave) { Remove-SchedulingMail -Address $who -Title $Title }
      }
    }
  )
}
