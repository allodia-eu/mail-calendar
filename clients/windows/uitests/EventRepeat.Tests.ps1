# What an event's Repeat row actually says (docs/calendar.md §10, "Which sentence a repeat rule
# gets is decided once; the words are each client's").
#
# WHY THIS SUITE EXISTS. The decisions are pinned in Mailcal.Tests (EventRepeatFormatTests): which
# frame a rule reaches, and the weekday and month names that go in it. What that assembly cannot
# reach is the other half, mapping a frame to its catalog string, because L10n.cs needs a Windows
# TFM it has not got. So a frame wired to the wrong string (the every-period one where the
# every-N-periods one belongs, say) compiles, passes `dotnet test`, and puts a plain untruth on
# screen. This suite is the only machine that looks at the sentence.
#
# WHY THE HARNESS. The showcase seed carries no recurrence at all, so there is no repeating event to
# open there. These rules arrive over JMAP from Stalwart, and the core summarises what the server
# actually sent.
#
# NOTHING BELOW MATCHES A LOCALISED STRING. A harness run comes up in the developer's own language,
# so asserting "Every 2 weeks on Tuesday" would make this suite pass or fail by whose machine it ran
# on. Three language-free handles do the work:
#   * the seeded event TITLES, which come from docker/stalwart/seed-calendar-week.sh, not the catalog;
#   * EventRepeatValue, the automation id on the row's VALUE, the label beside it is localised, so
#     an id is the only way to find this row in any language;
#   * the sentences compared to EACH OTHER, and to the DIGITS in them. A number is written the same
#     way in all seven catalog languages, and "these four rules say four different things" is exactly
#     what the old one-word summary could not do.
#
# THE SEEDED RULES (docker/stalwart/seed-calendar-week.sh, the `repeating` block):
#   Team sync        FREQ=WEEKLY;BYDAY=MO,WE     -> names two weekdays
#   Sprint planning  FREQ=WEEKLY;INTERVAL=2      -> the fortnightly case, the reason for all of this
#   Board meeting    FREQ=MONTHLY;BYDAY=1WE      -> a weekday's position in the month
#   Onboarding       FREQ=DAILY;COUNT=10         -> a rule that stops after a fixed number
#   Lunch            (no rule)                   -> the control
#
# NOT COVERED HERE: a rule bounded by a DATE (`UNTIL`). No fixture in the living week has one, so
# the frame that states it is proven by EventRepeatFormatTests and by nothing on screen.

