#!/usr/bin/env pwsh
# UI Automation primitives for driving and ASSERTING on the running WinUI client.
#
# Dot-source it; it defines functions, it does not do anything on its own:
#
#     . "$PSScriptRoot/uia.ps1"          # from clients/windows/
#     $row = Get-MailRows | Select-Object -First 1
#     Invoke-UiaElement $row             # opens it (raises ItemClick, see below)
#     if (Wait-UiaElement -Name 'Undo' -Type Button) { 'the undo bar is up' }
#
# `control.ps1 ui-dump` is built on this, and so is any verification script.
#
# ---------------------------------------------------------------------------------------------
# WHY THIS FILE EXISTS. Raw System.Windows.Automation against WinUI has traps, and THREE OF THEM
# PRODUCE FALSE PASSES, a green assertion for a thing that is not on screen. That is worse than a
# crash, because you believe it. Each one below cost real debugging time; every one is neutralized
# here, so use these functions rather than rolling your own.
#
#   1. FindFirst/FindAll with TreeScope.Descendants SILENTLY UNDER-WALKS the tree. It returned 90
#      elements and missed an entire open composer. Get-UiaTree does a recursive CHILDREN walk,
#      which sees everything. (Hit twice, in two different sessions. It looks like the feature is
#      broken, not the query.)
#   2. Matching on Name alone also matches the TextBlock INSIDE a control, it carries the same
#      Name and supports no patterns. `Find-UiaElement -Name Archive` once returned the reading
#      pane's label and reported a green PASS for a context-menu item that did not exist. So ALWAYS
#      pass -Type for anything you intend to press: Find-UiaElement -Name Archive -Type MenuItem.
#   3. A bare ListItem sweep also matches the NavigationView SIDEBAR entries (All Inboxes, each
#      account, "Add account…"). Selecting one opens the account-setup form and hides the shell,
#      which looks exactly like "the surface under test closed". Use Get-MailRows, which scopes to
#      the message list (#RowsList).
#   4. The mail list opens a row on ItemClick, which SelectionItem.Select() does NOT raise. Rows do
#      support InvokePattern; Invoke-UiaElement prefers it.
#
# WHAT UIA REACHES THAT YOU MIGHT NOT EXPECT: a WinUI TextBlock exposes TextPattern, so the FONT
# WEIGHT the user is actually looking at is readable, Get-UiaFontWeight returns 400/600/700 off the
# rendered text. "Is this row bold?" is therefore an assertion, not a screenshot someone squints at,
# and it catches a binding that was never wired as well as one bound to the wrong property. The
# suites in uitests/ are built on it.
#
# WHAT UIA CANNOT REACH, do not burn time trying:
#   * Gestures. UIA drives controls, not fingers, but real touch CAN be injected at the
#     pointer-device level: use touch.ps1 (Invoke-TouchFlick / Invoke-TouchPinch), or the
#     MAILCAL_SWIPE launch hook (control.ps1 swipe <action>) to skip past a gesture you are not
#     testing. (This header used to say a gesture "cannot be synthesized", corrected 2026-07-13;
#     see the verify-windows-ui skill §4.)
#   * Anything needing the mouse FROM THIS SCRIPT. UIA drives patterns, not pointers, and the
#     cursor-moving approaches tried here all addressed the cursor in 96-DPI space while UIA
#     reports physical pixels, so on a scaled display they click somewhere else entirely.
#   * A BARE LAYOUT PANEL. A Grid or StackPanel holding only other controls gets no automation peer,
#     so it is not in the tree and an AutomationProperties.AutomationId on it reaches nothing, a
#     wait for that id can only time out, however long you give it. Measure the row through a
#     CONTROL inside it (a ScrollViewer, a TextBlock), whose x:Name is already its AutomationId.
#
# CONTEXT FLYOUTS ARE REACHABLE, with the right tool (corrected 2026-07-31). This header used to
# say they were not: "neither a synthetic right-click nor the Apps key opens one; the row's
# ContextFlyout hangs off an inner Grid, so ContextRequested from the focused ListViewItem never
# reaches it." The diagnosis was wrong. Microsoft's `winapp` CLI (winget Microsoft.WinAppCLI, and
# the win-dev-skills plugin) opens the mail row's flyout first try:
#
#     winapp ui click <row-slug> -a <PID> --right     # -> PopupHost window + "Archive conversation"
#
# It works because it is DPI-aware and clicks the real physical point; nothing about WinUI or the
# inner Grid was ever the obstacle. Discover the row slug with `winapp ui inspect RowsList -a <PID>`.
# Two things to know before leaning on it: the slugs are per-run hashes, so they are for a scripted
# discover-then-click, never a hardcoded selector; and `winapp ui inspect --json` nests its tree
# under windows[].elements[], code that reads `.elements` at the ROOT silently gets $null, which
# is a green assertion over nothing (Microsoft's own skill template has this bug).
#
# WHAT `winapp ui` CANNOT DO, so nobody re-runs this evaluation (measured 2026-08-25, winapp 0.6.1,
# against this app). Reach for it for the physical-input jobs above; do NOT port assertions onto it.
#   * NO FONT WEIGHT. `get-property -p FontWeight` returns null on the element whose TextPattern
#     answers 600 here. That is the assertion this whole suite was built for.
#   * NO SCOPED OR TYPED QUERY. There is no --root and no --type, so a selector matches the whole
#     window by name, traps 2 and 3 above, unmitigated. Asking for the subject Text of a mail row
#     returns the ROW: bounds 506,184,905,94 where the text is 708,196,573,38. A geometry assertion
#     written that way measures the wrong element and PASSES.
#   * EVERY VERB IS A PROCESS. 456ms per attached call (161ms of that is process start alone),
#     against 5-7ms for the in-process equivalent. There is no batch or session mode, so a suite
#     of a few hundred primitives would spend minutes in process startup.
#
# THE SYSTEM SAVE/OPEN PICKER (FileSavePicker) IS REACHABLE, BUT ONLY BY KEYBOARD (2026-07-16):
#   the Save As dialog is a CHILD window (class '#32770') of the app window, never top-level, so
#   don't poll RootElement for it: (Get-MailcalWindow).FindFirst(Children, ClassName '#32770').
#   Its DirectUI controls expose a degenerate tree, the filename box (#1001 under
#   #FileNameControlHost) and the Save button (#1) are patternless Panes (no ValuePattern, no
#   InvokePattern; SetFocus throws). What works: SetForegroundWindow on the dialog's
#   NativeWindowHandle, then keystrokes, the filename box has default focus, so SendKeys '^a',
#   the full target path, '{ENTER}'. Assert on the written file's BYTES afterwards, not the dialog.
#
# CHOOSING A DATASET, this decides whether your test proves anything:
#   * showcase (MAILCAL_SHOWCASE=en, via showcase.ps1), TWO accounts, in-memory, deterministic.
#     The only way to exercise multi-account UI. But its engine DOES NOT REALLY PERFORM MAIL
#     ACTIONS, so a committed archive/delete there proves only that you dispatched into a void.
#   * harness (MAILCAL_DEV_ACCOUNT=stalwart), ONE account, but a REAL transport. The only way to
#     prove a destructive action or a send actually landed. Needs `scripts/dev/harness.sh up`.
# ---------------------------------------------------------------------------------------------

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes -ErrorAction Stop

