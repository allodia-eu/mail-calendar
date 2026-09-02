# What an event's Repeat row actually says on the detail, and what its controls open on in the
# editor (docs/calendar.md §10, "Which sentence a repeat rule gets is decided once; the words are
# each client's", and "A rule is editable exactly when it can be stated").
#
# WHY THIS SUITE EXISTS. The decisions are pinned in Mailcal.Tests (EventRepeatFormatTests): which
# frame a rule reaches, and the weekday and month names that go in it. What that assembly cannot
# reach is the other half, mapping a frame to its catalog string, because L10n.cs needs a Windows
# TFM it has not got. So a frame wired to the wrong string (the every-period one where the
# every-N-periods one belongs, say) compiles, passes `dotnet test`, and puts a plain untruth on
# screen. This suite is the only machine that looks at the sentence.
#
# The editor's controls are the same argument twice over. EventRepeatEditorTests pins what a save
# SENDS, but a control seeded from the wrong end of the rule sends exactly what it was seeded with,
# so no assertion there can see it: a fortnightly series whose interval box opens on 1 is a form
# proposing to make it weekly, and both halves are WinUI, which Mailcal.Tests cannot link.
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
#     what the old one-word summary could not do;
#   * for the editor's CONTROLS, those three plus what is on screen at all. Which controls a
#     frequency draws, how many weekday buttons are ticked, and the digits a spinner opens on are
#     all readable without knowing a word of the language the app is in.
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

<#
.SYNOPSIS
Open the EDITOR over an agenda event, and return the editor's dialog.
.DESCRIPTION
Two dialogs deep: the agenda opens the detail, and the detail's primary button opens the editor over
it. Get-DialogRoot then finds the editor rather than the detail because a ContentDialog opened over
another sorts ahead of it, which is also why every case here closes what it opened.
#>
function Open-EventEditor {
  param([Parameter(Mandatory)] [string] $Title)
  $null = Open-EventDetail $Title
  $edit = Find-UiaElement -AutomationId 'PrimaryButton' -Type Button
  if (-not $edit) { throw "'$Title' offered no Edit button, the harness calendar should be writable" }
  Invoke-UiaElement $edit -SettleMs 1500
  return Get-DialogRoot
}

<#
.SYNOPSIS
The text a repeat picker is currently showing, for the seeded event $Title.
.DESCRIPTION
Read off the SELECTION, never off the ComboBox's own Name: WinUI puts the control's HEADER there
("Repeat", "Ends"), so a picker opened on the wrong choice would still answer "Repeat" and every
comparison between two events would pass. Leaves the dialog closed.
#>
function Get-SelectedChoice {
  param(
    [Parameter(Mandatory)] [string] $Title,
    [Parameter(Mandatory)] [string] $AutomationId
  )
  $dialog = Open-EventEditor $Title
  $combo = Find-UiaElement -AutomationId $AutomationId -Root $dialog
  if (-not $combo) { throw "'$Title' shows no $AutomationId control" }
  $pattern = $combo.GetCurrentPattern([System.Windows.Automation.SelectionPattern]::Pattern)
  $selected = @($pattern.Current.GetSelection())
  if ($selected.Count -ne 1) {
    throw "$AutomationId on '$Title' has $($selected.Count) selected items, a picker showing nothing states no rule at all"
  }
  $text = $selected[0].Current.Name
  Close-Dialog
  if (-not $text) { throw "$AutomationId on '$Title' shows an empty choice, which says nothing to the reader" }
  return $text
}

<#
.SYNOPSIS
The NUMBER a repeat spinner is showing, for the seeded event $Title.
.DESCRIPTION
A NumberBox is a Spinner wrapping an Edit, and only the Edit carries ValuePattern. The Spinner's own
Name is its localised header ("Every 2 weeks"), so reading the number off the wrapper would be
reading the catalog. Returns the value as its string, digits being language-free.
#>
function Get-SpinnerValue {
  param(
    [Parameter(Mandatory)] [string] $Title,
    [Parameter(Mandatory)] [string] $AutomationId
  )
  $dialog = Open-EventEditor $Title
  $spinner = Find-UiaElement -AutomationId $AutomationId -Root $dialog
  if (-not $spinner) { throw "'$Title' shows no $AutomationId control" }
  $box = Find-UiaElement -Type Edit -Root $spinner
  if (-not $box) { throw "$AutomationId on '$Title' has no editable field, so its number cannot be read or typed" }
  $value = Get-UiaText $box
  Close-Dialog
  return $value
}

