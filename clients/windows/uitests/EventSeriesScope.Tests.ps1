#!/usr/bin/env pwsh
# Does a write on ONE occurrence actually ask which occurrences it meant? (docs/calendar.md §10.)
#
# WHY THIS SUITE EXISTS. `Mailcal.Tests` pins which of the two paths a tap takes, `EventOpenTests`
# asserts that a token means "ask" and no token means "the series", and it can go no further,
# because the question is a `ContentDialog` and that assembly links no WinUI. So a scope question
# that is decided correctly and never *raised* passes `dotnet test`, and the first person to find
# out is whoever deletes a standup meaning to cancel one Tuesday. That is the one outcome nobody
# can undo.
#
# WHY IT LOOKS LIKE THIS. The question is only reachable from a surface that names an occurrence,
# and the only such surface is the drawn grid: the agenda lists the series, so it names none (which
# is the second case below, and the control for the first). The grid is a canvas, its events are
# automation peers with **no Invoke pattern**, so UIA can read one and cannot open it.
#
# What UIA cannot do, the OS can. Every event peer publishes a live BOUNDING RECTANGLE in screen
# pixels, computed by the renderer's own geometry at the display's real scale
# (`CalendarItemPeer.GetBoundingRectangleCore`), and a left click injected with `mouse_event` lands
# on the surface's own pointer owner, the same instrument `CalendarWheel.Tests.ps1` uses on this
# same canvas. So the peer says where the block is and the OS does the clicking.
#
# NOT `GetClickablePoint()`, though the peer overrides `GetClickablePointCore`. UIA computes that
# property by hit-testing, and a hit test on a drawn node lands on the canvas that owns the pointer
# rather than on the node, so the client is told the element has no clickable point, for every peer
# on this surface, a plainly visible day header included. The rectangle is published and correct;
# its centre is the point.
#
# AND THE BLOCK HAS TO BE SCROLLED TO. The grid seats itself near the current time, so which hours
# are in view depends on when the suite is run. A block above the viewport still has a rectangle,
# and its y is then NEGATIVE, clicking that lands in the window chrome, several steps from
# anything that would name the cause. `Show-GridBlock` wheels the grid until the block sits in the
# timed area, which is what makes the click independent of the clock.
#
# WHAT IS NOT ASSERTED HERE. That the destructive answer removes the right occurrence: the oracle
# for that is the `EXDATE` the server ends up holding, which is a harness read this runner has no
# route to. This suite proves the question is put, and put only where it belongs.
#
# NOTHING BELOW MATCHES A LOCALISED STRING. A harness run comes up in the developer's own language.
# The handles are the seeded event TITLE (from docker/stalwart/seed-calendar-week.sh, not the
# catalog) and the BUTTON COUNT of the dialog, which is what tells the two questions apart: the
# scope question offers three answers, the generic delete confirmation two.
#
# NOTHING HERE WRITES. Every case ends by backing out, the destructive answers are never pressed,
# so the suite can be run against the seeded harness repeatedly without reseeding it.

# A series, so its blocks name an occurrence, and the DAILY one of the four the harness seeds
# (docker/stalwart/seed-calendar-week.sh), because which seven days the grid is showing is not
# something this suite can pin. A page is a week, but its ALIGNMENT is not fixed: it can be the
# Monday-aligned week or seven days running from today, depending on what the suite before this one
# left the pager on, these suites share one app launch. A daily series that starts on this week's
# Thursday and runs ten days is drawn on BOTH, whatever day the suite is run; the fortnightly one
# falls on Tuesday, and a today-anchored window on a Thursday leaves it behind.
$Repeating = 'Onboarding'   # FREQ=DAILY;COUNT=10, from this week's Thursday

Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SeriesScopeInput {
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, int dx, int dy, int data, UIntPtr extra);
  public const uint LEFTDOWN = 0x0002;
  public const uint LEFTUP   = 0x0004;
  public const uint WHEEL    = 0x0800;
}
'@ -ErrorAction SilentlyContinue

