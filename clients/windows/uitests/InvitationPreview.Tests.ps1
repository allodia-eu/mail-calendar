# "Your calendar that day" opens EXPANDED whenever the calendar was actually read, on every
# platform, and whatever the conflict count is (docs/invitations.md, "The preview opens expanded").
#
# WHY THIS NEEDS THE HARNESS, AND WHY IT NEEDS TWO INVITATIONS. The rule it replaced was "open only
# when the count is non-zero", so a test that can only see a CONFLICTED day passes under both rules
# and proves nothing. Telling them apart needs a day with no overlap at all, which is why
# docker/stalwart/seed.sh seeds a second invitation on the weekend the living week leaves empty.
# Both cases are here on purpose: the free day is the one that can fail, and the conflicted day is
# what stops a future "open only when it is empty" from looking correct.
#
# NOTHING BELOW MATCHES A LOCALISED STRING. The app's language on a harness run comes from the
# developer's own preference, so asserting on "Nothing else in your calendar then" would make this
# suite pass or fail by whose machine it ran on. Two language-free handles do the work instead:
# the Expander is found by ClassName, and the day's contents are read as EVENT TITLES, which come
# from the seed rather than the catalog. Hour labels are filtered by shape, so a 12-hour clock
# (docs/timestamps.md) does not read as an event called "9 AM".

# THE PREVIEW IS WITHHELD UNTIL THE CALENDAR HAS ACTUALLY BEEN READ, and that is correct, not a
# bug: a count of zero and "we have not looked" are different facts, and the core says the second
# one until the diary covers the meeting's day (`conflicts_for`, docs/calendar.md §4). A store that
# has never synced a calendar therefore shows every invitation as unknown, so this suite has a
# precondition it must establish rather than assume, see Prepare below. It went unnoticed for as
# long as it did because a developer's store had usually been synced by something earlier.

$FreeDayInvite = 'Weekend walk'          # seeded on the empty weekend day, 11:00–12:00
$FreeDayEndHour = 12
$ConflictedInvite = 'Quarterly planning' # Monday, inside the review/triage overlap

# control.ps1 relative to this file, captured at load: $PSScriptRoot inside a scriptblock binds to
# whoever invokes it, which is the runner.
$ControlScript = Join-Path $PSScriptRoot '../control.ps1'
# The shared log root, not the per-dev-mode store, app.log deliberately diagnoses whatever ran
# last on this machine (docs/logging.md), which is what SessionLog.Tests reads too.
$AppLog = Join-Path $env:LOCALAPPDATA 'Allodia\MailCalendar\logs\app.log'

<#
.SYNOPSIS
Block until the running app has read its calendars into the store.
.DESCRIPTION
The grid is on screen well before the sync it starts has landed, and what the NEXT launch primes
from is the store, so waiting on the window proves nothing. The core logs the rebuild when it
finishes; that line, in the current session, is the signal.

Only lines after the newest session marker count. app.log is shared across runs, so a rebuild from
an earlier launch would otherwise be read as this one's and the relaunch would race the sync, the
failure being a preview that is correctly withheld, i.e. exactly the symptom this suite exists to
tell apart from a broken card.
#>
function Wait-CalendarRead {
  param([int] $TimeoutSec = 90)
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($watch.Elapsed.TotalSeconds -lt $TimeoutSec) {
    if (Test-Path -LiteralPath $AppLog) {
      $lines = @(Get-Content -LiteralPath $AppLog)
      $start = 0
      for ($i = $lines.Count - 1; $i -ge 0; $i--) {
        if ($lines[$i] -match ' --- session start \(') { $start = $i; break }
      }
      for ($i = $start; $i -lt $lines.Count; $i++) {
        if ($lines[$i] -match 'rebuild_calendar_cache: (\d+) occurrence') { return }
      }
    }
    Start-Sleep -Milliseconds 500
  }
  throw "the calendar was never read into the store (no rebuild_calendar_cache in this session's app.log within ${TimeoutSec}s), is the harness up, and does its account have a calendar?"
}

