# An event's detail, and its editor, list who is coming (docs/calendar.md §4, "The attendee list").
#
# WHY THIS NEEDS THE HARNESS, and what the harness does NOT prove. A showcase seed is a record the
# client is handed, so a showcase test would be asserting that a fixture has three entries in it. The
# harness account talks JMAP to Stalwart, and that is the shape which broke this feature once already:
# Stalwart decodes `ORGANIZER` into a participant carrying CHAIR, with no `owner` role anywhere on the
# event, so a projection asking only for `owner` marks nobody and the feature silently does nothing on
# a whole class of account. No hand-built record catches that, which is why these cases open a real
# meeting over a real transport. Verified by mutation: reverting the chair fallback to owner-only
# turns "only the organiser is marked as the organiser" red, and nothing else here notices.
#
# The core's other decision, merging a split `ORGANIZER` + matching `ATTENDEE` pair into ONE row,
# is deliberately NOT claimed by this suite, because this account cannot exercise it: JSCalendar
# merges the pair server-side, so the projection is handed a single participant and there is nothing
# left to fold. Disabling the merge entirely leaves every case here green (measured, not assumed).
# That rule is covered by the core's unit tests, and would need a CalDAV account to reach from here;
# the case below still guards the rendering half of it, one row per person on screen.
#
# ALMOST NOTHING BELOW MATCHES A LOCALISED STRING. A harness run comes up in the developer's own
# language, so asserting on "Organiser" or "No answer yet" would make this suite pass or fail by whose
# machine it ran on. Three language-free handles do the work instead:
#   * attendee NAMES and ADDRESSES, which come from docker/stalwart/seed-calendar-week.sh, not the
#     message catalog;
#   * the heading's AutomationId (EventAttendeesHeading), so "shows no heading at all" is assertable
#     in any language, an absence needs a handle;
#   * the answer labels compared to EACH OTHER rather than to a word. Two attendees seeded ACCEPTED
#     must read identically and the one seeded NEEDS-ACTION must read differently, which pins the
#     status mapping without naming a single string.
#
# THE SEEDED MEETING (docker/stalwart/seed-calendar-week.sh, `invited week-invited`), Monday 10:30:
#   ORGANIZER  Bob Tester     <bob@test.local>     + a matching ATTENDEE, CHAIR, ACCEPTED  -> ONE row
#   ATTENDEE   Alice Tester   <alice@test.local>   REQ-PARTICIPANT, NEEDS-ACTION
#   ATTENDEE   Carol External <carol@example.com>  REQ-PARTICIPANT, ACCEPTED

$Meeting = 'Quarterly planning'    # the seeded meeting that has attendees
$Plain = 'Lunch'                   # seeded the same day, with no ATTENDEE line at all
$Organizer = 'Bob Tester'
$OrganizerMail = 'bob@test.local'
$Unanswered = 'Alice Tester'
$UnansweredMail = 'alice@test.local'
$Accepted = 'Carol External'
$AcceptedMail = 'carol@example.com'

<#
.SYNOPSIS
Put the shell on the calendar (the dataset launches on mail), idempotently.
#>
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
with no Invoke pattern (Calendar/CalendarSurfaceAutomation.cs), so UIA can read one but cannot open
it. The agenda is a real ListView whose ItemClick runs the same OnOpenEvent, the same code path,
reached by the one route a script can drive.