# Guarded because this file is dot-sourced: adding the same type twice in one session throws.
if (-not ('Allodia.UiaDpi' -as [type])) {
  Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;

namespace Allodia
{
    /// <summary>Per-window DPI, so a mixed-DPI desktop answers for the window under test.</summary>
    public static class UiaDpi
    {
        [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);
    }
}
'@
}

Set-Variable -Name UiaElement -Value ([System.Windows.Automation.AutomationElement]) -Scope Script
Set-Variable -Name UiaChildren -Value ([System.Windows.Automation.TreeScope]::Children) -Scope Script
Set-Variable -Name UiaAny -Value ([System.Windows.Automation.Condition]::TrueCondition) -Scope Script

<#
.SYNOPSIS
The running client's main window as an AutomationElement, or $null when it isn't up.
#>
function Get-MailcalWindow {
  $proc = Get-Process Mailcal -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
  if (-not $proc) { return $null }
  $script:UiaElement::FromHandle($proc.MainWindowHandle)
}

<#
.SYNOPSIS
The app window's display scale: 1.0 at 100%, 2.0 at 200%.
#>
function Get-UiaScale {
  $window = Get-MailcalWindow
  if (-not $window) { throw 'no Mailcal window, launch the app first (build-and-run.ps1 / control.ps1)' }
  $dpi = [Allodia.UiaDpi]::GetDpiForWindow([IntPtr] $window.Current.NativeWindowHandle)
  if ($dpi -eq 0) { throw 'the window reports no DPI, so a XAML size cannot be compared against a measured one' }
  $dpi / 96.0
}