<#
.SYNOPSIS
Put the shell on the calendar, idempotently.
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
Put the calendar into one named shape, by AutomationId. Idempotent.
.DESCRIPTION
Every case here has to pin the shape rather than inherit it. The layout is the CORE's and the
calendar reopens the way it was left (docs/calendar.md §8), so what a launch comes up on is
whatever the last session, this suite's own agenda case included, happened to leave in the
store. A grid case that only checks the calendar is on screen therefore passes or fails by that,
which is the one thing the error messages below promise it does not do.

Named by AutomationId, never by position or label: the ids exist for exactly this
(CalendarView.xaml says so), a harness run comes up in the developer's own language, and a
positional pick silently follows a separator or a new zoom level to the wrong item.
#>
function Set-CalendarShape {
  param([Parameter(Mandatory)] [string] $AutomationId)

  Show-Calendar
  $menu = Find-UiaElement -AutomationId 'CalendarViewMenu' -Type Button
  if (-not $menu) { throw 'the calendar header offers no view menu' }
  Invoke-UiaElement $menu -SettleMs 900
  $flyout = Find-UiaElements -Type 'Menu' -Root ([System.Windows.Automation.AutomationElement]::RootElement) |
    Where-Object { $_.Current.ClassName -eq 'MenuFlyout' } | Select-Object -First 1
  if (-not $flyout) { throw 'the view flyout did not open' }
  $item = Find-UiaElement -AutomationId $AutomationId -Type MenuItem -Root $flyout
  if (-not $item) { throw "the view menu offers no '$AutomationId'" }
  Invoke-UiaElement $item -SettleMs 1500

  # And bring the page back to now. Picking a shape from the menu re-seats the grid on the period it
  # was already showing, which under -Filter '*' is whatever the suite before this one left it on,
  # the suites share one app launch, and a suite that paged away leaves this one reading an empty
  # month. Today fixes WHICH week; it does not fix the week's alignment, which is why the event
  # above is the daily one.
  $today = Find-UiaElement -AutomationId 'CalendarToday' -Type Button
  if ($today) { Invoke-UiaElement $today -SettleMs 1200 }
}

<#
.SYNOPSIS
Open the detail of a drawn grid block by clicking it through the OS. Returns the dialog.
.DESCRIPTION
The peer is found by its spoken name, which begins with the event's title. Its rectangle is already
in screen pixels, so nothing here converts coordinates, doing that a second time is how a click
lands on the wrong block at any display scale but 100%.
#>
function Open-GridEvent {
  param([Parameter(Mandatory)] [string] $Title)

  Set-CalendarShape 'CalendarViewWeek'
  $app = Get-Process Mailcal -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $app) { throw 'the app is not running' }
  [SeriesScopeInput]::SetForegroundWindow($app.MainWindowHandle) | Out-Null
  Start-Sleep -Milliseconds 500

  $peer = Show-GridBlock -Title $Title
  $r = $peer.Current.BoundingRectangle
  [SeriesScopeInput]::SetCursorPos([int]($r.X + ($r.Width / 2)), [int]($r.Y + ($r.Height / 2))) | Out-Null
  Start-Sleep -Milliseconds 200
  [SeriesScopeInput]::mouse_event([SeriesScopeInput]::LEFTDOWN, 0, 0, 0, [UIntPtr]::Zero)
  [SeriesScopeInput]::mouse_event([SeriesScopeInput]::LEFTUP, 0, 0, 0, [UIntPtr]::Zero)
  Start-Sleep -Milliseconds 900
  $dialog = Get-DialogRoot
  # The click is aimed by geometry, so what it opened is checked rather than assumed: a detail for
  # the wrong event would otherwise be read as an answer about this one.
  if (-not (Get-UiaTree $dialog | Where-Object { $_.Current.Name -like "$Title*" })) {
    throw "the click at the block for '$Title' opened somebody else's detail"
  }
  return $dialog
}