The Agenda entry is taken as the LAST item of the view flyout rather than by its text, so this
survives whatever language the app is in; the count is asserted first, so a reordered menu fails here
rather than silently opening the month.
#>
function Show-Agenda {
  Show-Calendar
  $existing = Find-UiaElement -AutomationId 'CalendarAgenda'
  if ($existing) { return $existing }
  $menu = Find-UiaElement -AutomationId 'CalendarViewMenu' -Type Button
  Invoke-UiaElement $menu -SettleMs 900
  # The flyout hosts in its own top-level popup, so it is NOT under the app window, but a sweep of
  # the DESKTOP for MenuItem is not the answer either: every window on it exposes an Alt+Space
  # system menu as one, so the count came back 7, then 13, by what else happened to be open. It read
  # as "the view flyout changed", which is the one thing it had not.
  #
  # Scope to the flyout itself: its items hang off a Menu whose ClassName is MenuFlyout, which is
  # language-free and cannot collect another window's menu.
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
The open dialog, as an element, never the whole window.
.DESCRIPTION
Scoping matters more than it looks. A ContentDialog hosts in a Popup window over the shell, and the
agenda list is STILL in the tree behind it, drawn at overlapping heights: on the display this was
written on, the agenda's "Bug triage" row sits 2px from the dialog's "Alice Tester". Read the window
and a geometric join lands on the list behind the dialog, a false pass that looks like a real one.

The dialog is identified by the CloseButton every ContentDialog carries, NOT as "the first Popup":
the shell keeps a second, empty Popup (name 'Popup', one descendant) that sorts ahead of it in the
walk. Taking the first one hands back an element with no text in it at all, under which every
"the roster does not contain X" assertion passes, and passes for the wrong reason.
#>
function Get-DialogRoot {
  param([int] $TimeoutSec = 15)
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
  throw 'no dialog opened'
}

<#
.SYNOPSIS
Every Text node under $Root, with the y it is drawn at.
.DESCRIPTION
A node scrolled out of its dialog reports an INFINITE rectangle, which no comparison can use, the
editor's form is taller than its ScrollViewer, so that is the normal case there rather than an edge
one. Those nodes are kept, since the text is what most cases assert on, with Top = $null; the cases
that need geometry drop them.
#>
function Get-TextNodes {
  param([Parameter(Mandatory)] [object] $Root)
  foreach ($node in Get-UiaTree $Root) {
    if ($node.Current.ControlType -ne [System.Windows.Automation.ControlType]::Text) { continue }
    $name = $node.Current.Name
    if (-not $name) { continue }
    $top = $node.Current.BoundingRectangle.Top
    [pscustomobject]@{
      Text = $name
      Top  = $(if ([double]::IsInfinity($top)) { $null } else { [double] $top })
    }
  }
}

<#
.SYNOPSIS
Open an agenda event's detail dialog; returns the DIALOG, which every case then reads through.
.DESCRIPTION
Rows are matched on the text INSIDE them, never on the row's own Name: an agenda ListViewItem
announces `Allodia.Mailcal.ViewModels.EventItem`, because the DataTemplate carries no automation name
and the peer falls back to ToString(). That is a real accessibility defect, the calendar's half of
the one 0.13.5 fixed for the mailbox, but it predates this feature and is filed separately; it is
only mentioned here because it is why this lookup cannot be one line.
#>
function Open-EventDetail {
  param([Parameter(Mandatory)] [string] $Title)
  # Start from a known state. A case that throws leaves its dialog up, and a modal dialog swallows
  # the click on the next case's agenda row, so the failure would spread, and every case after the
  # first red one would report a fault it did not have.
  Close-Dialog
  $agenda = Show-Agenda
  $rows = @(Find-UiaElements -Type 'ListItem' -Root $agenda)
  Assert-GreaterThan 0 $rows.Count 'the agenda is empty, the harness calendar never synced'
  $match = $null
  foreach ($row in $rows) {
    if (@(Get-TextNodes $row | ForEach-Object { $_.Text }) -contains $Title) { $match = $row; break }
  }
  if (-not $match) {
    throw "no agenda row for '$Title', the living week is seeded relative to today, so a stale harness is the usual cause: scripts/dev/harness.sh reset"
  }
  Invoke-UiaElement $match -SettleMs 1500
  return Get-DialogRoot
}

<#
.SYNOPSIS
Close every open dialog, so the next case starts from the agenda.
.DESCRIPTION
A loop rather than one press: pressing Edit stacks the editor over the detail, and closing only the
top one leaves a modal still covering the agenda. Bounded, so a dialog that refuses to close fails
the case that opens the next one instead of hanging the suite.
#>
function Close-Dialog {
  for ($i = 0; $i -lt 4; $i++) {
    $button = Find-UiaElement -AutomationId 'CloseButton' -Type Button
    if (-not $button) { return }
    Invoke-UiaElement $button -SettleMs 800
  }
}

<#
.SYNOPSIS
The label an attendee's row shows for how they answered.
.DESCRIPTION
A row is a two-column Grid, who on the left, the answer on the right, so the answer is the Text node
drawn at the SAME y as the name, carrying different text. Read geometrically rather than by string
because the label is localized; the cases compare these to each other, never to a word.
#>
function Get-AnswerFor {
  param(
    [Parameter(Mandatory)] [object[]] $Nodes,
    [Parameter(Mandatory)] [string] $Name
  )
  $row = $Nodes | Where-Object { $_.Text -eq $Name -and $null -ne $_.Top } | Select-Object -First 1
  if (-not $row) { throw "'$Name' is not drawn, so there is no answer to read beside them" }
  $answer = $Nodes |
    Where-Object { $null -ne $_.Top -and [Math]::Abs($_.Top - $row.Top) -lt 2 -and $_.Text -ne $Name } |
    Select-Object -First 1
  if (-not $answer) { throw "'$Name' has no label beside them, an attendee row must say how they answered" }
  return $answer.Text
}

$Suite = @{
  Dataset = 'harness'
  Cases   = @(
    @{
      Name = 'a meeting lists everyone on it, organizer first'
      Body = {
        $nodes = @(Get-TextNodes (Open-EventDetail $Meeting))
        $texts = @($nodes | ForEach-Object { $_.Text })
        foreach ($who in $Organizer, $Unanswered, $Accepted) {
          Assert-True ($texts -contains $who) "the roster must name everyone the server put on the meeting, '$who' is missing"
        }
        # Order, read off the drawn y. The organiser is who a reader looks for first, so the core
        # sorts them to the top and everyone else keeps the order the event gave them.
        $ys = @{}
        foreach ($who in $Organizer, $Unanswered, $Accepted) {
          $node = $nodes | Where-Object { $_.Text -eq $who -and $null -ne $_.Top } | Select-Object -First 1
          if (-not $node) { throw "'$who' is in the tree but not drawn, the detail is clipping its own roster" }
          $ys[$who] = $node.Top
        }
        Assert-GreaterThan $ys[$Organizer] $ys[$Unanswered] 'the organizer sorts above every other attendee'
        Assert-GreaterThan $ys[$Unanswered] $ys[$Accepted] 'after the organizer, attendees keep the order the event gave them'
        Close-Dialog
      }
    },
    @{
      Name = 'the organizer appears once, not once per line the server sent'
      Body = {
        # The seed carries ORGANIZER:bob@ *and* a matching ATTENDEE;CN=Bob Tester;ROLE=CHAIR line, and
        # the roster must be one row per person: one longer than the "of N" tally beside it is a
        # discrepancy with nothing on screen to explain it.
        #
        # Read the header before strengthening this: over JMAP the two lines arrive already merged, so
        # what this case pins is the RENDERING (a row is drawn once per attendee, and the address line
        # with it), not the core's fold. Turning the fold off does not make it fail.
        $nodes = @(Get-TextNodes (Open-EventDetail $Meeting))
        Assert-Equal 1 (@($nodes | Where-Object { $_.Text -eq $Organizer }).Count) `
          'an ORGANIZER line and its matching ATTENDEE line are one person, so they are one row'
        Assert-Equal 1 (@($nodes | Where-Object { $_.Text -like "*$OrganizerMail*" }).Count) `
          'one row also means one address line, not two'
        Close-Dialog
      }
    },
    @{
      Name = 'the two who accepted read alike, and the one who has not reads differently'
      Body = {
        # The status mapping, pinned without naming a localised word. Bob (ACCEPTED, arrived through
        # the organiser merge) and Carol (ACCEPTED, plainly) must say the same thing; Alice
        # (NEEDS-ACTION) must not. A projection that flattened every status to one label fails the
        # second half; one that let the organiser inference re-answer for somebody fails the first.
        $nodes = @(Get-TextNodes (Open-EventDetail $Meeting))
        $organizerAnswer = Get-AnswerFor -Nodes $nodes -Name $Organizer
        $acceptedAnswer = Get-AnswerFor -Nodes $nodes -Name $Accepted
        $unansweredAnswer = Get-AnswerFor -Nodes $nodes -Name $Unanswered
        Assert-Equal $acceptedAnswer $organizerAnswer 'both accepted, so both rows say it in the same words'
        Assert-True ($acceptedAnswer -ne $unansweredAnswer) `
          "an attendee who has not replied may not read as one who accepted (both say '$acceptedAnswer')"
        Close-Dialog
      }
    },
    @{
      Name = 'only the organizer is marked as the organizer'
      Body = {
        # The second line is the address (when the first line used a name), then "Organiser". So the
        # organiser's line is strictly longer than their bare address and an ordinary attendee's is
        # exactly their address: it asserts the SHAPE, not the word.
        $nodes = @(Get-TextNodes (Open-EventDetail $Meeting))
        $texts = @($nodes | ForEach-Object { $_.Text })
        $organizerLine = $texts | Where-Object { $_ -like "$OrganizerMail*" } | Select-Object -First 1
        if (-not $organizerLine) { throw "the organizer's row shows no address line" }
        Assert-GreaterThan $OrganizerMail.Length $organizerLine.Length `
          'the organizer line says more than the bare address, that is where the organizer label goes'
        Assert-True ($texts -contains $UnansweredMail) `
          'an attendee shown by name carries their bare address beneath it, and nothing else'
        Close-Dialog
      }
    },
    @{
      Name = 'an event nobody was invited to shows no attendee heading at all'
      Body = {
        # Not "an empty list": an "Attendees" caption with nothing under it reads as "we looked and
        # found none", which is a different statement from "this is not a meeting".
        $dialog = Open-EventDetail $Plain
        $nodes = @(Get-TextNodes $dialog)
        $texts = @($nodes | ForEach-Object { $_.Text })
        foreach ($who in $Organizer, $Accepted, $AcceptedMail) {
          Assert-True ($texts -notcontains $who) "a plain appointment must not carry the roster of another event, found '$who'"
        }
        Assert-Equal $null (Find-UiaElement -AutomationId 'EventAttendeesHeading' -Type Text -Root $dialog) `
          'no attendees means no heading, an empty one would claim we looked and found none'
        Close-Dialog
      }
    },
    @{
      Name = 'the heading IS there when there are attendees'
      Body = {
        # The guard for the case above. Without it, a heading that stopped rendering entirely would
        # make "no heading on a plain appointment" pass while the feature was gone.
        $dialog = Open-EventDetail $Meeting
        if (-not (Find-UiaElement -AutomationId 'EventAttendeesHeading' -Type Text -Root $dialog)) {
          throw 'the meeting draws its roster with no heading over it, then the absence case above proves nothing'
        }
        Close-Dialog
      }
    },
    @{
      Name = 'the editor carries the same list, and says it cannot be changed there'
      Body = {
        # An edit made blind to who is coming is what this prevents. The editor's form is taller than
        # its ScrollViewer, so the roster is genuinely below the fold, the scroll below is what tells
        # "reachable" from "clipped away", which is the difference between a design and a bug.
        $null = Open-EventDetail $Meeting
        $edit = Find-UiaElement -AutomationId 'PrimaryButton' -Type Button
        if (-not $edit) { throw 'the detail offered no Edit button, the harness calendar should be writable' }
        Invoke-UiaElement $edit -SettleMs 1500
        $dialog = Get-DialogRoot

        $scroller = Get-UiaTree $dialog | Where-Object {
          $_.Current.ClassName -eq 'ScrollViewer' -and
          ($_.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName }) -match 'ScrollPattern' -and
          $_.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern).Current.VerticallyScrollable
        } | Select-Object -First 1
        if (-not $scroller) {
          throw 'the editor form does not scroll, then everything past the fold is unreachable rather than merely off-screen'
        }
        $scroller.GetCurrentPattern([System.Windows.Automation.ScrollPattern]::Pattern).SetScrollPercent(-1, 100)
        Start-Sleep -Milliseconds 900

        $heading = Find-UiaElement -AutomationId 'EventAttendeesHeading' -Type Text -Root $dialog
        if (-not $heading) { throw 'the editor shows no attendee list, an edit would be made blind to who is on the meeting' }
        $nodes = @(Get-TextNodes $dialog)
        $texts = @($nodes | ForEach-Object { $_.Text })
        foreach ($who in $Organizer, $Unanswered, $Accepted) {
          Assert-True ($texts -contains $who) "the editor shows the same roster as the detail, not a subset, '$who' is missing"
        }
        Assert-Equal 1 (@($nodes | Where-Object { $_.Text -eq $Organizer }).Count) `
          'the merge holds in the editor too, one row per person, not one per line'

        # The sentence that keeps a read-only surface honest, found by ELIMINATION rather than by its
        # wording: it is the one line below the heading that is not an attendee's name or address.
        # Asserting the localised string itself would tie this suite to one language.
        $roster = @($Organizer, $Unanswered, $Accepted, $UnansweredMail, $AcceptedMail)
        $note = $nodes | Where-Object {
          $null -ne $_.Top -and $_.Top -gt $heading.Current.BoundingRectangle.Top -and
          $_.Text -notin $roster -and $_.Text -notlike "*$OrganizerMail*" -and $_.Text -match '\s'
        } | Select-Object -First 1
        if (-not $note) {
          throw 'the editor lists attendees and says nothing about them being read-only, a surface that shows a thing and stays silent invites an edit it will drop'
        }
        Close-Dialog
      }
    }
  )
}
