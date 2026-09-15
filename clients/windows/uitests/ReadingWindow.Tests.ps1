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
$ThreadSubject = 'Project kickoff'      # the seeded two-message conversation, by its thread subject
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
  $subject = Find-UiaElement -AutomationId 'SubjectText' -Root $root
  if ($subject) { $subject.Current.Name } else { '' }
}

$Suite = @{
  Dataset = 'harness'
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
        # THE REGRESSION THIS CASE IS NAMED AFTER. The window appeared, and about fifty
        # milliseconds later disappeared behind the mailbox. Two separate causes, and the second
        # one is why a plain foreground check is not enough on its own: Activate() left the
        # FOREGROUND on the mailbox, and even once the window took it, the mailbox's message list
        # still held keyboard focus, so the next snapshot reconciled the rows under it, restored
        # focus there and re-activated the mailbox with it.
        #
        # So it is checked twice: once as soon as the window is up, and again after the snapshots
        # the double-click set off have landed. Only the second one fails on the bug.
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
        #   * a window that did open loses the foreground, because WinUI 3 reassigns focus when
        #     the element holding it is removed and that activates the mailbox
        #     (microsoft-ui-xaml#8520). That is what the case above is named for, and it is why
        #     the foreground is checked here too, well past the mark-read.
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
