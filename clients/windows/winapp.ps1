#!/usr/bin/env pwsh
# Physical input for the running client, through Microsoft's `winapp ui` CLI.
#
# uia.ps1 dot-sources this, so every suite and script that uses it has both halves:
#
#     $row = Get-MailRowByTitle 'Your June usage report'   # FOUND in-process (uia.ps1)
#     Invoke-WinappClick $row -Right                        # PRESSED by winapp (here)
#
# A suite finds an element in-process, with exact names and a scoped walk, and hands it here to be
# clicked, hovered, dragged or typed at. `winapp` does the pressing because it is DPI-aware and
# injects real pointer and keyboard input, so a press lands where it was aimed at any display
# scale. It is the PowerShell suites' implementation of input; the suites in Mailcal.UiTests send
# their own (Driver/Input.cs).
#
# Why finding stays in-process: every `winapp` verb is a process of its own, 0.2–0.7 s a call
# (winapp 0.7.1, measured against this app), where an in-process read takes milliseconds. A suite
# reads the tree hundreds of times and presses a handful, so the presses can pay that and the reads
# cannot. A batched in-process `winapp` is upstream work (microsoft/winappCli#824).
#
# Traps, each found by hitting it on winapp 0.7.1:
#   * A text selector is a SUBSTRING match, and `click`, `hover` and `invoke` take no --root or
#     --type: `click 'Allodia Mail & Calendar'` pressed the mail row "Welcome to Allodia Mail &
#     Calendar". Nothing here presses by text. Get-WinappSelector turns the element into winapp's
#     own slug by matching its name, its control type AND its bounds on screen.
#   * `-a <pid>` picks the FOREGROUND window when the app has more than one, so every call names
#     the element's own top-level window with -w. A flyout is a window of its own (PopupHost).
#   * While a composer is open, winapp's walk of the window never ends: it finds a cycle through the
#     editor's WebView2 that the UI Automation tree does not have. A lookup that finds its element
#     returns, slowly; a search over the whole window does not return at all, and that holds until
#     the editor's browser window has gone, which can be after the composer has left the screen.
#     So a composer is closed by Close-Composer (uia.ps1), which waits for that window, and the
#     runner counts the hosts between suites (dataset.ps1). Every call here is capped, and one that
#     runs out says so rather than hanging the suite. A keystroke and a press at a point look
#     nothing up, so both work in a composer.

$script:WinappMinimum = [version] '0.7.0'

function Get-WinappPath {
  if ($script:WinappPath) { return $script:WinappPath }
  $command = Get-Command winapp -ErrorAction SilentlyContinue
  if (-not $command) {
    throw "winapp is not installed, and every press in these suites goes through it: winget install Microsoft.WinAppCLI"
  }
  $reported = (& $command.Source --version 2>$null | Select-Object -First 1) -replace '[^0-9.].*$', ''
  $version = $null
  if (-not [version]::TryParse($reported, [ref] $version) -or $version -lt $script:WinappMinimum) {
    throw "winapp $reported is older than $($script:WinappMinimum), which brought --type, --root and --action: winget upgrade Microsoft.WinAppCLI"
  }
  $script:WinappPath = $command.Source
  $script:WinappPath
}

<#
.SYNOPSIS
Runs one `winapp ui` verb against -Window and returns its JSON answer as an object.
.DESCRIPTION
Throws when winapp reports an error or exits non-zero, and when it has not answered within
-TimeoutSec, killing it.

A search walks the whole window while the app is free to change it, and an element that goes away
under the walk fails the search with "Element is no longer accessible" (winapp 0.7.1), as a suite
does right after the one before it closed a window. A search presses nothing, so that one failure
is asked again, up to $WinappSearchAttempts times in all. Every other verb is asked once: a press
that failed partway may already have landed, and repeating it would press twice.
#>
function Invoke-Winapp {
  param(
    [Parameter(Mandatory, Position = 0)] [string] $Verb,
    [Parameter(Position = 1)] [string[]] $Arguments = @(),
    [Parameter(Mandatory)] [long] $Window,
    [int] $TimeoutSec = 30
  )
  $attempts = if ($Verb -eq 'search') { $script:WinappSearchAttempts } else { 1 }
  for ($attempt = 1; ; $attempt++) {
    try {
      return Invoke-WinappOnce $Verb $Arguments -Window $Window -TimeoutSec $TimeoutSec
    }
    catch {
      if ($attempt -ge $attempts -or $_.Exception.Message -notmatch 'Element is no longer accessible') { throw }
      Start-Sleep -Milliseconds 250
    }
  }
}