<#
.SYNOPSIS
$Dip, a size as written in XAML, in the physical pixels BoundingRectangle reports.
.DESCRIPTION
Convert before comparing a measured rectangle against ANY constant taken from XAML. The two units
agree only at 100%, so the naive comparison is a test that passes on the author's monitor and fails
on the next person's, and the direction that does not fail is worse: a floor assertion on a scaled
display silently accepts half of what it names. Every other suite here compares one measured edge
against another, which needs none of this.
#>
function ConvertTo-UiaPixels {
  param([Parameter(Mandatory)] [double] $Dip)
  $Dip * (Get-UiaScale)
}

<#
.SYNOPSIS
The rendered bounds of $Element, or throws saying it is present but not on screen.
.DESCRIPTION
Read a rectangle through this, never straight off BoundingRectangle. A collapsed WinUI element
stays in the tree with an empty rectangle, and one scrolled off the surface reports infinities,
so a bare comparison is handed two infinities and reports a confident PASS about something nobody
can see. $What names the element in the failure, which is read by someone who did not write the test.
#>
function Get-RenderedBounds {
  # $Element is deliberately NOT Mandatory: callers pass a Find-UiaElement straight in, and a
  # mandatory parameter rejects the $null it returns before the branch below can say what is
  # actually wrong, leaving "Cannot bind argument ... because it is null" where the name of the
  # missing element should be.
  param([object] $Element, [Parameter(Mandatory)] [string] $What)
  if (-not $Element) { throw "$What is not in the automation tree at all" }
  $rect = $Element.Current.BoundingRectangle
  if ([double]::IsInfinity($rect.X) -or $rect.Width -le 0 -or $rect.Height -le 0) {
    throw "$What is in the tree but not rendered (bounds $rect), a collapsed element cannot be measured"
  }
  $rect
}

<#
.SYNOPSIS
Every element under $Root (default: the main window), depth-first.
.DESCRIPTION
A recursive CHILDREN walk, deliberately, NOT FindAll(TreeScope.Descendants), which under-walks
WinUI's tree and will quietly hide whole surfaces from you (see the header).
#>
function Get-UiaTree {
  param([object] $Root)
  if (-not $Root) { $Root = Get-MailcalWindow }
  if (-not $Root) { throw 'no Mailcal window, launch the app first (build-and-run.ps1 / control.ps1)' }
  # NB: PowerShell variables are case-INSENSITIVE, so a local $children must not collide with a
  # script-scope $Children. Names here are deliberately distinct.
  $Root
  $kids = $Root.FindAll($script:UiaChildren, $script:UiaAny)
  for ($i = 0; $i -lt $kids.Count; $i++) { Get-UiaTree -Root $kids[$i] }
}

<#
.SYNOPSIS
Find elements by name / automation id / control type.
.PARAMETER Type
A ControlType NAME ('Button', 'MenuItem', 'ListItem', 'Edit', 'Text', …). ALWAYS pass this for
anything you intend to press, matching on -Name alone also returns the inert TextBlock inside the
control, which is how a false PASS happens (see the header).
#>
function Find-UiaElements {
  param(
    [string] $Name,
    [string] $AutomationId,
    [string] $Type,
    [object] $Root
  )
  $wanted = $null
  if ($Type) {
    $field = [System.Windows.Automation.ControlType].GetField($Type, 'Public,Static')
    if (-not $field) { throw "unknown ControlType '$Type' (try Button, MenuItem, ListItem, Edit, ComboBox, Text)" }
    $wanted = $field.GetValue($null)
  }
  Get-UiaTree -Root $Root | Where-Object {
    ($(-not $Name) -or $_.Current.Name -eq $Name) -and
    ($(-not $AutomationId) -or $_.Current.AutomationId -eq $AutomationId) -and
    ($(-not $wanted) -or $_.Current.ControlType -eq $wanted)
  }
}

<#
.SYNOPSIS
The first element matching the filter, or $null.
#>
function Find-UiaElement {
  param([string] $Name, [string] $AutomationId, [string] $Type, [object] $Root)
  Find-UiaElements -Name $Name -AutomationId $AutomationId -Type $Type -Root $Root | Select-Object -First 1
}

<#
.SYNOPSIS
The MESSAGE rows only, scoped to the message list, not every ListItem in the window.
.DESCRIPTION
A bare ListItem sweep also matches the NavigationView sidebar entries; selecting one of those opens
the account-setup form and hides the whole shell, which reads as "the surface under test closed".
#>
function Get-MailRows {
  $list = Find-UiaElement -AutomationId 'RowsList'
  if (-not $list) { throw 'the message list (#RowsList) is not on screen' }
  Find-UiaElements -Type 'ListItem' -Root $list
}

