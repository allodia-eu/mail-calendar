# A message in a window of its own (docs/reading-window.md): the double-click that opens it, the
# pane it must not disturb, the action row it carries, and the composer window a reply raises.
#
# WHY THIS SUITE AND NOT Mailcal.Tests. Everything here is a SECOND TOP-LEVEL WINDOW, and a plain
# net10.0 assembly cannot see one at all. The rules that matter are also exactly the ones a green
# unit test would not notice: whether the pane moved, whether a second double-click stacked a
# window or raised the open one, and whether the row of buttons the window draws is the row the
# contract fixes. The pure halves (which window a message belongs in, and what counts as a
# double-click) are pinned in Mailcal.Tests/ReadingWindowsTests.cs instead.
#
# WHY THE HARNESS. The window opens a real body into a slot of its own, and the showcase engine
# performs no real mail read, so what it would hand a second reader is fiction.
#
# WHY SYNTHETIC MOUSE INPUT. A double-click is the gesture under test and UI Automation has no
# pattern for one: Invoke raises a click, never two, and the whole point is what the SECOND press
# does. So the presses are injected at the OS level with mouse_event, the same way
# CalendarWheel.Tests injects a wheel. The context-menu route beside it needs none of that, which
# is the point of having it: it is the route a keyboard or a screen reader takes, and it is what
# the capability matrix claims.

$CatalogDir = Join-Path $PSScriptRoot '../../../messages'
$RowSubject = 'Quarterly planning'      # seeded, has an invitation card, so the window has content
$OtherSubject = 'HTML message with a remote image'
# The seeded two-message conversation. A WILDCARD because the row's title is whichever message is
# newest: a freshly seeded harness shows the reply, 'Re: Project kickoff', and one that has been
# read and re-synced shows the original. Pinning either spelling makes this case depend on how long
# ago the harness was reset, which is not what it is testing.
$ThreadSubject = '*Project kickoff'
# The action row in the order docs/reading-actions.md fixes, overflow last of all.
$RowButtons = @(
  'ReadingReply'
  'ReadingReplyAll'
  'ReadingForward'
  'ReadingArchive'
  'ReadingDelete'
  'ReadingOverflow'
)

# The window and pointer helpers, shared so a second suite can assert on windows too.
. (Join-Path $PSScriptRoot 'appwindows.ps1')
. (Join-Path $PSScriptRoot '../applog.ps1')

<#
.SYNOPSIS
Every catalog locale's value for one message key.
.DESCRIPTION
The whole catalog rather than one locale, for the reason InvitationReplyPrompt.Tests gives: a
harness run comes up in whatever language the developer prefers, and re-deriving LanguageStore's
fallback chain here would turn a correct app into a red suite. Matching against the set asks the
question the test actually has, is this the item bound to THIS key.
#>
function Get-CatalogValues {
  param([Parameter(Mandatory)] [string] $Key)
  $values = @()
  foreach ($file in Get-ChildItem -Path $CatalogDir -Filter '*.json') {
    $json = Get-Content -LiteralPath $file.FullName -Raw -Encoding utf8 | ConvertFrom-Json
    if ($json.PSObject.Properties.Name -contains $Key) { $values += $json.$Key }
  }
  if (-not $values) { throw "no catalog locale defines '$Key', the key was renamed or removed" }
  $values
}

<#
.SYNOPSIS
The item in -Subject's row menu whose label is the catalog's -Key, in whatever locale the app came
up in, or $null when the menu does not offer it.
.DESCRIPTION
Beside Get-CatalogValues rather than in appwindows.ps1, because reading the catalog is what it
does; opening the menu is one line of that.

Leaves the menu open, so a caller that finds nothing closes it itself.
#>
function Find-RowMenuItem {
  param([Parameter(Mandatory)] [string] $Subject, [Parameter(Mandatory)] [string] $Key)
  Show-RowContextMenu -Subject $Subject
  $labels = Get-CatalogValues $Key
  $item = $null
  $timer = [Diagnostics.Stopwatch]::StartNew()
  while (-not $item -and $timer.Elapsed.TotalSeconds -lt 10) {
    $item = Find-UiaElements -Type 'MenuItem' |
      Where-Object { $labels -contains $_.Current.Name } | Select-Object -First 1
    if (-not $item) { Start-Sleep -Milliseconds 200 }
  }
  $item
}