$Weekly = 'Team sync'
$Fortnightly = 'Sprint planning'
$MonthlyNth = 'Board meeting'
$Counted = 'Onboarding'
$Plain = 'Lunch'

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
with no Invoke pattern, so UIA can read one but cannot open it. The agenda is a real ListView whose
ItemClick runs the same OnOpenEvent. The Agenda entry is taken as the LAST item of the view flyout
rather than by its text, so this survives whatever language the app is in.
#>
function Show-Agenda {
  Show-Calendar
  $existing = Find-UiaElement -AutomationId 'CalendarAgenda'
  if ($existing) { return $existing }
  $menu = Find-UiaElement -AutomationId 'CalendarViewMenu' -Type Button
  Invoke-UiaElement $menu -SettleMs 900
  # Scope to the flyout itself: a desktop-wide MenuItem sweep also collects every window's
  # Alt+Space system menu, so the count comes back by what else happens to be open.
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
A ContentDialog hosts in a Popup over the shell, and the agenda list is STILL in the tree behind it.
The dialog is identified by the CloseButton every ContentDialog carries, NOT as "the first Popup":
the shell keeps a second, empty Popup that sorts ahead of it, and reading through that one finds
no repeat row at all, which would read as "the row is gone" rather than "you are looking at the
wrong element".
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
Close every open dialog, so the next case starts from the agenda.
.DESCRIPTION
A loop rather than one press: pressing Edit stacks the editor over the detail, and closing only the
top one leaves a modal still covering the agenda.
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
Every Text node's text under $Root.
#>
function Get-TextsUnder {
  param([Parameter(Mandatory)] [object] $Root)
  foreach ($node in Get-UiaTree $Root) {
    if ($node.Current.ControlType -ne [System.Windows.Automation.ControlType]::Text) { continue }
    if ($node.Current.Name) { $node.Current.Name }
  }
}

<#
.SYNOPSIS
Open an agenda event's detail dialog; returns the DIALOG.
.DESCRIPTION
Rows are matched on the text INSIDE them, never on the row's own Name: an agenda ListViewItem
announces its view-model's type name, because the DataTemplate carries no automation name. That is
a real accessibility defect, filed separately; it is only mentioned here because it is why this
lookup cannot be one line.
#>
function Open-EventDetail {
  param([Parameter(Mandatory)] [string] $Title)
  # A case that throws leaves its dialog up, and a modal swallows the click on the next case's
  # agenda row, so the failure would spread past the case that had it.
  Close-Dialog
  $agenda = Show-Agenda
  $rows = @(Find-UiaElements -Type 'ListItem' -Root $agenda)
  Assert-GreaterThan 0 $rows.Count 'the agenda is empty, the harness calendar never synced'
  $match = $null
  foreach ($row in $rows) {
    if (@(Get-TextsUnder $row) -contains $Title) { $match = $row; break }
  }
  if (-not $match) {
    throw "no agenda row for '$Title', the living week is seeded relative to today, and its repeating events are newer than a long-lived container, so a stale harness is the usual cause: scripts/dev/harness.sh reset"
  }
  Invoke-UiaElement $match -SettleMs 1500
  return Get-DialogRoot
}

<#
.SYNOPSIS
What the Repeat row says for the seeded event $Title, read off its detail dialog.
.DESCRIPTION
Found by automation id, because the caption beside it is localized. Leaves the dialog CLOSED, so
cases can read several events in a row.
#>
function Get-RepeatSentence {
  param([Parameter(Mandatory)] [string] $Title)
  $dialog = Open-EventDetail $Title
  $value = Find-UiaElement -AutomationId 'EventRepeatValue' -Type Text -Root $dialog
  if (-not $value) { throw "'$Title' shows no repeat row at all, every event says how it repeats, including the ones that do not" }
  $text = $value.Current.Name
  Close-Dialog
  if (-not $text) { throw "'$Title' has an empty repeat row, which says nothing to the reader" }
  return $text
}

$Suite = @{
  Dataset = 'harness'
  Cases   = @(
    @{
      Name = 'a fortnightly rule does not read as the weekly one, and says how many weeks it skips'
      Body = {
        # THE bug this surface exists to fix. A frequency word alone cannot tell "every week" from
        # "every second week", so the old summary called the first "Weekly" and gave up on the
        # second with the generic "Repeats".
        $weekly = Get-RepeatSentence $Weekly
        $fortnightly = Get-RepeatSentence $Fortnightly
        Assert-True ($weekly -ne $fortnightly) `
          "a rule that repeats every week and one that skips every other must not read alike (both say '$weekly')"
        Assert-True ($fortnightly -match '2') `
          "a fortnightly rule states its interval; '$fortnightly' does not contain the number of weeks it skips"
      }
    },
    @{
      Name = 'a rule that stops after a fixed number of occurrences says the number'
      Body = {
        # The end is part of the sentence, not dropped from it: the core carries it and the old
        # summary spoke none of it.
        $counted = Get-RepeatSentence $Counted
        Assert-True ($counted -match '10') `
          "a rule seeded COUNT=10 must say how many times it runs; '$counted' does not contain 10"
      }
    },
    @{
      Name = 'a weekly rule on two weekdays names both of them'
      Body = {
        # Language-free because the JOIN is ours, not the catalog's: the weekdays arrive from the
        # core in week order and are joined with ", " whatever the language names them.
        $weekly = Get-RepeatSentence $Weekly
        Assert-True ($weekly -match ',\s') `
          "a rule seeded BYDAY=MO,WE names both weekdays; '$weekly' names at most one"
      }
    },
    @{
      Name = 'four different rules say four different things'
      Body = {
        # The regression guard for the whole feature, and the case the old summary failed: with only
        # a frequency word, the fortnightly and the monthly-by-weekday rules both read "Repeats".
        # It also catches the opposite failure, a frame wired to the wrong catalog string tends to
        # collapse two sentences into one.
        $sentences = @{}
        foreach ($title in $Weekly, $Fortnightly, $MonthlyNth, $Counted) {
          $sentences[$title] = Get-RepeatSentence $title
        }
        $distinct = @($sentences.Values | Sort-Object -Unique)
        Assert-Equal 4 $distinct.Count `
          ("four rules of different shapes must read differently; got: " + (($sentences.GetEnumerator() |
            ForEach-Object { "$($_.Key)='$($_.Value)'" }) -join ', '))
      }
    },
    @{
      Name = 'an event with no rule says it does not repeat, and says it differently'
      Body = {
        # The control. Without it, a change that made every event read the same word would still
        # pass "it has a repeat row", and "Does not repeat" on a repeating event is the worst of
        # the failures available here.
        $plain = Get-RepeatSentence $Plain
        foreach ($title in $Weekly, $Fortnightly, $MonthlyNth, $Counted) {
          $repeating = Get-RepeatSentence $title
          Assert-True ($plain -ne $repeating) `
            "'$title' repeats and '$Plain' does not, so they cannot read alike (both say '$plain')"
        }
      }
    },
    @{
      Name = 'the editor shows the same sentence as the detail'
      Body = {
        # Two surfaces, one function, so a reader who opens the editor is not told a different
        # thing about the same event. They are separate call sites, which is what makes this
        # assertable rather than obvious.
        $detail = Get-RepeatSentence $Fortnightly
        $null = Open-EventDetail $Fortnightly
        $edit = Find-UiaElement -AutomationId 'PrimaryButton' -Type Button
        if (-not $edit) { throw 'the detail offered no Edit button, the harness calendar should be writable' }
        Invoke-UiaElement $edit -SettleMs 1500
        $dialog = Get-DialogRoot
        $value = Find-UiaElement -AutomationId 'EventRepeatValue' -Type Text -Root $dialog
        if (-not $value) { throw 'the editor shows no repeat row, an edit would be made blind to how the event repeats' }
        Assert-Equal $detail $value.Current.Name `
          'the editor and the detail read the same rule, so they say the same sentence'
        Close-Dialog
      }
    }
  )
}