<#
.SYNOPSIS
The Settings dialog if it is already open, else $null. Never opens it.
.DESCRIPTION
The passive half of Get-SettingsDialog, for a caller that wants to ASSERT on whether Settings is
up rather than get it up. Both live here, and neither is redefined in a suite: the runner
dot-sources every *.Tests.ps1 into ONE scope in alphabetical order, so a helper defined in two
suites silently resolves to whichever sorted last, a suite that opened Settings for itself would
hand the passive version to every suite after it, which then reads "Settings has no About
category" against the whole window.
#>
function Find-SettingsDialog {
  Get-UiaTree -Root (Get-MailcalWindow) |
    Where-Object { $_.Current.ClassName -eq 'Popup' -and $_.Current.Name -eq 'Settings' } |
    Select-Object -First 1
}

<#
.SYNOPSIS
The Settings dialog's UIA root, opening it from the sidebar gear first if it is not already up.
.DESCRIPTION
Scoped to the dialog rather than to the window: the categories share names with the sidebar's own
destinations (the duplicate-name trap above), and a bare ListItem sweep also returns the message
rows. Search under this root and neither can be mistaken for the dialog.
#>
function Get-SettingsDialog {
  $dialog = Find-SettingsDialog
  if ($dialog) { return $dialog }

  # The NavigationView gear exposes SelectionItem and NOT Invoke, so Invoke-UiaElement reaches it
  # only through its fallback. Going through the helper keeps that ordering in one place.
  $gear = Find-UiaElement -AutomationId 'SettingsItem'
  if (-not $gear) { throw 'the sidebar has no Settings gear (#SettingsItem)' }
  Invoke-UiaElement $gear -SettleMs 2000

  $dialog = Find-SettingsDialog
  if (-not $dialog) { throw 'pressing the Settings gear opened no Settings dialog' }
  $dialog
}

<#
.SYNOPSIS
Opens one Settings category by name and returns the dialog root its panel is under.
.DESCRIPTION
Selects rather than Invokes: the category source-list switches on SELECTION, and Invoke is a no-op
for a ListViewItem, a script that only Invokes leaves the detail pane on General and then asserts
against the wrong panel.
#>
function Open-SettingsCategory {
  param([Parameter(Mandatory)] [string] $Name)
  $dialog = Get-SettingsDialog
  $item = Find-UiaElements -Name $Name -Type 'ListItem' -Root $dialog | Select-Object -First 1
  if (-not $item) {
    $have = (Find-UiaElements -Type 'ListItem' -Root $dialog | ForEach-Object { $_.Current.Name }) -join ' | '
    throw "Settings has no '$Name' category. It holds: $have"
  }
  $item.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
  Wait-UiaQuiet -CapMs 1200
  Get-SettingsDialog
}

<#
.SYNOPSIS
Wait for the window to stop changing shape, or until $CapMs is spent.
.DESCRIPTION
The settle after an action used to be a flat sleep of its whole budget, on every call. That is the
thing this file tells everyone else not to do, and it was the single biggest line item in the suite's
running time, the invoke primitive alone is called from forty-odd places, several of them in loops.

Quiescence is two consecutive walks agreeing on the shape of the tree. A walk costs real time, so
this can never be much cheaper than two of them, but two walks is still a fraction of the budget a
flat sleep spends unconditionally, and it CANNOT be slower, because the cap is the old sleep.

$FloorMs is not padding. A commit that redraws the list arrives after a round trip that has not
started when the click returns, so the first two walks can agree simply because nothing has happened
yet. The floor is the minimum time this waits before it is willing to believe "settled".
#>
function Wait-UiaQuiet {
  param([int] $CapMs = 1200, [int] $FloorMs = 300, [int] $PollMs = 120)
  $timer = [Diagnostics.Stopwatch]::StartNew()
  $previous = $null
  while ($timer.Elapsed.TotalMilliseconds -lt $CapMs) {
    $shape = $null
    try {
      $shape = (Get-UiaTree | ForEach-Object {
          "$($_.Current.ControlType.ProgrammaticName)/$($_.Current.AutomationId)/$($_.Current.Name)"
        }) -join "`n"
    }
    catch { }   # mid-transition the tree can be torn down under us; that is a "not yet"
    if ($shape -and $shape -eq $previous -and $timer.Elapsed.TotalMilliseconds -ge $FloorMs) { return }
    $previous = $shape
    Start-Sleep -Milliseconds $PollMs
  }
}