$script:WinappSearchAttempts = 3

function Invoke-WinappOnce {
  param(
    [Parameter(Mandatory, Position = 0)] [string] $Verb,
    [Parameter(Position = 1)] [string[]] $Arguments = @(),
    [Parameter(Mandatory)] [long] $Window,
    [int] $TimeoutSec
  )
  $start = [Diagnostics.ProcessStartInfo]::new((Get-WinappPath))
  foreach ($argument in @('ui', $Verb) + $Arguments + @('-w', "$Window", '--json')) {
    $start.ArgumentList.Add($argument)
  }
  $start.RedirectStandardOutput = $true
  $start.RedirectStandardError = $true
  $start.UseShellExecute = $false
  $process = [Diagnostics.Process]::Start($start)
  $out = $process.StandardOutput.ReadToEndAsync()
  $err = $process.StandardError.ReadToEndAsync()
  if (-not $process.WaitForExit($TimeoutSec * 1000)) {
    $process.Kill($true)
    # What the window held when it happened, so the next reader of a red run need not guess.
    $hosts = try { Get-WebViewHostCount } catch { '?' }
    throw ("winapp ui $Verb $($Arguments -join ' ') did not answer within ${TimeoutSec}s, with $hosts " +
      'WebView2 host(s) in the mailbox window. A composer''s editor among them, open or still closing, is ' +
      'winapp walking the cycle through it (see winapp.ps1), not the app hanging')
  }
  # An answer is JSON on stdout, but a refusal can arrive as JSON on stderr instead.
  $answer = $null
  foreach ($text in @($out.Result.Trim(), $err.Result.Trim())) {
    if ($answer) { break }
    try { $answer = $text | ConvertFrom-Json -Depth 50 } catch { }
  }
  if (-not $answer) {
    throw "winapp ui $Verb answered with no JSON (exit $($process.ExitCode)): $($out.Result.Trim()) $($err.Result.Trim())"
  }
  if ($answer.error) { throw "winapp ui ${Verb}: $($answer.error.message)" }
  # A failure without an error object is still a failure. The one non-zero exit that is an answer is
  # a search that matched nothing, which exits 1 (winapp 0.7.1) and leaves the count to the caller.
  if ($process.ExitCode -ne 0 -and -not ($Verb -eq 'search' -and $answer.matchCount -eq 0)) {
    throw "winapp ui $Verb $($Arguments -join ' ') exited $($process.ExitCode): $($out.Result.Trim()) $($err.Result.Trim())"
  }
  $answer
}

<#
.SYNOPSIS
The handle of the top-level window -Element is drawn in.
#>
function Get-UiaWindowHandle {
  param([Parameter(Mandatory, Position = 0)] [object] $Element)
  $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
  $root = [System.Windows.Automation.AutomationElement]::RootElement
  $node = $Element
  while ($true) {
    $parent = $walker.GetParent($node)
    if (-not $parent -or $parent -eq $root) { break }
    $node = $parent
  }
  $handle = $node.Current.NativeWindowHandle
  if (-not $handle) { throw "'$($Element.Current.Name)' is in no window with a handle" }
  [long] $handle
}