<#
.SYNOPSIS
The subject the reading PANE is showing, or '' when it is on its placeholder.
#>
function Get-PaneSubject {
  $main = Get-MainWindow
  if (-not $main) { throw 'the mailbox window is not open' }
  $root = [System.Windows.Automation.AutomationElement]::FromHandle($main.Handle)
  # A reading window is OWNED by the mailbox, so UI Automation hangs it under the mailbox's own
  # element and a plain descendant search for the shared 'SubjectText' can answer with the WINDOW's
  # subject. That reads as the pane having moved when it has not: a test reporting a product bug
  # that does not exist. So take every subject under the mailbox and subtract the ones belonging to
  # a window; what is left is the pane's. Runtime ids, because that is UI Automation's own identity
  # for an element and geometry is not.
  $all = @(Find-UiaElements -AutomationId 'SubjectText' -Root $root)
  foreach ($extra in Get-ExtraWindows) {
    $window = [System.Windows.Automation.AutomationElement]::FromHandle($extra.Handle)
    foreach ($theirs in @(Find-UiaElements -AutomationId 'SubjectText' -Root $window)) {
      $all = @($all | Where-Object { -not [System.Windows.Automation.Automation]::Compare($_, $theirs) })
    }
  }
  if ($all.Count -gt 0) { $all[0].Current.Name } else { '' }
}