<#
.SYNOPSIS
The drawn block whose spoken name begins with -Title, once the grid has actually painted it.
.DESCRIPTION
A poll, not a look. Choosing a shape from the view menu re-seats the grid, and its blocks arrive on
a later frame than the surface they sit on, so the first read after a switch finds the day headers
and none of the events, and reports it as "the seeded week is not on screen". That reads as a
harness or a seeding problem and is neither.

Returning $null rather than throwing is deliberate: the cancellation case asks the opposite
question, and "still drawn?" must not be answered by an exception.
#>
function Wait-GridBlock {
  param([Parameter(Mandatory)] [string] $Title, [int] $TimeoutSec = 20)
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($watch.Elapsed.TotalSeconds -lt $TimeoutSec) {
    $peer = Get-UiaTree |
      Where-Object { $_.Current.ClassName -eq 'CalendarItem' -and $_.Current.Name -like "$Title*" } |
      Select-Object -First 1
    if ($peer) { return $peer }
    Start-Sleep -Milliseconds 500
  }
  return $null
}

<#
.SYNOPSIS
The drawn block for -Title, wheeled into the timed area so a click on it lands on it.
.DESCRIPTION
The grid seats itself near the current time, so a fixed-time block is in view or above it depending
on the hour the suite runs, and a rectangle above the viewport has a negative y, which a click
would follow into the window chrome. Wheeling until the block's middle sits in the lower part of
the grid clears the day headers and the all-day band in one rule, and is what makes this case
independent of the clock.