<#
.SYNOPSIS
Brings -Window to the front, unless it is there already.
.DESCRIPTION
winapp refuses to press or type into a window that is not in front, rather than act on the wrong
one. Windows in turn lets a process take the front only when it was the last to send input, so a
plain SetForegroundWindow from this script, and winapp's own `screenshot --focus`, are refused
while the user, or the launch that just failed to take the front, owns it. When it is refused, a
keystroke makes this script the last to send input, and the request after it is granted. That key
reaches whichever window is in front, which is then someone else's, so it is 0xE8: Windows assigns
it to nothing, and no window acts on it.
Throws when the window is still not in front, because every press after it would be refused.
#>
function Show-WinappWindow {
  param([Parameter(Mandatory, Position = 0)] [long] $Window)
  $target = [IntPtr] $Window
  if ([Allodia.UiaDpi]::GetForegroundWindow() -eq $target) { return }
  if (-not [Allodia.UiaDpi]::SetForegroundWindow($target)) {
    [Allodia.UiaDpi]::keybd_event(0xE8, 0, 0, [UIntPtr]::Zero)   # down
    [Allodia.UiaDpi]::keybd_event(0xE8, 0, 2, [UIntPtr]::Zero)   # KEYEVENTF_KEYUP
    [void][Allodia.UiaDpi]::SetForegroundWindow($target)
  }
  $timer = [Diagnostics.Stopwatch]::StartNew()
  while ([Allodia.UiaDpi]::GetForegroundWindow() -ne $target) {
    if ($timer.ElapsedMilliseconds -gt 2000) { throw "window $Window would not come to the front" }
    Start-Sleep -Milliseconds 50
  }
  Start-Sleep -Milliseconds 300
}

<#
.SYNOPSIS
winapp's selector for -Element: the one element with its name, its control type and its bounds.
.DESCRIPTION
winapp's slugs are hashes minted per run, so one is looked up afresh rather than kept across
launches. A match by name alone would be a substring match, which is how a press meant for one
element lands on another that contains its name; the bounds pin it to the element the suite found.
An element with no name is looked up by its AutomationId, which winapp searches too.
#>
function Get-WinappSelector {
  param([Parameter(Mandatory, Position = 0)] [object] $Element)
  $current = $Element.Current
  $query = if ($current.Name) { $current.Name } else { $current.AutomationId }
  if (-not $query) {
    throw "an element with neither a name nor an AutomationId cannot be handed to winapp ($($current.ControlType.ProgrammaticName)); press its named child or its named parent instead"
  }
  $bounds = $current.BoundingRectangle
  if ($bounds.IsEmpty -or [double]::IsInfinity($bounds.X)) {
    throw "'$($current.Name)' is not on screen, so there is nothing to press"
  }
  $type = $current.ControlType.ProgrammaticName -replace '^ControlType\.', ''
  $found = Invoke-Winapp search @($query, '--type', $type, '--max', '200') -Window (Get-UiaWindowHandle $Element)
  # An empty name or AutomationId can be absent from winapp's answer rather than empty, and $null
  # is not equal to '', so both sides are compared as strings.
  $same = @($found.matches | Where-Object {
      [string] $_.name -eq [string] $current.Name -and
      [string] $_.automationId -eq [string] $current.AutomationId -and
      [Math]::Abs($_.x - $bounds.X) -le 2 -and [Math]::Abs($_.y - $bounds.Y) -le 2 -and
      [Math]::Abs($_.width - $bounds.Width) -le 2 -and [Math]::Abs($_.height - $bounds.Height) -le 2
    })
  if ($same.Count -ne 1) {
    throw "winapp found $($same.Count) $type element(s) '$query' at $bounds, where exactly one was wanted"
  }
  $same[0].selector
}

<#
.SYNOPSIS
Clicks the middle of -Element with the mouse: a left click, a right click, or a double click.
#>
function Invoke-WinappClick {
  param([Parameter(Mandatory, Position = 0)] [object] $Element, [switch] $Right, [switch] $Double)
  $arguments = @(Get-WinappSelector $Element)
  if ($Right) { $arguments += '--right' }
  if ($Double) { $arguments += '--double' }
  $window = Get-UiaWindowHandle $Element
  Show-WinappWindow $window
  [void](Invoke-Winapp click $arguments -Window $window)
}

