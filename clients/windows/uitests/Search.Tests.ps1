#!/usr/bin/env pwsh
# What a search puts on screen here, which `docs/search.md` rules 9 and 10 decide for every client
# and this client had no machine watching. The core half of both rules is covered by
# `crates/mailcal-app/src/tests_search/`; what those cannot see is whether this client renders the
# row kind the core sent, and whether the field in front of it asks the core once or once per key.
# Both are invisible to `Mailcal.Tests`: one needs a rendered list, the other needs a real keystroke.
#
# THE FIXTURE is the `en` showcase seed (crates/mailcal-bindings/src/showcase_data/en.rs), pinned by
# the runner to -Locale en, and its one three-message conversation is what makes the cases below
# assertions rather than coincidences. Two queries pick it out from different angles:
#
#   "no-go"     the subject, so all three messages match. A search that ignored the grouping would
#               answer with a row each.
#   "rollback"  a word in the first two messages only. The thread's NEWEST message does not carry
#               it, so the row has to be labelled by Eva's reply at 07:41 rather than by Tom's
#               follow-up at 08:06, which is how the mailbox list labels the same thread.
#
# If a case stops finding its row, that seed changed; check the thread is still three messages, and
# that only the older two say "rollback", before assuming the app broke.

$Thread = [pscustomobject]@{
  # Matches every message in the thread, and nothing else in either account's seed.
  WholeThread = 'no-go'
  # Matches the first two only. Careful: "Q3 launch" also finds Eva's "Q3 board update".
  OlderOnly   = 'rollback'
  Title       = 'Re: Q3 launch*'
  Messages    = 3
  # Who labels the row: the thread's newest message is Tom's, the newest to say "rollback" is Eva's.
  Newest      = 'Tom de Vries'
  NewestMatch = 'Eva Jansen'
}

# A query nothing in either seeded account answers, which is what takes the count to zero.
$NoMatch = 'zzzznothingmatchesthis'

$LogPath = Join-Path $env:LOCALAPPDATA 'Allodia\MailCalendar\logs\app.log'

# `2026-09-12 12:40:39.735 +02:00 [info] [mailcal_app::snapshot] rebuild_snapshot: 1 row(s) of 1 in 5ms`
$RebuildPattern =
  '^(?<ts>\d{4}-\d{2}-\d{2} \d{2}:\d{2}:\d{2}\.\d{3} [+-]\d{2}:\d{2}) .*rebuild_snapshot: '

<#
.SYNOPSIS
The search field, as the AutoSuggestBox's inner Edit.
#>
function Get-SearchBox {
  Find-UiaElement -AutomationId 'TextBox' -Type Edit
}

<#
.SYNOPSIS
Put -Query in the search field and wait for the list to settle on -Expected rows.
.DESCRIPTION
Waits for a count rather than for a fixed delay: a search is asynchronous, and the debounce under
test adds a quarter of a second of its own before the core is even asked. Omit -Expected (clearing
the field) to wait for the list to stop moving instead.
#>
function Set-Search {
  param([Parameter(Mandatory)] [AllowEmptyString()] [string] $Query, [int] $Expected = -1)
  Set-UiaText (Get-SearchBox) $Query
  if ($Expected -lt 0) {
    $null = Wait-MailRowCount
    return @(Get-MailRows)
  }
  for ($i = 0; $i -lt 60; $i++) {
    Start-Sleep -Milliseconds 500
    if (@(Get-MailRows).Count -eq $Expected) {
      # The rows arrive with the snapshot that carries the count line; give the binding a beat
      # before anything reads the footer off the back of this.
      Start-Sleep -Milliseconds 500
      return @(Get-MailRows)
    }
  }
  return @(Get-MailRows)
}