The wheel goes to whatever is under the pointer, so the pointer is parked on the grid first.
#>
function Show-GridBlock {
  param([Parameter(Mandatory)] [string] $Title)

  $grid = Find-UiaElement -AutomationId 'CalendarGrid'
  if (-not $grid) { throw 'the calendar grid is not on screen' }
  $g = $grid.Current.BoundingRectangle
  [SeriesScopeInput]::SetCursorPos([int]($g.X + ($g.Width / 2)), [int]($g.Y + ($g.Height / 2))) | Out-Null
  Start-Sleep -Milliseconds 200

  $floor = $g.Y + ($g.Height * 0.35)
  $ceiling = $g.Y + ($g.Height * 0.85)
  for ($i = 0; $i -lt 30; $i++) {
    $peer = Wait-GridBlock -Title $Title
    if (-not $peer) {
      # Say which week is on screen and what is drawn on it. "Not found" alone is read as a
      # seeding or harness problem, and under -Filter '*' it is neither: these suites share one
      # app launch, so the answer is almost always the page the suite before this one left.
      $period = (Find-UiaElement -AutomationId 'CalendarPeriod').Current.Name
      $drawn = (Get-UiaTree |
        Where-Object { $_.Current.ClassName -eq 'CalendarItem' } |
        ForEach-Object { ($_.Current.Name -split ',')[0] }) -join ' | '
      throw "no drawn block for '$Title'. The grid is showing '$period' and holds: $drawn"
    }
    $r = $peer.Current.BoundingRectangle
    $middle = $r.Y + ($r.Height / 2)
    if ($middle -ge $floor -and $middle -le $ceiling) { return $peer }
    $notch = if ($middle -lt $floor) { 120 } else { -120 }
    [SeriesScopeInput]::mouse_event([SeriesScopeInput]::WHEEL, 0, 0, $notch, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 250
  }
  throw "the block for '$Title' would not scroll into the timed area"
}

<#
.SYNOPSIS
Switch to the agenda and open a row's detail. Returns the dialog.
.DESCRIPTION
The agenda is a real ListView, so this one is an ordinary UIA invoke. It is the control for the
grid cases: an agenda row IS the series, so nothing opened from here may ask about scope.
#>
function Open-AgendaEvent {
  param([Parameter(Mandatory)] [string] $Title)

  Set-CalendarShape 'CalendarViewAgenda'
  $agenda = Find-UiaElement -AutomationId 'CalendarAgenda'
  if (-not $agenda) { throw 'the agenda list did not come up' }
  $row = Find-UiaElements -Type 'ListItem' -Root $agenda |
    Where-Object { (Get-UiaTree $_ | Where-Object { $_.Current.Name -like "$Title*" }) } |
    Select-Object -First 1
  if (-not $row) { throw "no agenda row for '$Title'" }
  Invoke-UiaElement $row -SettleMs 900
  return Get-DialogRoot
}

<#
.SYNOPSIS
The open dialog, identified by the CloseButton every ContentDialog carries.
.DESCRIPTION
Not "the first Popup": the shell keeps a second, empty Popup that sorts ahead of it, and reading
through that one finds nothing, which reads as "the dialog never opened" rather than "you are
looking at the wrong element".
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
Press the detail dialog's Delete (the Secondary button) and return whatever dialog replaces it.
#>
function Invoke-Delete {
  param([Parameter(Mandatory)] [object] $Dialog)
  $button = Find-UiaElement -AutomationId 'SecondaryButton' -Type Button -Root $Dialog
  if (-not $button) { throw 'the detail dialog offers no Delete' }
  Invoke-UiaElement $button -SettleMs 1200
  return Get-DialogRoot
}

<#
.SYNOPSIS
Press the detail dialog's Edit (the Primary button) and return the editor.
#>
function Invoke-Edit {
  param([Parameter(Mandatory)] [object] $Dialog)
  $button = Find-UiaElement -AutomationId 'PrimaryButton' -Type Button -Root $Dialog
  if (-not $button) { throw 'the detail dialog offers no Edit' }
  Invoke-UiaElement $button -SettleMs 1200
  $editor = Get-DialogRoot
  if (-not (Find-UiaElement -AutomationId 'EventTitleBox' -Root $editor)) {
    throw 'Edit did not open the event editor'
  }
  return $editor
}

<#
.SYNOPSIS
Press the editor's Save (the Primary button) and return whatever dialog replaces it.
#>
function Invoke-Save {
  param([Parameter(Mandatory)] [object] $Dialog)
  $button = Find-UiaElement -AutomationId 'PrimaryButton' -Type Button -Root $Dialog
  if (-not $button) { throw 'the editor offers no Save' }
  Invoke-UiaElement $button -SettleMs 1200
  return Get-DialogRoot
}

<#
.SYNOPSIS
How many of the three ContentDialog answer buttons this dialog actually offers.
.DESCRIPTION
The language-free way to tell the two questions apart. A scope question offers three (this event /
all events / cancel); the generic delete confirmation offers two.
#>
function Get-AnswerCount {
  param([Parameter(Mandatory)] [object] $Dialog)
  $ids = @('PrimaryButton', 'SecondaryButton', 'CloseButton')
  @($ids | Where-Object {
    $b = Find-UiaElement -AutomationId $_ -Type Button -Root $Dialog
    $b -and $b.Current.IsOffscreen -eq $false
  }).Count
}

<#
.SYNOPSIS
Dismiss every open dialog, so the next case starts from a clean screen.
#>
function Close-Dialog {
  for ($i = 0; $i -lt 4; $i++) {
    $button = Find-UiaElement -AutomationId 'CloseButton' -Type Button
    if (-not $button) { return }
    Invoke-UiaElement $button -SettleMs 800
  }
}

$Suite = @{
  Dataset = 'harness'
  Cases   = @(
    @{
      Name = 'deleting one occurrence from the grid asks which occurrences it meant'
      Body = {
        $detail = Open-GridEvent -Title $Repeating
        try {
          $question = Invoke-Delete -Dialog $detail
          Assert-Equal 3 (Get-AnswerCount -Dialog $question) (
            'a delete opened from the grid on a repeating event must offer three answers, this ' +
            'event, all events, cancel. Two means the generic confirmation was raised instead, ' +
            'and answering it removes the whole series.')
        } finally { Close-Dialog }
      }
    },
    @{
      Name = 'the same delete from the agenda asks nothing about scope'
      Body = {
        # The control, and the reason the case above proves anything: an agenda row lists the
        # series, so there is no single day to name and the ordinary confirmation is correct. A
        # client that asked here would be offering a choice it cannot honour.
        $detail = Open-AgendaEvent -Title $Repeating
        try {
          $question = Invoke-Delete -Dialog $detail
          Assert-Equal 2 (Get-AnswerCount -Dialog $question) (
            'an agenda row IS the series, there is no occurrence to name, so the scope question ' +
            'must not be put.')
        } finally { Close-Dialog }
      }
    },
    @{
      Name = 'saving an edit to one occurrence asks which occurrences it meant'
      Body = {
        # The delete has asked since the scope question shipped; the edit could not, because the
        # form was seeded from the SERIES' times and This event against those would have moved the
        # occurrence onto the series' date. It is the same question from the same detail, so a
        # client that wired only the delete passes every case above and fails this one.
        $detail = Open-GridEvent -Title $Repeating
        try {
          $editor = Invoke-Edit -Dialog $detail
          $question = Invoke-Save -Dialog $editor
          Assert-Equal 3 (Get-AnswerCount -Dialog $question) (
            'a save from the grid on a repeating event must offer three answers, this event, ' +
            'all events, cancel. Two means the whole series was about to be rewritten to change ' +
            'one Tuesday.')
        } finally { Close-Dialog }
      }
    },
    @{
      Name = 'backing out of the scope question keeps what was typed'
      Body = {
        # The Windows-only half of the contract. WinUI permits one ContentDialog at a time, so the
        # editor is already CLOSED by the time the question is put, and the only way back is to
        # reopen it, over the same state, or cancel and discard everything the user typed are the
        # same button. Nothing here writes: the editor is cancelled at the end.
        $detail = Open-GridEvent -Title $Repeating
        try {
          $editor = Invoke-Edit -Dialog $detail
          $box = Find-UiaElement -AutomationId 'EventTitleBox' -Root $editor
          if (-not $box) { throw 'the editor has no title field' }
          $typed = "$Repeating (edited)"
          Set-UiaText -Element $box -Text $typed

          $question = Invoke-Save -Dialog $editor
          $cancel = Find-UiaElement -AutomationId 'CloseButton' -Type Button -Root $question
          if (-not $cancel) { throw 'the scope question offers no way out' }
          Invoke-UiaElement $cancel -SettleMs 1200

          $reopened = Get-DialogRoot
          $again = Find-UiaElement -AutomationId 'EventTitleBox' -Root $reopened
          Assert-True ([bool]$again) (
            'backing out of the scope question left no editor on screen. Cancelling a question ' +
            'about a save must not also discard the save.')
          Assert-Equal $typed (Get-UiaText -Element $again) (
            'the reopened editor no longer holds what was typed. On this platform the question ' +
            'cannot be raised OVER the editor, so the editor is reopened, and reopening it on ' +
            'anything but the same state makes cancel and discard the same button.')
        } finally { Close-Dialog }
      }
    },
    @{
      Name = 'backing out of the scope question deletes nothing'
      Body = {
        # The question carries its own way out, and taking it must leave the calendar exactly as
        # it was. The oracle is the block still being drawn afterwards.
        $detail = Open-GridEvent -Title $Repeating
        $question = Invoke-Delete -Dialog $detail
        $cancel = Find-UiaElement -AutomationId 'CloseButton' -Type Button -Root $question
        Invoke-UiaElement $cancel -SettleMs 1200
        Close-Dialog

        Set-CalendarShape 'CalendarViewWeek'
        $still = Wait-GridBlock -Title $Repeating
        Assert-True ([bool]$still) (
          "'$Repeating' is no longer drawn after cancelling the scope question. Backing out of a " +
          'destructive question must write nothing at all.')
      }
    }
  )
}