$Suite = @{
  Dataset = 'harness'
  # DEBUG, because one case below reads the app's own log: the rule it checks (opening a window
  # must not activate the mailbox) is invisible from the outside, and the log is where the app
  # says it. Costs this suite a launch of its own, which is what an Env does.
  Env     = @{ ALLODIA_LOG_LEVEL = 'debug' }
  # Every case starts from "no extra windows and a known message in the pane", because these open
  # windows for a living and a leftover one from the case before would answer for this one.
  Prepare = {
    Close-ExtraWindows
    Invoke-RowClicks -Subject $OtherSubject
  }
  Cases   = @(
    @{
      Name = 'a double-click opens the message in a window of its own'
      Body = {
        Close-ExtraWindows
        Invoke-RowClicks -Subject $OtherSubject
        Invoke-RowClicks -Subject $RowSubject -Times 2
        $null = Wait-AppWindow -Title $RowSubject
        Assert-Equal 1 (Get-ExtraWindows).Count 'exactly one window opened'
      }
    },
    @{
      Name = 'and leaves the reading pane on the message it was showing'
      Body = {
        # THE RULE THE WHOLE ARRANGEMENT EXISTS FOR, and the one that fails silently. The list
        # raises its click on the FIRST press of a double-click, so the pane opens the row before
        # anything knows a window was wanted; it is put back afterwards. Opening a second reader
        # must not disturb the first.
        Close-ExtraWindows
        Invoke-RowClicks -Subject $OtherSubject
        Assert-Equal $OtherSubject (Get-PaneSubject) 'the pane starts on the other message'

        Invoke-RowClicks -Subject $RowSubject -Times 2
        $null = Wait-AppWindow -Title $RowSubject
        Assert-Equal $OtherSubject (Get-PaneSubject) `
          'the pane keeps the message it had; a window is a second reader, not a replacement'
      }
    },
    @{
      Name = 'and the window stays in front rather than sinking behind the mailbox'
      Body = {
        # THE SYMPTOM, read from outside: the window opened and, tens of milliseconds later, was
        # behind the mailbox. This is the weaker of the two checks on that rule, because a steal
        # shorter than a screen read passes it; the case below that reads the app's log is the one
        # with teeth. It is kept because it is the state a reader would actually report, and
        # because it is the only one that would see a window sinking for a reason nobody has
        # thought of yet.
        #
        # Twice, because the first read lands before the snapshots the double-click set off have
        # arrived and the window has to still be in front once they have.
        Close-ExtraWindows
        Invoke-RowClicks -Subject $OtherSubject
        Invoke-RowClicks -Subject $RowSubject -Times 2
        $null = Wait-AppWindow -Title $RowSubject
        Assert-Equal $RowSubject (Get-ForegroundTitle) `
          'the window is the one in front once the double-click has settled'
        Start-Sleep -Seconds 2
        Assert-Equal $RowSubject (Get-ForegroundTitle) `
          'and still is a further two seconds later, with the mailbox done reconciling behind it'
        # The crash that the first attempt at this fix caused: asking the focus manager to search a
        # window whose visual tree has not loaded throws, out of a double-click handler, taking the
        # process with it. Every assertion above would have read green on the way down.
        Assert-True ($null -ne (Get-Process Mailcal -ErrorAction SilentlyContinue)) `
          'and the app is still running'
      }
    },
    @{
      Name = 'an unread message opens in a window, though being marked read rewrites its row'
      Body = {
        # THE CASE ABOVE PASSES ON A MESSAGE THAT IS ALREADY READ, which is the whole of why the
        # bug survived it. Opening an unread one marks it read, which rewrites its row, and the
        # reconcile used to REPLACE the row's item, so the ListView threw away the container the
        # gesture was working on. Two things follow from that one destruction, and a live mailbox
        # rewrites rows constantly, so a user met both:
        #
        #   * the double-click does not open a window at all, which is what this case catches
        #     first and what the assertion below reports;
        #   * a window that did open loses the foreground, which is the rule the log-reading case
        #     below holds; it is checked here as well, well past the mark-read, because this is the
        #     one case where the row the gesture is working on is rewritten under it.
        Close-ExtraWindows
        $unread = Find-RowMenuItem -Subject $RowSubject -Key 'action_mark_unread'
        if ($unread) { Invoke-UiaElement $unread }
        else { [System.Windows.Forms.SendKeys]::SendWait('{ESC}') }
        # Let the mark-UNREAD rewrite land before the gesture, so the only row rewrite left is the
        # mark-READ the double-click itself causes. Without this the row moves under the pointer
        # mid-gesture and the case fails one step earlier, on a window that never opens, which is
        # the same defect wearing a symptom that hides the one this case is named for.
        Start-Sleep -Seconds 3
        Invoke-RowClicks -Subject $RowSubject -Times 2
        $null = Wait-AppWindow -Title $RowSubject
        Assert-Equal $RowSubject (Get-ForegroundTitle) 'the window is in front when it opens'
        Start-Sleep -Seconds 4
        Assert-Equal $RowSubject (Get-ForegroundTitle) `
          'and is still in front once the message has been marked read'
        Assert-True ($null -ne (Get-Process Mailcal -ErrorAction SilentlyContinue)) `
          'and the app is still running'
      }
    },
    @{
      Name = 'a regular message stays in front after its body has been in the pane'
      Body = {
        # THE REPORTED RECIPE, and the one no case here performed: single-click a REGULAR message
        # so the pane renders its body, then double-click THE SAME row. Every other case in this
        # file double-clicks the invitation, because its card guarantees the window has content to
        # draw, and the invitation is exactly the message type the fault spares: its card is native
        # XAML, where a regular body is a document in a WebView2. A fixture chosen to make one
        # assertion easy had been hiding the defect from all the others.
        Close-ExtraWindows
        Invoke-RowClicks -Subject $OtherSubject
        Assert-Equal $OtherSubject (Get-PaneSubject) 'the pane is showing the message first'
        Start-Sleep -Seconds 2
        Invoke-RowClicks -Subject $OtherSubject -Times 2
        $null = Wait-AppWindow -Title $OtherSubject
        Assert-Equal $OtherSubject (Get-ForegroundTitle) 'the window is in front when it opens'
        Start-Sleep -Seconds 5
        Assert-Equal $OtherSubject (Get-ForegroundTitle) `
          'and is still in front five seconds later'
        Assert-True ($null -ne (Get-Process Mailcal -ErrorAction SilentlyContinue)) `
          'and the app is still running'
      }
    },
    @{
      Name = 'the mailbox does not climb over a window while it opens'
      Body = {
        # THE RULE THE WHOLE Z-ORDER RESTS ON, checked from the app's own log because nothing
        # outside can see it: the steal it guards against lasted tens of milliseconds and the app
        # had the front back before a screen read landed, so a suite full of foreground checks
        # passed on every run while the window still sank.
        #
        # The rule is that the mailbox takes NO activation of its own after a window opens. The
        # shell DOES put the window back when one happens (KeepInFrontWhileItSettles), and this
        # case deliberately refuses that as an excuse: the correction is there for the reader, not
        # to launder a regression, and a gate that accepted it would have passed the very fault
        # this case is named for. The activation it was written for landed about 35 ms in, because
        # the window was shown from inside the second press of the double-click and the list then
        # focused the row under the pointer.
        #
        # ON AN ORDINARY MESSAGE, not the invitation the cases above use: an invitation's card is
        # native XAML where an ordinary body is a WebView2 document, and the fixture that makes an
        # assertion easy is not the one closest to what breaks.
        Close-ExtraWindows
        # The pane on a different message, so the double-click's restore actually runs; on the
        # message already open, RestoreReadingPane returns without touching anything.
        Invoke-RowClicks -Subject $RowSubject
        $since = Get-Date
        Invoke-RowClicks -Subject $OtherSubject -Times 2
        $null = Wait-AppWindow -Title $OtherSubject
        Start-Sleep -Seconds 4
        $lines = (Get-AppLogNewestSession) -split "`n"
        $openedAt = $null
        $opened = 0
        $gaveTheFront = 0
        $took = @()
        foreach ($line in $lines) {
          if ($line -notmatch '^(?<at>\S+ \S+ \S+) ') { continue }
          $at = [datetimeoffset]::MinValue
          if (-not [datetimeoffset]::TryParse($Matches.at, [ref] $at)) { continue }
          if ($at -lt ([datetimeoffset] $since)) { continue }
          if ($line -match 'reading window: opened') { $openedAt = $at; $opened++ }
          if (-not $openedAt -or $at -lt $openedAt) { continue }
          # Invoke-RowClicks brings the mailbox forward before it clicks, which is an activation
          # the harness asked for; only what happens AFTER the window opened is the app's doing.
          if ($line -match 'window order: mailbox (code|pointer)') { $took += $line }
          if ($line -match 'window order: mailbox lost the front') { $gaveTheFront++ }
        }
        Assert-True ($opened -gt 0) 'the app logged the window it opened'
        # A quiet log proves nothing on its own: without this, a run where the window never took
        # the front at all would report zero activations and read as a pass.
        Assert-True ($gaveTheFront -gt 0) 'and the window took the front off it, so there was a front to steal'
        Assert-Equal 0 $took.Count `
          "the mailbox took the front back after the window opened: $($took -join ' / ')"
        Assert-Equal $OtherSubject (Get-ForegroundTitle) 'and the window is the one in front at the end'
      }
    },
    @{
      Name = 'the window carries the whole action row, overflow last'
      Body = {
        Close-ExtraWindows
        Invoke-RowClicks -Subject $RowSubject -Times 2
        $window = Wait-AppWindow -Title $RowSubject
        # docs/reading-actions.md binds the window entire: same buttons, same order, overflow last.
        foreach ($id in $RowButtons) {
          Assert-True ($null -ne (Find-UiaElement -AutomationId $id -Root $window)) `
            "the window's action row carries #$id"
        }
        $lefts = $RowButtons | ForEach-Object {
          (Find-UiaElement -AutomationId $_ -Root $window).Current.BoundingRectangle.X
        }
        $sorted = @($lefts | Sort-Object)
        Assert-Equal ($sorted -join ',') ($lefts -join ',') `
          'the row is drawn in the contract order, left to right'
      }
    },
    @{
      Name = 'the window draws the app''s own caption, named after the message'
      Body = {
        # A window opened out of the mailbox is the same application, and until it drew its own
        # caption it did not look like one: the system's strip carries neither the app's theme nor
        # the caption the mailbox has beside it, so a message window read as something else the
        # desktop had put on screen.
        Close-ExtraWindows
        Invoke-RowClicks -Subject $RowSubject -Times 2
        $window = Wait-AppWindow -Title $RowSubject
        $caption = Find-UiaElement -AutomationId 'AppTitleBar' -Root $window
        Assert-True ($null -ne $caption) (
          'the window''s caption must be the app''s own TitleBar control, the same one the mailbox ' +
          'carries. Without it the system draws a strip that follows neither the appearance ' +
          'setting nor the brand')
        Assert-Equal $RowSubject $caption.Current.Name (
          'and it names the message, which is what tells two open windows apart in the window ' +
          'list the OS draws')

        # The same rule TitleBar.Tests.ps1 holds for the mailbox: the three buttons the app does
        # NOT draw still have to be as tall as the caption it does.
        $expected = ConvertTo-UiaPixels (Get-CaptionHeightDip)
        foreach ($entry in (Get-CaptionButtons -Root $window).GetEnumerator()) {
          $bounds = Get-RenderedBounds -Element $entry.Value -What "the window's $($entry.Key) button"
          Assert-True ([Math]::Abs($bounds.Height - $expected) -le 1) (
            "the $($entry.Key) button is $($bounds.Height)px tall against a caption of ${expected}px, " +
            'so this window''s caption was taken over only halfway')
        }
      }
    },
    @{
      Name = 'double-clicking the same message again raises its window instead of opening a second'
      Body = {
        # One window per message. A minted id would stack a second window here, and the two would
        # then hold two copies of one body.
        Close-ExtraWindows
        Invoke-RowClicks -Subject $RowSubject -Times 2
        $null = Wait-AppWindow -Title $RowSubject
        Invoke-RowClicks -Subject $RowSubject -Times 2
        Start-Sleep -Milliseconds 800
        Assert-Equal 1 (Get-ExtraWindows).Count 'still one window for this message'
      }
    },
    @{
      Name = 'the context menu opens one too, which is the route without a pointer'
      Body = {
        Close-ExtraWindows
        $item = Find-RowMenuItem -Subject $RowSubject -Key 'action_open_in_window'
        Assert-True ($null -ne $item) 'the row menu offers "open in new window"'
        Invoke-UiaElement $item
        $null = Wait-AppWindow -Title $RowSubject
        Assert-Equal 1 (Get-ExtraWindows).Count 'the menu item opened the window'
      }
    },
    @{
      Name = 'replying from the window opens the draft in a window of its own'
      Body = {
        # Never the shell's inline composer: a draft answering the window in front of you does not
        # belong in a different window behind it.
        Close-ExtraWindows
        Invoke-RowClicks -Subject $RowSubject -Times 2
        $window = Wait-AppWindow -Title $RowSubject
        Invoke-UiaElement (Find-UiaElement -AutomationId 'ReadingReply' -Root $window)
        # The draft's window is named after the draft, and its title is the CORE's derived Re:
        # subject, which this script cannot call. So it is found as the extra window that is not the
        # reading one, which is also the assertion worth making: the reading window stayed open
        # behind it, rather than the reply having replaced it.
        $timer = [Diagnostics.Stopwatch]::StartNew()
        while ((Get-ExtraWindows).Count -lt 2 -and $timer.Elapsed.TotalSeconds -lt 30) {
          Start-Sleep -Milliseconds 250
        }
        Assert-Equal 2 (Get-ExtraWindows).Count 'the reading window is still open behind its reply'
        $draftWindow = Get-ExtraWindows | Where-Object { $_.Title -ne $RowSubject } | Select-Object -First 1
        Assert-True ($null -ne $draftWindow) 'the draft opened in a window of its own'
        $draft = [System.Windows.Automation.AutomationElement]::FromHandle($draftWindow.Handle)
        Assert-True ($null -ne (Find-UiaElement -AutomationId 'ToField' -Root $draft)) `
          'and it is the full composer, not a stub'
        Assert-Equal $draftWindow.Title (Get-ForegroundTitle) `
          'the draft is the window in front: it is what the user just asked for'
        # Every window this app opens wears the brand, including the third one. The composer window
        # shipped without it once, because the icon was a line to remember beside the title rather
        # than something the window could not be built without.
        foreach ($window in Get-ExtraWindows) {
          Assert-True (Test-WindowIcon $window.Handle) `
            "the '$($window.Title)' window carries the app icon"
        }
      }
    },
    @{
      Name = 'a conversation header opens no window'
      Body = {
        # The row a window is for is a MESSAGE: a flat row, or one message inside an expanded
        # conversation. A conversation header is neither, and keeps what a second click already did
        # there, which on Windows is collapsing the thread the first click expanded.
        Close-ExtraWindows
        # The seeded thread's header. Asserted to BE one rather than assumed: a conversation row
        # carries a message count beside its subject and a flat row does not, so if the seed ever
        # stops threading these two this case fails here instead of passing over a flat row, which
        # opens no window for an entirely different reason.
        $thread = Get-MailRowByTitle $ThreadSubject
        $count = Get-UiaTree $thread |
          Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::Text } |
          Where-Object { $_.Current.Name -match '^\d+$' }
        Assert-True ($null -ne $count) `
          "'$ThreadSubject' is a conversation row, carrying the count that makes it one"

        # Clicked near its TOP edge, because the header expands on the first click and the sub-rows
        # it reveals are drawn below it: a click at the row's centre would land on the first
        # sub-row, which IS a message and would correctly open a window.
        $bounds = $thread.Current.BoundingRectangle
        Invoke-ClicksAt -X ([int] ($bounds.X + $bounds.Width / 2)) `
                        -Y ([int] ($bounds.Y + 12)) -Times 2
        Assert-Equal 0 (Get-ExtraWindows).Count `
          'a conversation is not a message, so double-clicking its header opens no window'
      }
    },
    @{
      Name = 'closing the window leaves the pane where it was'
      Body = {
        # The other half of "a window is a second reader": it takes nothing with it when it goes.
        Close-ExtraWindows
        Invoke-RowClicks -Subject $OtherSubject
        Invoke-RowClicks -Subject $RowSubject -Times 2
        $null = Wait-AppWindow -Title $RowSubject
        Close-ExtraWindows
        Assert-Equal 0 (Get-ExtraWindows).Count 'the window closed'
        Assert-Equal $OtherSubject (Get-PaneSubject) 'and the pane still holds its own message'
      }
    }
  )
}