<#
.SYNOPSIS
The lines app.log has grown by since byte -Offset.
.DESCRIPTION
Opened sharing ReadWrite because the app is writing to it as this reads: Get-Content is refused.
#>
function Get-LogLinesSince {
  param([Parameter(Mandatory)] [string] $Path, [Parameter(Mandatory)] [long] $Offset)
  $stream = [System.IO.File]::Open($Path, 'Open', 'Read', 'ReadWrite')
  try {
    $null = $stream.Seek($Offset, 'Begin')
    $reader = New-Object System.IO.StreamReader($stream)
    try { $reader.ReadToEnd() -split "`n" } finally { $reader.Dispose() }
  } finally { $stream.Dispose() }
}

<#
.SYNOPSIS
The list's own count line ("1 conversation"), as the footer draws it.
.DESCRIPTION
Matched by text: the footer's TextBlock is bound and carries no AutomationId, and the sentence is
its whole job, so reading anything else would not be reading what the user reads.
#>
function Get-CountFooter {
  $found = Get-UiaTree (Get-MailcalWindow) |
    Where-Object { $_.Current.Name -match '^\d+ (conversation|message|result)' } |
    Select-Object -First 1
  if (-not $found) { throw 'the message list draws no count line at all' }
  $found.Current.Name
}

<#
.SYNOPSIS
The conversation badge on -Row (how many messages it carries), or 0 when it draws none.
#>
function Get-RowMessageCount {
  param([Parameter(Mandatory)] $Row)
  $badge = Get-UiaTree $Row |
    Where-Object {
      $_.Current.ControlType.ProgrammaticName -eq 'ControlType.Text' -and $_.Current.Name -match '^\d+$'
    } | Select-Object -First 1
  if (-not $badge) { return 0 }
  [int] $badge.Current.Name
}