<#
.SYNOPSIS
Activate an element: press a button/menu item, or open a message row.
.DESCRIPTION
Prefers InvokePattern and falls back to SelectionItem. This ordering matters: the mail list opens a
row on ItemClick, which SelectionItem.Select() does NOT raise, a row selected that way just
highlights, and the message never opens. Rows do support Invoke.

$SettleMs is a CAP on the wait afterwards, not the length of it, see Wait-UiaQuiet.
#>
function Invoke-UiaElement {
  param(
    [Parameter(Mandatory)] [object] $Element,
    [int] $SettleMs = 1200
  )
  $patterns = $Element.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName }
  if ($patterns -match 'InvokePattern') {
    $Element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
  } elseif ($patterns -match 'SelectionItemPattern') {
    $Element.GetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern).Select()
  } else {
    throw "'$($Element.Current.Name)' supports neither Invoke nor SelectionItem, is it a label rather than a control? (see uia.ps1 trap 2)"
  }
  Wait-UiaQuiet -CapMs $SettleMs
}

<#
.SYNOPSIS
Set a ToggleSwitch / CheckBox to a known state (the consent switch, the settings switches).
.DESCRIPTION
A ToggleSwitch supports NEITHER Invoke NOR SelectionItem, so Invoke-UiaElement throws on one, it
exposes TogglePattern instead, whose Toggle() *cycles* rather than sets. Cycling is the trap: a
script that toggles blind flips a switch that was already on, and "I opted in" silently becomes "I
opted out". So read ToggleState first and act only on a mismatch, which also makes this idempotent.
.EXAMPLE
Set-UiaToggle (Find-UiaElement -AutomationId 'ShareStats') -On
#>
function Set-UiaToggle {
  param(
    [Parameter(Mandatory)] [object] $Element,
    [switch] $On,
    [int] $SettleMs = 600
  )
  $pattern = $Element.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
  if (-not $pattern) {
    throw "'$($Element.Current.Name)' has no TogglePattern, it is not a ToggleSwitch/CheckBox."
  }
  $want = if ($On) { [System.Windows.Automation.ToggleState]::On } else { [System.Windows.Automation.ToggleState]::Off }
  if ($pattern.Current.ToggleState -ne $want) {
    $pattern.Toggle()
    Start-Sleep -Milliseconds $SettleMs
  }
  if ($pattern.Current.ToggleState -ne $want) {
    throw "'$($Element.Current.Name)' would not go to $want (it is $($pattern.Current.ToggleState))."
  }
}

<#
.SYNOPSIS
Read a ToggleSwitch / CheckBox state as a bool.
#>
function Get-UiaToggle {
  param([Parameter(Mandatory)] [object] $Element)
  $pattern = $Element.GetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern)
  return $pattern.Current.ToggleState -eq [System.Windows.Automation.ToggleState]::On
}

<#
.SYNOPSIS
Type into a text box (raises TextChanged, so the app sees it as a user edit).
#>
function Set-UiaText {
  param(
    [Parameter(Mandatory)] [object] $Element,
    [Parameter(Mandatory)] [AllowEmptyString()] [string] $Text,
    [int] $SettleMs = 600
  )
  $Element.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).SetValue($Text)
  Start-Sleep -Milliseconds $SettleMs
}

<#
.SYNOPSIS
Read a text box's current value.
#>
function Get-UiaText {
  param([Parameter(Mandatory)] [object] $Element)
  $Element.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern).Current.Value
}

<#
.SYNOPSIS
The FontWeight a Text element is actually drawn with: 400 = Normal, 600 = SemiBold, 700 = Bold.
.DESCRIPTION
A WinUI TextBlock exposes TextPattern, so the weight the user sees is readable, which means "is
this row bold?" is an assertion and not a screenshot someone has to squint at. The value comes from
the RENDERED text, so it catches a binding that was never wired as well as one bound to the wrong
property.

Two things to know before trusting it:

  * Ask for the element you mean. Matching on -Name alone also returns the inert TextBlock inside a
    control (uia.ps1 trap 2), and a row's subject and sender are two Text elements in one subtree,
    scope with -Root <row> and pass -Type Text, or you will read the weight of a different string.
  * A MIXED range returns TextPattern.MixedAttributeValue, not a number. That happens when one
    TextBlock draws more than one weight (Android bolds a sender SPAN inside a longer line for
    exactly this reason). Windows keeps subject and sender in separate TextBlocks today, so this
    returns a number, but assert on it rather than assuming, because a silent Mixed compared
    against 600 is a FALSE FAIL that looks like the feature broke.

