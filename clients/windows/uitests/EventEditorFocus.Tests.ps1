# Where the event editor's caret opens (docs/calendar.md §11): a NEW event starts in its title, an
# EDIT does not.
#
# WHY IT IS HERE AND NOT IN `Mailcal.Tests`. EventEditorState, which is linked there, and which
# owns every other decision this dialog makes, knows whether it is editing, and nothing else. Who
# takes focus is a property of the WinUI ContentDialog, and that assembly cannot link one.
#
# WHY BOTH HALVES ARE ASSERTED, AND WHY THE EDIT HALF IS NOT REDUNDANT. §11 warns that a toolkit
# silently drops a focus request made before the surface is on screen; the less obvious failure on
# this platform is the opposite one. A ContentDialog whose content holds a focusable control
# focuses the FIRST one of its own accord, and the first one here is the title, so the edit half
# cannot be satisfied by withholding the request, and a client that simply did not ask would look
# correct in code and be wrong on screen. That is the state this file was written against.
#
# WHY HARNESS. The edit half needs an event that already exists. The harness seeds a calendar
# (docker/stalwart/seed/calendar); nothing here writes to it, the editor is opened and cancelled.

<#
.SYNOPSIS
The open ContentDialog, as an element, never the whole window.
.DESCRIPTION
Identified by the CloseButton every ContentDialog carries, not as "the first Popup": the shell keeps
a second, empty Popup that sorts ahead of it, and taking that one hands back an element with nothing
in it, under which every "X is not focused" assertion passes for the wrong reason. (The same trap,
at more length, in Attendees.Tests.ps1, each suite defines its own copy because the runner loads
one file at a time under -Filter.)
#>
function Get-DialogRoot {
  param([int] $TimeoutSec = 20)
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($watch.Elapsed.TotalSeconds -lt $TimeoutSec) {
    $popup = Get-UiaTree |
      Where-Object {
        $_.Current.ClassName -eq 'Popup' -and
        $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Window -and
        (Find-UiaElement -AutomationId 'CloseButton' -Type Button -Root $_)
      } | Select-Object -First 1
    if ($popup) { return $popup }
    Start-Sleep -Milliseconds 250
  }
  throw 'no dialog appeared'
}

function Close-Dialog {
  $close = Find-UiaElement -AutomationId 'CloseButton' -Type Button
  if ($close) { Invoke-UiaElement $close -SettleMs 900 }
  # A detail dialog can sit behind the editor; clear it too, so the next case starts on the agenda.
  $second = Find-UiaElement -AutomationId 'CloseButton' -Type Button
  if ($second) { Invoke-UiaElement $second -SettleMs 900 }
}

# Put the shell on the calendar, idempotently (the dataset launches on mail).
function Show-Calendar {
  if (Find-UiaElement -AutomationId 'CalendarViewMenu' -Type Button) { return }
  $nav = Find-UiaElement -AutomationId 'NavCalendar'
  if (-not $nav) { throw 'no Calendar entry in the navigation pane' }
  Invoke-UiaElement $nav -SettleMs 1500
  if (-not (Wait-UiaElement -AutomationId 'CalendarViewMenu' -TimeoutSec 30)) {
    throw 'the calendar did not come up'
  }
}

<#
.SYNOPSIS
Switch the calendar to the agenda list, and return it.
.DESCRIPTION
The agenda rather than the grid because the grid is a DRAWN canvas: its events are automation peers
with no Invoke pattern, so UIA can read one but cannot open it. The agenda is a real ListView whose
ItemClick runs the same OnOpenEvent. The Agenda entry is taken as the LAST item of the view flyout
so this survives whatever language the app is in; the count is asserted first, so a reordered menu
fails here rather than silently opening the month.
#>
function Show-Agenda {
  Show-Calendar
  $existing = Find-UiaElement -AutomationId 'CalendarAgenda'
  if ($existing) { return $existing }
  Invoke-UiaElement (Find-UiaElement -AutomationId 'CalendarViewMenu' -Type Button) -SettleMs 900
  # Scoped to the flyout's own MenuFlyout: a desktop-wide MenuItem sweep also collects every other
  # window's Alt+Space system menu, and the count then varies by what else is open.
  $flyout = Find-UiaElements -Type 'Menu' -Root ([System.Windows.Automation.AutomationElement]::RootElement) |
    Where-Object { $_.Current.ClassName -eq 'MenuFlyout' } | Select-Object -First 1
  if (-not $flyout) { throw 'the view flyout did not open' }
  $items = @(Find-UiaElements -Type 'MenuItem' -Root $flyout)
  Assert-Equal 6 $items.Count 'the view flyout is day / 3 days / work week / week / month / agenda, if that changed, the positional pick below is opening something else'
  Invoke-UiaElement $items[-1] -SettleMs 1500
  $agenda = Find-UiaElement -AutomationId 'CalendarAgenda'
  if (-not $agenda) { throw 'the agenda list did not come up' }
  return $agenda
}