<#
.SYNOPSIS
Open a message and return its invitation card's preview Expander.
#>
function Get-PreviewExpander {
  param([Parameter(Mandatory)] [string] $Title)
  Invoke-UiaElement (Get-MailRowByTitle $Title)
  # The card is built on Apply, so poll for the Expander rather than sleeping a guessed interval.
  $cond = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ClassNameProperty, 'Microsoft.UI.Xaml.Controls.Expander')
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($watch.Elapsed.TotalSeconds -lt 30) {
    $found = @((Get-MailcalWindow).FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond))
    if ($found.Count -eq 1) { return $found[0] }
    if ($found.Count -gt 1) { throw "expected one Expander in the reading pane, found $($found.Count), scope this test before trusting it" }
    Start-Sleep -Milliseconds 300
  }
  throw "'$Title' showed no invitation preview. Three things produce that, in the order worth checking. (1) The client's harness store is stale: a re-bootstrapped Stalwart hands out its old ids for a new set of messages, so every cached body belongs to a different message and this one carries no calendar part at all. Read it with 'scripts/dev/store.sh sql ... --store dev' joining message to message_body, and clear it with 'scripts/dev/harness.sh reset'. (2) The calendar was never read, so the preview is correctly withheld and the card says so, Prepare above is what establishes that, so check it ran. (3) The card itself was not built, which is the RSVP gate (docs/invitations.md)."
}

<#
.SYNOPSIS
Split the Expander's Text nodes into hour labels (hour -> the y they are drawn at) and event blocks.
.DESCRIPTION
Both halves are read by SHAPE, not by string, so the suite survives a language change and a 12-hour
clock (docs/timestamps.md): an hour label is a bare number with an optional :mm and an optional
am/pm, and everything else is an event title, which comes from the seed.
#>
function Get-PreviewContents {
  param([Parameter(Mandatory)] [object] $Expander)
  $header = $Expander.Current.Name
  $hours = @{}
  $events = @()
  foreach ($node in Get-UiaTree $Expander) {
    if ($node.Current.ControlType -ne [System.Windows.Automation.ControlType]::Text) { continue }
    $name = $node.Current.Name
    if (-not $name -or $name -eq $header) { continue }
    if ($name -match '^\s*(\d{1,2})(?::(\d{2}))?\s*(?:([AaPp])\.?[Mm]\.?)?\s*$') {
      $hour = [int] $Matches[1]
      if ($Matches[3] -and $Matches[3] -in @('p', 'P') -and $hour -lt 12) { $hour += 12 }
      if ($Matches[3] -and $Matches[3] -in @('a', 'A') -and $hour -eq 12) { $hour = 0 }
      $hours[$hour] = [double] $node.Current.BoundingRectangle.Top
    }
    else {
      $events += [pscustomobject]@{ Title = $name; Top = [double] $node.Current.BoundingRectangle.Top }
    }
  }
  [pscustomobject]@{ Hours = $hours; Events = $events }
}

<#
.SYNOPSIS
The drawn events that could overlap a meeting ending at -EndHour, i.e. the ones that would make the
invitation's conflict count non-zero.
.DESCRIPTION
Why not simply require an EMPTY day: a freshly seeded weekend is empty, but a developer who scratched
an event onto Saturday while testing the event editor would then get a red suite over something that
does not conflict with an 11:00 meeting at all. That is a false failure, and false failures are how a
suite stops being read.

The filter is sound in the one direction that matters. An event's TITLE is drawn at the top of its
block, so its y gives the event's START, not its end. An event starting at or after the meeting ends
therefore cannot overlap it, and is dropped. Anything starting earlier is KEPT even though it might
also end before the meeting begins: over-reporting leaves the guard able to fail, and under-reporting
would let the case quietly stop discriminating, which is the whole thing this guard is for.
#>
function Get-PreviewConflicts {
  param(
    [Parameter(Mandatory)] [object] $Contents,
    [Parameter(Mandatory)] [int] $EndHour,
    [Parameter(Mandatory)] [string] $Meeting
  )
  # The meeting's OWN block is drawn here, deliberately: the core keeps the invitation's tentative
  # hold in the preview "so the user can see where the meeting would land among their commitments",
  # and excludes it from the count by UID. So the picture legitimately holds one block this guard
  # must not read as a clash, it asks whether anything ELSE overlaps.
  #
  # Leaving it in did not merely over-report: it made the guard depend on whether the meeting had
  # been answered. A DECLINED event is hidden from every calendar surface, so once
  # InvitationReplyPrompt.Tests had declined this meeting the block vanished and the guard read
  # zero; on a freshly seeded harness, where it is still unanswered, the same guard read one. The
  # suite passed or failed by what had run against the server before it.
  $Contents = [pscustomobject]@{
    Hours  = $Contents.Hours
    Events = @($Contents.Events | Where-Object { $_.Title -ne $Meeting })
  }
  $cutoff = $Contents.Hours[$EndHour]
  # No label for the meeting's end hour (the grid is scrolled past it, or the band is off-screen):
  # fall back to counting every drawn event rather than silently passing on a comparison we cannot
  # make.
  if ($null -eq $cutoff) { return $Contents.Events }
  # A label is drawn at the top of its own band, and so is an event block's title, so the two are on
  # the same scale: at-or-below the end-hour label means the event starts at or after the meeting
  # ends. The 1px slack absorbs rounding between the label's baseline box and the block's.
  $Contents.Events | Where-Object { $_.Top -lt ($cutoff - 1) }
}