Returns $null when the element exposes no TextPattern at all.
#>
function Get-UiaFontWeight {
  param([Parameter(Mandatory)] [object] $Element)
  $pattern = $null
  if (-not $Element.TryGetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern, [ref] $pattern)) {
    return $null
  }
  $value = $pattern.DocumentRange.GetAttributeValue([System.Windows.Automation.TextPattern]::FontWeightAttribute)
  if ($value -eq [System.Windows.Automation.TextPattern]::MixedAttributeValue) {
    throw "'$($Element.Current.Name)' draws more than one font weight, so there is no single value to compare, assert on the spans instead"
  }
  [int] $value
}

<#
.SYNOPSIS
Poll until an element appears; returns it, or $null on timeout.
.DESCRIPTION
Prefer this to a fixed Start-Sleep. The app syncs over the wire against the harness, so "how long
until the rows arrive" is not a constant, and a transient state you are trying to catch (the undo
bar, which closes after ~4s) is easy to sleep straight past.
#>
function Wait-UiaElement {
  param(
    [string] $Name,
    [string] $AutomationId,
    [string] $Type,
    [int] $TimeoutSec = 30,
    [int] $PollMs = 250
  )
  $deadline = $TimeoutSec * 1000 / $PollMs
  for ($i = 0; $i -lt $deadline; $i++) {
    $found = $null
    # The window may not be up yet, which Get-UiaTree throws on, that is a "not yet", not a failure.
    try { $found = Find-UiaElement -Name $Name -AutomationId $AutomationId -Type $Type } catch { }
    if ($found) { return $found }
    Start-Sleep -Milliseconds $PollMs
  }
  return $null
}

<#
.SYNOPSIS
Poll until an element is GONE. Returns $true if it went, $false on timeout.
#>
function Wait-UiaGone {
  param([string] $Name, [string] $AutomationId, [string] $Type, [int] $TimeoutSec = 30, [int] $PollMs = 250)
  $deadline = $TimeoutSec * 1000 / $PollMs
  for ($i = 0; $i -lt $deadline; $i++) {
    $found = $null
    try { $found = Find-UiaElement -Name $Name -AutomationId $AutomationId -Type $Type } catch { }
    if (-not $found) { return $true }
    Start-Sleep -Milliseconds $PollMs
  }
  return $false
}

<#
.SYNOPSIS
Wait until the message list settles on a stable, non-zero row count, and return it.
.DESCRIPTION
Against the harness the rows arrive over the wire in several snapshots, so "count the rows after
N seconds" is a race. This waits for the count to hold still.
#>
function Wait-MailRowCount {
  param([int] $TimeoutSec = 40, [int] $StableChecks = 4)
  $stable = 0
  $last = -1
  for ($i = 0; $i -lt $TimeoutSec * 2; $i++) {
    Start-Sleep -Milliseconds 500
    $n = -1
    try { $n = @(Get-MailRows).Count } catch { }
    if ($n -gt 0 -and $n -eq $last) {
      $stable++
      if ($stable -ge $StableChecks) { return $n }
    } else {
      $stable = 0
    }
    $last = $n
  }
  return $last
}

<#
.SYNOPSIS
Print the live automation tree (control type / name / #automationId), indented. Discovery tool.
#>
function Show-UiaTree {
  param([object] $Root)
  $window = if ($Root) { $Root } else { Get-MailcalWindow }
  if (-not $window) { throw 'no running Mailcal window, launch it first (build-and-run.ps1)' }
  Write-UiaNode -Element $window -Depth 0
}

function Write-UiaNode {
  param([object] $Element, [int] $Depth)
  $type = $Element.Current.ControlType.ProgrammaticName -replace '^ControlType\.', ''
  $line = ('  ' * $Depth) + $type
  if ($Element.Current.Name) { $line += " '$($Element.Current.Name)'" }
  if ($Element.Current.AutomationId) { $line += " #$($Element.Current.AutomationId)" }
  Write-Output $line
  $kids = $Element.FindAll($script:UiaChildren, $script:UiaAny)
  for ($i = 0; $i -lt $kids.Count; $i++) { Write-UiaNode -Element $kids[$i] -Depth ($Depth + 1) }
}