<#
.SYNOPSIS
How many weekday buttons are ticked in the editor for the seeded event $Title.
.DESCRIPTION
A count rather than a set of names, because the names are localised: what is assertable in any
language is HOW MANY days a rule opens with, which is what separates BYDAY=MO,WE from the implicit
single day, and what catches a row that opens with none.
#>
function Get-CheckedWeekdays {
  param([Parameter(Mandatory)] [string] $Title)
  $dialog = Open-EventEditor $Title
  $days = @(Find-UiaElements -AutomationId 'EventRepeatWeekday' -Root $dialog)
  if ($days.Count -eq 0) { throw "'$Title' is a weekly rule and shows no weekday row" }
  $checked = @($days | Where-Object { Get-UiaToggle $_ }).Count
  Close-Dialog
  return $checked
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
      Name = 'the editor opens the rule as controls, not as the sentence the detail shows'
      Body = {
        # The detail READS a rule and the editor SETS one, so the editor is the one surface that
        # must not settle for the sentence: a reader who can only be told "every 2 weeks" has no
        # way to make it every third. EventRepeatValue is the detail's row, and its ABSENCE here is
        # the assertion.
        $dialog = Open-EventEditor $Fortnightly
        if (-not (Find-UiaElement -AutomationId 'EventRepeatFrequency' -Root $dialog)) {
          throw 'the editor offers no frequency control, so the rule cannot be changed at all'
        }
        Assert-True (-not (Find-UiaElement -AutomationId 'EventRepeatValue' -Type Text -Root $dialog)) `
          'the editor states the rule as a read-only sentence, which is the surface it replaced'
        Close-Dialog
      }
    },
    @{
      Name = 'the frequency opens on the rule the event has, not on the first choice'
      Body = {
        # The failure this catches is a picker that opens at index 0 ("Does not repeat") whatever
        # the event does: every control below it is then seeded from nothing, and a save stops the
        # series the user came to adjust. Language-free by comparing the three to EACH OTHER, the
        # handle the sentence cases already use.
        $weekly = Get-SelectedChoice $Fortnightly 'EventRepeatFrequency'
        $monthly = Get-SelectedChoice $MonthlyNth 'EventRepeatFrequency'
        $none = Get-SelectedChoice $Plain 'EventRepeatFrequency'
        Assert-True ($weekly -ne $monthly) `
          "a weekly and a monthly rule cannot open on the same frequency (both say '$weekly')"
        Assert-True ($weekly -ne $none) `
          "a repeating event cannot open on the choice an event with no rule opens on (both say '$none')"
        Assert-True ($monthly -ne $none) `
          "a monthly rule cannot open on the choice an event with no rule opens on (both say '$none')"
      }
    },
    @{
      Name = 'an event with no rule offers the frequency alone, and nothing to configure'
      Body = {
        # Every control below the picker describes a rule, so on an event that has none they would
        # be describing one the user never asked for. The panel drops them; this says it did.
        $dialog = Open-EventEditor $Plain
        Assert-True (-not (Find-UiaElement -AutomationId 'EventRepeatInterval' -Root $dialog)) `
          'an event that does not repeat offers an interval, setting a period on a rule that does not exist'
        Assert-True (-not (Find-UiaElement -AutomationId 'EventRepeatEnds' -Root $dialog)) `
          'an event that does not repeat offers an end condition, ending a rule that does not exist'
        Close-Dialog
      }
    },
    @{
      Name = 'the interval box opens on the number of periods the rule skips'
      Body = {
        # THE bug of this whole surface, moved from the sentence to the control: a fortnightly rule
        # seeded into a box reading 1 is a form quietly proposing to make it weekly, and the user
        # finds out after saving. Digits are written the same way in all seven catalog languages.
        Assert-Equal '2' (Get-SpinnerValue $Fortnightly 'EventRepeatInterval') `
          'the fortnightly rule is seeded INTERVAL=2, so its interval box opens on 2'
        Assert-Equal '1' (Get-SpinnerValue $Weekly 'EventRepeatInterval') `
          'a rule with no INTERVAL repeats every period, so its box opens on 1'
      }
    },
    @{
      Name = 'a rule that stops after a fixed number opens on that number'
      Body = {
        # The end is part of the rule, and a count seeded as anything else rewrites how long the
        # series runs. The count box exists only while "Ends" is on the after-N choice, so its
        # presence is half of what is asserted here.
        $dialog = Open-EventEditor $Counted
        if (-not (Find-UiaElement -AutomationId 'EventRepeatEndCount' -Root $dialog)) {
          throw 'a rule seeded COUNT=10 opens with no count control, so its end is not on screen at all'
        }
        Close-Dialog
        Assert-Equal '10' (Get-SpinnerValue $Counted 'EventRepeatEndCount') `
          'a rule seeded COUNT=10 opens on 10, not on a default the control invented'
      }
    },
    @{
      Name = 'the weekday row belongs to a weekly rule and to no other'
      Body = {
        # A day of the week means nothing in a monthly rule, and the panel rebuilds per frequency to
        # say so. Found by id rather than by name: the day names are localised and the order is the
        # locale's, so neither one identifies the row.
        $weeklyDialog = Open-EventEditor $Weekly
        $days = @(Find-UiaElements -AutomationId 'EventRepeatWeekday' -Root $weeklyDialog)
        Assert-Equal 7 $days.Count 'a weekly rule offers the whole week, one button per day'
        Close-Dialog

        $monthlyDialog = Open-EventEditor $MonthlyNth
        $absent = @(Find-UiaElements -AutomationId 'EventRepeatWeekday' -Root $monthlyDialog)
        Assert-Equal 0 $absent.Count `
          'a monthly rule draws the weekday row, offering to set a day its frequency cannot use'
        Close-Dialog
      }
    },
    @{
      Name = 'the weekday row opens with the rule''s own days ticked, and never with none'
      Body = {
        # Two silent failures at once. A row ticked from the wrong end of the week (DayOfWeek counts
        # Sunday as 0, the core counts from Monday) still draws a plausible row, so the COUNT of
        # ticked days is what separates BYDAY=MO,WE from a single day. And a weekly rule naming no
        # day is one the core refuses, which reads in the app as a save that simply did nothing.
        Assert-Equal 2 (Get-CheckedWeekdays $Weekly) `
          'the rule seeded BYDAY=MO,WE opens with exactly those two days ticked'
        Assert-Equal 1 (Get-CheckedWeekdays $Fortnightly) `
          "a weekly rule naming no day takes the event's own weekday, so exactly one day is ticked"
      }
    }
  )
}