<#
.SYNOPSIS
Presses and releases the mouse at -Point (physical screen pixels) in -Window: a click, or with
-Right a right-click.
.DESCRIPTION
For a press that has to land on a part of a control rather than its middle, the header line of an
open folder row whose bounds take in its subfolders, or inside the composer's body, where winapp
cannot look an element up. winapp's `click` takes no point, so this is its `drag` with nowhere to
go: the button goes down and comes up on the same spot, which is what a click is.
#>
function Invoke-WinappPress {
  param([Parameter(Mandatory)] [int[]] $Point, [Parameter(Mandatory)] [long] $Window, [switch] $Right)
  $at = "$($Point[0]),$($Point[1])"
  $arguments = @($at, $at)
  if ($Right) { $arguments += '--right' }
  Show-WinappWindow $Window
  [void](Invoke-Winapp drag $arguments -Window $Window)
}

<#
.SYNOPSIS
Moves the pointer onto the middle of -Element and rests there for -DwellMs.
.DESCRIPTION
Real pointer input, so the element under it gets its pointer-over: hovering a button with a tooltip
raises the tooltip. A cursor warp does not, which is why a hover is never a SetCursorPos.
#>
function Invoke-WinappHover {
  param([Parameter(Mandatory, Position = 0)] [object] $Element, [int] $DwellMs = 800)
  $arguments = @(Get-WinappSelector $Element) + @('--dwell-time', "$DwellMs")
  $window = Get-UiaWindowHandle $Element
  Show-WinappWindow $window
  [void](Invoke-Winapp hover $arguments -Window $window)
}

<#
.SYNOPSIS
Presses on the middle of -From, moves to -ToPoint (physical screen pixels), and lets go after -DwellMs.
.DESCRIPTION
A point rather than an element at the far end, because a drop target is often a part of a control
rather than its middle: an open folder's row covers its subfolders too.
#>
function Invoke-WinappDrag {
  param(
    [Parameter(Mandatory)] [object] $From,
    [Parameter(Mandatory)] [int[]] $ToPoint,
    [int] $DwellMs = 400
  )
  $arguments = @((Get-WinappSelector $From), "$($ToPoint[0]),$($ToPoint[1])", '--dwell-ms', "$DwellMs")
  $window = Get-UiaWindowHandle $From
  Show-WinappWindow $window
  [void](Invoke-Winapp drag $arguments -Window $window)
}

<#
.SYNOPSIS
Turns the mouse wheel over -Element by -Notches: positive is up, negative is down.
.DESCRIPTION
A caller that wheels one notch at a time and reads the result in between passes the -Selector it
looked up once, rather than paying for the lookup on every notch.
#>
function Invoke-WinappWheel {
  param(
    [Parameter(Mandatory, Position = 0)] [object] $Element,
    [Parameter(Mandatory)] [int] $Notches,
    [string] $Selector = (Get-WinappSelector $Element)
  )
  $arguments = @($Selector, '--wheel', "$Notches")
  $window = Get-UiaWindowHandle $Element
  Show-WinappWindow $window
  [void](Invoke-Winapp scroll $arguments -Window $window)
}

<#
.SYNOPSIS
Types -Keys at whatever holds keyboard focus in -Window, as real keystrokes.
.DESCRIPTION
winapp's key syntax: named keys and chords as tokens (`esc`, `ctrl+a`, `enter`), or -Text to type the
argument as written. Sent as real input (`--via send-input`), because WinUI's controls are windowless
and ignore the posted messages winapp sends by default, and an accelerator fires only on real input.
Real input goes to whatever window is in front, so -Window is brought forward first
(Show-WinappWindow).
#>
function Send-WinappKeys {
  param(
    [Parameter(Mandatory, Position = 0)] [string] $Keys,
    [long] $Window = [long] (Get-MailcalWindow).Current.NativeWindowHandle,
    [switch] $Text
  )
  Show-WinappWindow $Window
  $arguments = @($Keys, '--via', 'send-input')
  if ($Text) { $arguments += '--verbatim' }
  [void](Invoke-Winapp send-keys $arguments -Window $Window)
}