$Suite = @{
  Dataset = 'showcase'
  Cases   = @(
    @{
      Name = 'a search lists the conversation its matches belong to, not the matches'
      Body = {
        # Rule 9. Before it, search projected flat rows whatever the list was set to, so this
        # query answered with a row per matching message: one thread meant one thing in the
        # mailbox and another here, and archiving the row did two different things on two screens
        # (docs/list-selection.md).
        $rows = Set-Search -Query $Thread.WholeThread -Expected 1
        Assert-Equal 1 $rows.Count (
          'every message in the seeded thread matches this query, so a search that obeys the ' +
          'list grouping answers with the ONE conversation they are all in')
      }
    },
    @{
      Name = 'and the row carries the whole conversation, not the part that matched'
      Body = {
        # The badge is the client's statement of what the row stands for. A row carrying fewer
        # than the thread's messages is a fragment wearing a conversation's clothes, and nothing
        # downstream (open, select, archive) would mean what the mailbox means by it.
        $row = Get-MailRowByTitle $Thread.Title
        Assert-Equal $Thread.Messages (Get-RowMessageCount -Row $row) (
          'a conversation row stands for its whole thread in the mailbox, so it has to stand ' +
          'for its whole thread in the results too')
      }
    },
    @{
      Name = 'a count of one reads as one'
      Body = {
        # The catalog's plural forms, from the client's side: `mailbox_count_conversations` has a
        # `_one` partner now, and the generated `L10n.MailboxCountConversations` picks between
        # them. Nothing else on Windows exercises that branch.
        Assert-Equal '1 conversation' (Get-CountFooter) (
          'a folder holding one thing said "1 conversations"')
      }
    },
    @{
      Name = 'a count of none stays plural, which is what makes the line above an assertion'
      Body = {
        # Without this, the case above passes just as well against a client that lost the plural
        # altogether and says "1 conversation" for every count there is.
        $null = Set-Search -Query $NoMatch -Expected 0
        Assert-Equal '0 conversations' (Get-CountFooter) (
          'English takes the plural at zero; only French takes the singular there (CLDR)')
      }
    },
    @{
      Name = 'a conversation is labelled by the message that matched, not by its newest'
      Body = {
        # The half of rule 9 that a flat list cannot fake, and that grouping alone does not buy:
        # this query matches the two OLDER messages, so the row has to read as Eva's reply. A
        # thread summarised by an unrelated later message answers nothing the user asked, and it
        # is also what would sort the thread by a date no match happened on (rule 1).
        $rows = Set-Search -Query $Thread.OlderOnly -Expected 1
        Assert-Equal 1 $rows.Count 'the query is a word only this thread uses'
        $name = $rows[0].Current.Name
        Assert-True ($name -like "*$($Thread.NewestMatch)*") (
          "the row reads <$name>; the newest message that MATCHED is " +
          "$($Thread.NewestMatch)'s, and that is what a result has to be labelled by")
        Assert-True ($name -notlike "*$($Thread.Newest)*") (
          "the row reads <$name>, labelled by the follow-up from $($Thread.Newest), which is the " +
          'newest message in the thread but matched nothing: that is how the MAILBOX list labels ' +
          'this thread, and a result labelled that way answers nothing the user asked')
      }
    },
    @{
      Name = 'the core is asked after the typing stops, not while it is happening'
      Body = {
        # Rule 10's client half. The core drops a superseded list, but only the client knows a
        # keystroke happened, so only the client can stop nine searches being started to type one
        # word. Real keystrokes, because that is the whole question: Set-UiaText writes the value
        # in one go and would pass against a field with no debounce at all.
        #
        # WHAT IS MEASURED IS THE GAP, not how many rebuilds there were, and that is deliberate.
        # SendWait returns as soon as the key is posted, so how many TextChanged events WinUI
        # raises for a word depends on how the loop below races its event pump: measured here, a
        # 40ms gap coalesces nine keys into ONE event, which hands a counting assertion a
        # confident green against a field that has no debounce at all, while a 150ms gap
        # sometimes drifts past the 250ms the debounce waits and splits one word into three
        # searches. A gap cannot be tuned into both at once. The DELAY has no such window: the
        # first rebuild after the last key lands a debounce later when there is one, and within
        # milliseconds when there is not, whatever the pump did in between.
        $null = Set-Search -Query ''
        Start-Sleep -Seconds 2
        $before = (Get-Item -LiteralPath $LogPath).Length

        $box = Get-SearchBox
        $app = Get-Process Mailcal | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
        $null = [Allodia.UiaDpi]::SetForegroundWindow($app.MainWindowHandle)
        $box.SetFocus()
        Start-Sleep -Milliseconds 300
        Add-Type -AssemblyName System.Windows.Forms
        $word = 'quarterly'
        foreach ($c in $word.ToCharArray()) {
          [System.Windows.Forms.SendKeys]::SendWait($c)
          Start-Sleep -Milliseconds 120
        }
        $lastKey = [DateTimeOffset]::Now
        Start-Sleep -Seconds 3

        Assert-Equal $word (Get-UiaText $box) (
          'the keystrokes have to have reached the field, or this case measures nothing')
        $answered = Get-LogLinesSince -Path $LogPath -Offset $before |
          Where-Object { $_ -match $RebuildPattern } |
          ForEach-Object { [DateTimeOffset]::Parse($Matches.ts, [Globalization.CultureInfo]::InvariantCulture) } |
          Where-Object { $_ -ge $lastKey } |
          Select-Object -First 1
        Assert-True ($null -ne $answered) (
          'the core rebuilt nothing after the last key, so the word never reached it at all')
        $waited = ($answered - $lastKey).TotalMilliseconds
        Assert-True ($waited -ge 200) (
          "the list was rebuilt ${waited}ms after the last key; a search is a full-text query per " +
          'account plus a store read per hit, and they run concurrently, so the field has to go ' +
          'quiet for 250ms before the core is asked (docs/search.md, "Typing does not mean searching")')

        # Leave the list unsearched: the runner relaunches when a suite hands the next one a
        # different set of controls, and a search adds the horizon line under the header.
        $null = Set-Search -Query ''
      }
    }
  )
}