<#
.SYNOPSIS
$true once the title box in $Dialog reports keyboard focus, polling until -TimeoutSec.
.DESCRIPTION
By AutomationId, never by the header text: the header is localized, so a Name match would assert
the app's language as much as its focus. Polled because the request is made on Opened, which is
raised after the dialog has finished presenting.
#>
function Wait-TitleFocus {
  param([Parameter(Mandatory)] [object] $Dialog, [int] $TimeoutSec = 10)
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($watch.Elapsed.TotalSeconds -lt $TimeoutSec) {
    $box = Find-UiaElement -AutomationId 'EventTitleBox' -Root $Dialog
    if ($box -and $box.Current.HasKeyboardFocus) { return $true }
    Start-Sleep -Milliseconds 200
  }
  return $false
}

# The title box, asserted present. Both cases need it on screen before its focus means anything,
# "the caret is not in the title" is otherwise satisfied by a dialog that never opened.
function Get-TitleBox {
  param([Parameter(Mandatory)] [object] $Dialog)
  $box = Find-UiaElement -AutomationId 'EventTitleBox' -Root $Dialog
  if (-not $box) { throw 'the event editor has no title field (#EventTitleBox), the dialog on screen is not the editor' }
  return $box
}

$Suite = @{
  Dataset = 'harness'
  Cases   = @(
    @{
      Name = 'a new event opens with the caret in its title'
      Body = {
        Show-Calendar
        $new = Find-UiaElement -AutomationId 'CalendarNewEvent'
        if (-not $new) { throw 'no New event button on the calendar' }
        Assert-True $new.Current.IsEnabled `
          'New event is disabled, so this case cannot reach the editor, the harness account should have a writable calendar'
        Invoke-UiaElement $new -SettleMs 1500
        try {
          $dialog = Get-DialogRoot
          Get-TitleBox -Dialog $dialog | Out-Null
          Assert-True (Wait-TitleFocus -Dialog $dialog) `
            'a new event opened without the caret in its title, the one field it cannot be saved without, and the same rule the composer To follows (docs/calendar.md §11)'
        }
        finally { Close-Dialog }
      }
    },
    @{
      Name = 'editing an existing event does not put the caret in its title'
      Body = {
        $agenda = Show-Agenda
        $rows = @(Find-UiaElements -Type 'ListItem' -Root $agenda)
        if ($rows.Count -eq 0) { throw 'the agenda is empty, so there is no event to edit (scripts/dev/harness.sh up seeds one)' }
        # Take the first row that actually offers Edit, rather than row 0. The agenda merges every
        # account's events in time order, and the detail withholds Edit on a read-only one
        # (EventDetailDialog: `canWrite`), so row 0 belongs to whichever account happens to sort
        # first, and an extra account left in the dev store puts a foreign, unwritable event there.
        # Any editable event proves the rule; needing a *particular* one would couple this to the
        # seed's dates for no more proof.
        $edit = $null
        $detail = $null
        foreach ($row in $rows) {
          Invoke-UiaElement $row -SettleMs 1500
          $detail = Get-DialogRoot
          $edit = Find-UiaElement -AutomationId 'PrimaryButton' -Type Button -Root $detail
          if ($edit) { break }
          Close-Dialog   # read-only: no Edit to press, so put it away and try the next row
          $detail = $null
        }
        try {
          if (-not $edit) {
            throw ("no agenda row offered an Edit button, so none of the $($rows.Count) events on " +
              'screen sits on a writable calendar. The harness calendar IS writable, so either its ' +
              'events never reached the agenda (scripts/dev/harness.sh up) or the write capability ' +
              'regressed. The other cause is a dev store still holding a disconnected account, ' +
              'whose events crowd the window and offer no Edit, list them with: ' +
              'scripts/dev/store.sh sql (SELECT scope_key, COUNT(*) FROM event_index GROUP BY ' +
              'scope_key) --store dev')
          }
          Invoke-UiaElement $edit -SettleMs 1800

          $dialog = Get-DialogRoot
          Get-TitleBox -Dialog $dialog | Out-Null
          # Settle before reading. The focus this case forbids is placed by the dialog as it
          # presents, so reading immediately can catch the frame before it happens and pass for the
          # wrong reason.
          Start-Sleep -Milliseconds 1200
          $box = Get-TitleBox -Dialog $dialog
          Assert-True (-not $box.Current.HasKeyboardFocus) `
            'editing an event opened with the caret in its title: the event already has one, and on a touch host the keyboard that comes with it covers the dates the user opened the editor to change (docs/calendar.md §11). Withholding the focus request is not enough, a ContentDialog focuses the first focusable control in its content by itself.'
        }
        finally { Close-Dialog }
      }
    }
  )
}