$Suite = @{
  Dataset = 'harness'
  # Make the calendar read happen, so "we have not looked" is not the honest answer to every case
  # below. Two relaunches rather than a click, because the only handles for the calendar and the
  # mailbox in the pane are their LABELS, which are localised; control.ps1's verbs are not.
  #
  # It has to be a relaunch back rather than a return trip in one session: the mail list is
  # collapsed while the grid is up, so it is not in the automation tree to navigate with. The
  # second launch is also what proves the read stuck, the boot path rebuilds the grid cache from
  # the store (`prime_calendar`), and it only does that once the store has a calendar in it.
  Prepare = {
    & $ControlScript calendar | Out-Null
    if (-not (Wait-UiaElement -AutomationId 'CalendarPeriod' -TimeoutSec 60)) {
      throw 'the calendar surface never came up, so its diary was never read'
    }
    # The grid appears before the sync it kicks off has landed; the store is what the next launch
    # primes from, so wait for the core to say it rebuilt the cache rather than guessing an interval.
    Wait-CalendarRead
    & $ControlScript home | Out-Null
    if (-not (Wait-UiaElement -AutomationId 'RowsList' -TimeoutSec 60)) {
      throw 'the message list never came back after warming the calendar'
    }
  }
  Cases   = @(
    @{
      Name = 'the preview opens expanded on a day with nothing else on it'
      Body = {
        # THE DISCRIMINATING CASE. Under the old rule this expander was Collapsed: the count is
        # zero, so there was "nothing to see". The point of the change is that a drawn, visibly
        # empty day answers "what does my day look like" better than the sentence above it does.
        $expander = Get-PreviewExpander $FreeDayInvite
        $state = $expander.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Current.ExpandCollapseState
        Assert-Equal 'Expanded' "$state" `
          'a free day is exactly when the picture settles it fastest, so the grid opens without being asked'

        # FIXTURE GUARD, not a product assertion. The case above only tells the new rule from the
        # old one while this day's conflict count is zero; if something now overlaps the meeting,
        # the old rule would have opened the preview too and the check silently stops proving
        # anything. Failing here says "the fixture drifted", not "the app regressed".
        $clashing = @(Get-PreviewConflicts -Contents (Get-PreviewContents $expander) `
            -EndHour $FreeDayEndHour -Meeting $FreeDayInvite)
        Assert-Equal 0 $clashing.Count `
          "nothing may overlap the seeded weekend meeting, or this case stops telling the new rule from the old one, found: $($clashing.Title -join ', '). That is a harness-fixture problem, not a regression: clear it with 'scripts/dev/harness.sh reset'"
      }
    },
    @{
      Name = 'the preview opens expanded on a day that has clashes too'
      Body = {
        $expander = Get-PreviewExpander $ConflictedInvite
        $state = $expander.GetCurrentPattern([System.Windows.Automation.ExpandCollapsePattern]::Pattern).Current.ExpandCollapseState
        Assert-Equal 'Expanded' "$state" `
          'the conflicted day opens too, one rule, not a pair of opposite special cases'

        # Guards the fixture rather than the app: if the Monday invitation ever stopped landing in
        # the review/triage overlap, the case above and this one would be the SAME test, and the
        # suite would quietly stop covering half the rule.
        Assert-GreaterThan 0 (@((Get-PreviewContents $expander).Events)).Count `
          'the Monday invitation is seeded to overlap the living week, so its preview must draw something'
      }
    }
  )
}
