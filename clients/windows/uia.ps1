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
# FINDING HERE, PRESSING THROUGH winapp.ps1. This file finds and reads; it never moves the pointer
# or types. Every click, double-click, right-click, hover, drag, wheel and keystroke goes through
# winapp.ps1 (Invoke-WinappClick, Send-WinappKeys, …), which hands the element found here to
# Microsoft's `winapp` CLI. That is how a context flyout opens, how a canvas peer with no Invoke
# pattern is pressed, and how a tooltip is raised, at any display scale.
#
# Why reading does not move to winapp as well (measured on winapp 0.7.1 against this app, which now
# does what 0.6.1 could not: --root and --type scoping, FontWeight through get-property, explicit
# --action, the window's DPI in `status`):
#   * EVERY VERB IS A PROCESS: 140 ms to start, 0.2 to 0.7 s a call, where an in-process read takes
#     milliseconds. A suite reads the tree hundreds of times; it presses a handful.
#   * A TEXT SELECTOR IS A SUBSTRING, and --class-name only narrows a text match, so "the element of
#     class CalendarSurface" cannot be asked at all.
#   * WHILE A COMPOSER IS OPEN, winapp's walk of the window never ends (winapp.ps1 says why), and
#     eleven of the 49 suites open one.
# Batched in-process use is upstream work (microsoft/winappCli#824); revisit this when it ships.
#
# WHAT NOTHING HERE REACHES, do not burn time trying:
#   * A MULTI-TOUCH GESTURE. touch.ps1 injects real contacts, a diagonal pinch included, which no
#     touchpad and no winapp verb produces; the MAILCAL_SWIPE launch hook (control.ps1 swipe
#     <action>) skips past a swipe you are not testing.
#   * A BARE LAYOUT PANEL. A Grid, StackPanel or BORDER holding only other controls gets no
#     automation peer, so it is not in the tree and an AutomationProperties.AutomationId on it
#     reaches nothing, a wait for that id can only time out, however long you give it. Measure the
#     row through a CONTROL inside it (a ScrollViewer, a TextBlock), whose x:Name is already its
#     AutomationId.
#     ⚠️ The cost is invisible, which is why this is worth re-reading: #SelectionBar is on a Border,
#     and SelectionBar.Tests waited thirty seconds for it, twice a run, then DISCARDED the $null
#     and passed on elements it found by other means. A whole minute of every run, and nothing on
#     screen or in the summary said so. If a wait's result is not read, it is not a wait.
#   * THE COMPOSER'S MESSAGE BODY. It is a web page, one group named "Write your message" here;
#     composer.ps1 reads and sets it over DevTools.
#
# THE SYSTEM SAVE/OPEN PICKER (FileSavePicker) IS ANSWERED BY KEYBOARD: the Save As dialog is a
#   CHILD window (class '#32770') of the app window, never top-level, so don't poll RootElement for
#   it: (Get-MailcalWindow).FindFirst(Children, ClassName '#32770'). Its DirectUI controls expose a
#   degenerate tree, the filename box (#1001 under #FileNameControlHost) and the Save button (#1)
#   are patternless Panes (no ValuePattern, no InvokePattern; SetFocus throws). The filename box has
#   default focus, so Ctrl+A, the path and Enter save (Save-ThroughFilePicker). Assert on the
#   written file's BYTES afterwards, not the dialog.
#
# CHOOSING A DATASET, this decides whether your test proves anything:
#   * showcase (MAILCAL_SHOWCASE=en, via showcase.ps1), TWO accounts, in-memory, deterministic.
#     The only way to exercise multi-account UI. But its engine DOES NOT REALLY PERFORM MAIL
#     ACTIONS, so a committed archive/delete there proves only that you dispatched into a void.
#   * harness (MAILCAL_DEV_ACCOUNT=stalwart), ONE account, but a REAL transport. The only way to
#     prove a destructive action or a send actually landed. Needs `scripts/dev/harness.sh up`.
# ---------------------------------------------------------------------------------------------

Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes -ErrorAction Stop

# Pressing, as against finding: every click, hover, drag, wheel and keystroke goes through winapp.
. (Join-Path $PSScriptRoot 'winapp.ps1')

$uiaDpiSource = @'
using System;
using System.Runtime.InteropServices;
using System.Text;

namespace Allodia
{
    /// <summary>Per-window DPI, so a mixed-DPI desktop answers for the window under test.</summary>
    public static class UiaDpi
    {
        [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr hwnd);

        /// <summary>The primary display's DPI, for a question asked before there is a window.</summary>
        [DllImport("user32.dll")] public static extern uint GetDpiForSystem();

        /// <summary>
        /// Bring a window to the front before pressing or typing at it: real input goes to whatever
        /// is in front (Show-WinappWindow, and Search.Tests' timed keystroke).
        /// </summary>
        [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hwnd);

        /// <summary>The window in front, to bring another forward only when it is not.</summary>
        [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();

        /// <summary>
        /// A keystroke, which makes this process the last to send input:
        /// Windows grants SetForegroundWindow only then (Show-WinappWindow).
        /// </summary>
        [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);

        /// <summary>Whether a native window still exists, for a browser window that closes after the
        /// control that owned it (Close-Composer).</summary>
        [DllImport("user32.dll")] public static extern bool IsWindow(IntPtr hwnd);

        private delegate bool ChildProc(IntPtr hwnd, IntPtr state);

        [DllImport("user32.dll")] private static extern bool EnumChildWindows(IntPtr parent, ChildProc proc, IntPtr state);

        [DllImport("user32.dll", CharSet = CharSet.Unicode)] private static extern int GetClassName(IntPtr hwnd, StringBuilder name, int capacity);

        /// <summary>
        /// The native child windows of <paramref name="parent"/>, shown or hidden, whose class name
        /// starts with <paramref name="prefix"/> (Get-WebViewHostCount).
        /// </summary>
        public static int CountChildWindows(IntPtr parent, string prefix)
        {
            var count = 0;
            var name = new StringBuilder(256);
            EnumChildWindows(parent, (hwnd, _) =>
            {
                name.Clear();
                GetClassName(hwnd, name, name.Capacity);
                if (name.ToString().StartsWith(prefix, StringComparison.Ordinal)) { count++; }
                return true;
            }, IntPtr.Zero);
            return count;
        }
    }
}
'@
# Guarded because this file is dot-sourced: adding the same type twice in one session throws. A
# type cannot be replaced once loaded either, so a session that dot-sourced an older copy of this
# file keeps that copy's type, and a member added since would fail at its first call as "method not
# found". It is named here instead.
$uiaDpiType = 'Allodia.UiaDpi' -as [type]
if (-not $uiaDpiType) {
  Add-Type -TypeDefinition $uiaDpiSource
} else {
  $missing = @([regex]::Matches($uiaDpiSource, 'public static (?:extern )?\w+ (\w+)\(') |
      ForEach-Object { $_.Groups[1].Value } | Where-Object { -not $uiaDpiType.GetMethod($_) })
  if ($missing) {
    throw "this PowerShell session loaded an older uia.ps1, whose Allodia.UiaDpi has no $($missing -join ', '); start a new session"
  }
}

Set-Variable -Name UiaElement -Value ([System.Windows.Automation.AutomationElement]) -Scope Script
Set-Variable -Name UiaChildren -Value ([System.Windows.Automation.TreeScope]::Children) -Scope Script
Set-Variable -Name UiaAny -Value ([System.Windows.Automation.Condition]::TrueCondition) -Scope Script

<#
.SYNOPSIS
The running client's main window as an AutomationElement, or $null when it isn't up.
#>
function Get-MailcalWindow {
  $proc = Get-Process Mailcal -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $proc) { return $null }
  # THE MAILBOX, BY TITLE, and not Process.MainWindowHandle. That property is whichever top-level
  # window Windows hands back, and this app opens message and composer windows beside the mailbox
  # now, so it can be one of those: a suite then searched a reading window for the mailbox's own
  # controls and reported that the message list was not on screen. The mailbox is the window whose
  # title is the app's name; a message window carries its subject.
  # Filtered here rather than with an AndCondition, which New-Object cannot bind to its params
  # constructor and which silently finds nothing.
  # ⚠️ Compared with the ampersand taken out of BOTH sides. UI Automation reports this window's
  # Name as "Allodia Mail  Calendar": the & is an accelerator marker to the framework and it is
  # dropped from the automation name, while GetWindowTextW keeps it. Comparing the two directly
  # never matches, and the fallback below then answers with whichever window Windows feels like.
  $title = (Get-BrandAppTitle) -replace '&', ''
  $tops = $script:UiaElement::RootElement.FindAll($script:UiaChildren, $script:UiaAny)
  for ($i = 0; $i -lt $tops.Count; $i++) {
    $top = $tops[$i]
    if ($top.Current.ProcessId -eq $proc.Id -and ($top.Current.Name -replace '&', '') -eq $title) {
      return $top
    }
  }
  # Before the mailbox has a title, at the very start of a launch.
  $started = Get-Process Mailcal -ErrorAction SilentlyContinue |
    Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
  if ($started) { $script:UiaElement::FromHandle($started.MainWindowHandle) }
}

<#
.SYNOPSIS
How many WebView2 hosts the mailbox window holds, shown or hidden: one for each message body and
composer editor it has, and one for each still closing.
.DESCRIPTION
Read off the window's native children, the browser windows the hosts draw in. A composer's editor
leaves the automation tree as soon as the composer closes it, while the browser window it drew in
goes only once the WebView2 runtime has shut it down, so only this sees one still closing. A
winapp search of the window can walk into an editor and not come back (winapp.ps1), so a composer
is closed when this count is back to what it was before it opened.
#>
function Get-WebViewHostCount {
  param([object] $Window = (Get-MailcalWindow))
  if (-not $Window) { return 0 }
  [Allodia.UiaDpi]::CountChildWindows([IntPtr] [long] $Window.Current.NativeWindowHandle, 'Chrome_WidgetWin_')
}

<#
.SYNOPSIS
Throws the mailbox's open composer away by its Discard, answering "Discard draft?" when it asks,
and waits until it is gone, its editor's browser window included. Returns whether it asked; does
nothing and returns $false with no composer open.
.DESCRIPTION
By AutomationId, never by the label, which is whatever language the app runs in. It throws rather
than returning with the composer still there: a composer left open is closed later by the runner,
and a winapp lookup that runs while its editor is still closing walks into the editor and times out
in a suite that never opened it. The editor is waited for by its own window handle, which the
automation tree stops showing before the window itself goes.
#>
function Close-Composer {
  param([int] $TimeoutSec = 15)
  $discard = Find-UiaElement -AutomationId 'DiscardButton' -Type Button
  if (-not $discard) { return $false }
  $editor = Get-UiaTree | Where-Object { $_.Current.AutomationId -eq 'Editor' } | Select-Object -First 1
  $editorWindow = if ($editor) { [IntPtr] $editor.Current.NativeWindowHandle } else { [IntPtr]::Zero }
  Invoke-UiaElement $discard
  # The question's primary button is Discard (ComposerView.Drafts.cs).
  Wait-UiaQuiet -CapMs 800
  $confirm = Find-UiaElement -AutomationId 'PrimaryButton' -Type Button
  if ($confirm) { Invoke-UiaElement $confirm }
  if (-not (Wait-UiaGone -AutomationId 'SendButton' -TimeoutSec $TimeoutSec)) {
    throw "the composer is still open ${TimeoutSec}s after its Discard was pressed"
  }
  $watch = [Diagnostics.Stopwatch]::StartNew()
  while ($editorWindow -ne [IntPtr]::Zero -and [Allodia.UiaDpi]::IsWindow($editorWindow)) {
    if ($watch.Elapsed.TotalSeconds -ge $TimeoutSec) {
      throw "the composer closed, but its editor's browser window was still there ${TimeoutSec}s later"
    }
    Start-Sleep -Milliseconds 100
  }
  $null -ne $confirm
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
The name THIS build calls itself: "MailCal" unbranded, whatever `branding/allodia.env` says when
that file is present.
.DESCRIPTION
Never write the branded name in a suite. The app's name is injected, not written in a client
(docs/branding.md), so a literal asserts against a build nobody but the brand owner makes: CI is
unbranded, every fork is unbranded, and the caption there reads "MailCal". The failure names the
caption rather than the branding, which is how this cost a CI run to find.

Read out of the generated resource the app itself compiles, `Strings/en/Resources.resw`, rather
than by parsing `branding/*.env` again: the resolution order lives in `scripts/dev/brand.sh` and a
fourth copy of it is a copy that can disagree. `en` because the showcase dataset pins that locale,
and the product name is one string in all seven languages anyway.
#>
function Get-BrandAppTitle {
  $resw = Join-Path $PSScriptRoot 'Mailcal/Strings/en/Resources.resw'
  if (-not (Test-Path -LiteralPath $resw)) {
    throw "no generated $resw; build the client first (build-and-run.ps1 -NoRun), the l10n codegen writes it."
  }
  $value = ([xml] (Get-Content -Raw -LiteralPath $resw)).SelectSingleNode("//data[@name='app_title']/value")
  if (-not $value) { throw "the generated $resw carries no app_title" }
  $value.InnerText
}

<#
.SYNOPSIS
A window's minimise / maximise / close buttons, keyed by automation id.
.DESCRIPTION
The SYSTEM draws these, on a surface that is not in the XAML tree, so they are NOT under the app's
own caption control and a search scoped to it finds none of them. They sit in a pane of the window's
own, which is why $Root is a WINDOW here (default: the mailbox) and never #AppTitleBar.

They are the part of a custom caption that most easily ends up wrong while everything else about it
is right, because nothing but a rendered window says so: a window either asks for them to match the
caption's height or gets 32 epx ones, a third of a row above the icon and the name beside them.
#>
function Get-CaptionButtons {
  param([object] $Root)
  $buttons = [ordered] @{}
  foreach ($id in 'Minimize', 'Maximize', 'Close') {
    $button = Find-UiaElement -AutomationId $id -Type 'Button' -Root $Root
    if (-not $button) {
      throw "the window carries no '$id' caption button. Extending into the caption hides the " +
        'system strip and leaves these three in place; a window that lost them cannot be closed ' +
        'with a pointer at all'
    }
    $buttons[$id] = $button
  }
  $buttons
}

<#
.SYNOPSIS
How tall a caption is in this app, in the logical units XAML is written in.
.DESCRIPTION
Read out of App.xaml, where every window's caption reads it from (Services/WindowCaption.cs), rather
than written here again: a suite carrying its own copy of the number asserts that the app matches
the SUITE, and stays green on the day the two stop agreeing.
#>
function Get-CaptionHeightDip {
  $appXaml = Join-Path $PSScriptRoot 'Mailcal/App.xaml'
  if (-not (Test-Path -LiteralPath $appXaml)) { throw "no $appXaml to read the caption height from" }
  $value = ([xml] (Get-Content -Raw -LiteralPath $appXaml)).SelectSingleNode(
    "//*[local-name()='Double'][@*[local-name()='Key']='AppCaptionHeight']")
  if (-not $value) { throw "$appXaml no longer defines AppCaptionHeight" }
  [double] $value.InnerText
}

<#
.SYNOPSIS
The desktop's size in the LOGICAL units a XAML window is sized in: @(width, height).
.DESCRIPTION
For the one question that has to be answered before the app exists: is this desktop big enough to
hold the window the suites are about to measure? `Screen.PrimaryScreen.Bounds` alone will not do
it. PowerShell 7 is per-monitor DPI aware, so that property reports PHYSICAL pixels, 2880x1920 on
a 200% display; comparing it against a size written in XAML says yes to a desktop half the size
it needs. Dividing by the system scale puts both sides in the same unit.
#>
function Get-DesktopLogicalSize {
  Add-Type -AssemblyName System.Windows.Forms
  $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  $dpi = [Allodia.UiaDpi]::GetDpiForSystem()
  $scale = if ($dpi -gt 0) { $dpi / 96.0 } else { 1.0 }
  @([int] [Math]::Round($bounds.Width / $scale), [int] [Math]::Round($bounds.Height / $scale))
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
Every tooltip the app has on screen right now, as @(element).
.DESCRIPTION
Walked from the main window rather than from the desktop: a WinUI tooltip lives in a popup of its
own (a `PopupHost` site bridge), and that popup hangs off the window in the control view, so one
walk finds both the window's own elements and anything floating over it.
#>
function Get-UiaToolTips {
  @(Get-UiaTree | Where-Object { $_.Current.ControlType -eq [System.Windows.Automation.ControlType]::ToolTip })
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
  if (-not $list) {
    $searched = Get-MailcalWindow
    $where = if ($searched) { $searched.Current.Name } else { '(no window)' }
    throw "the message list (#RowsList) is not on screen; searched [$where]"
  }
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
Opens one account's page in Settings → Accounts, by its position in the list or its address, and
returns the dialog root.
.DESCRIPTION
The list is drawn afresh first, by leaving the category and coming back: that rebuilds it from the
core, so what the page shows afterwards is the core's answer and not a value the client kept. Two
accounts can share an address (a mailbox and its calendar account), which is why -Index exists.
#>
function Open-SettingsAccount {
  param([int] $Index = 0, [string] $Address)
  Open-SettingsCategory -Name 'General' | Out-Null
  $dialog = Open-SettingsCategory -Name 'Accounts'
  $rows = @(Find-UiaElements -Type 'Button' -Root $dialog | Where-Object { $_.Current.Name -match '@' })
  if ($Address) { $rows = @($rows | Where-Object { $_.Current.Name -eq $Address }) }
  if ($rows.Count -le $Index) {
    throw "Settings → Accounts lists $($rows.Count) matching account(s), not $($Index + 1)"
  }
  Invoke-UiaElement $rows[$Index]
  Wait-UiaQuiet -CapMs 1500 -FloorMs 400
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
  # THE HOT PATH OF THE WHOLE SUITE, so that nobody optimises it twice. Each turn reads three
  # properties off every node, and two consecutive turns have to agree, so one settle is at least
  # two full walks. Measured on the showcase list, 130 nodes: 309ms a walk.
  #
  # A UI Automation CacheRequest over the same recursion, which is the obvious fix because it
  # collapses the property reads into the FindAll that fetched the node, gets it to 254ms and
  # returns a byte-identical shape. 18%. The cost is not the property reads: it is one COM
  # FindAll per node plus PowerShell's own recursion, and a cache request changes neither. Not
  # worth a second walk function and a cached/live split across the file. Measured 2026-09-10.
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
The screen pixels of a rectangle (physical pixels) as a Bitmap the caller disposes, read with
-Element's window in front.
.DESCRIPTION
The pixels come off the composited desktop, so they are whatever is drawn there: any window lying
over the app is read as the app's own colours. The window is brought to the front first.
#>
function Get-UiaScreenPixels {
  param(
    [Parameter(Mandatory)] [object] $Element,
    [Parameter(Mandatory)] [int] $X, [Parameter(Mandatory)] [int] $Y,
    [Parameter(Mandatory)] [int] $Width, [Parameter(Mandatory)] [int] $Height
  )
  Show-WinappWindow (Get-UiaWindowHandle $Element)
  Add-Type -AssemblyName System.Drawing
  $bmp = New-Object System.Drawing.Bitmap $Width, $Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  try { $g.CopyFromScreen($X, $Y, 0, 0, $bmp.Size) } finally { $g.Dispose() }
  $bmp
}

<#
.SYNOPSIS
Wait until the message list settles on a stable, non-zero row count, and return it.
.DESCRIPTION
Against the harness the rows arrive over the wire in several snapshots, so "count the rows after
N seconds" is a race. This waits for the count to hold still.

An empty list never settles, and waits out -TimeoutSec, unless -AllowEmpty: then none counts too,
once the list draws its empty panel. That panel is up for any list without rows, a folder whose
first sync has not arrived included, so pass it only for a folder that may rightly be empty, and go
on to wait for whatever is expected in it.
#>
function Wait-MailRowCount {
  param([int] $TimeoutSec = 40, [int] $StableChecks = 4, [switch] $AllowEmpty)
  $stable = 0
  $last = -1
  for ($i = 0; $i -lt $TimeoutSec * 2; $i++) {
    Start-Sleep -Milliseconds 500
    $n = -1
    try { $n = @(Get-MailRows).Count } catch { }
    $settled = $n -gt 0 -or ($AllowEmpty -and $n -eq 0 -and (Find-UiaElement -AutomationId 'EmptyMailboxText'))
    if ($settled -and $n -eq $last) {
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
Answer the system Save As dialog with -Destination, and wait for it to close. Asserts nothing about
the file: read the BYTES afterwards, which is the only evidence the write actually happened.
.DESCRIPTION
A FileSavePicker (the .eml export, Settings -> Diagnostics -> Export log) opens the classic Save As
dialog, and three things about it defeat every obvious approach:

  * It is a CHILD window (class #32770) of the app window, never a top-level one, so polling
    RootElement's children for a new window finds nothing and reads as "the picker never opened".
  * Its controls expose a DEGENERATE tree: the filename box (#1001 under #FileNameControlHost) and
    the Save button (#1) are patternless Panes, with no ValuePattern, no InvokePattern, and
    SetFocus() throws. So there is nothing here to Invoke or Set-UiaText.
  * The keyboard works. The filename box has default focus, so Ctrl+A, the path typed as written,
    and Enter saves.
.EXAMPLE
Save-ThroughFilePicker -Destination (Join-Path $env:TEMP 'export.eml')
#>
function Save-ThroughFilePicker {
  param(
    [Parameter(Mandatory)] [string] $Destination,
    [int] $TimeoutSec = 20,
    # Receives the name the app PRE-FILLED, before it is replaced. The suggested name is the only
    # evidence of what the client asked the core to call the file, and it is gone once Ctrl+A lands.
    [ref] $SuggestedName
  )
  $condition = New-Object System.Windows.Automation.PropertyCondition(
    $script:UiaElement::ClassNameProperty, '#32770')
  $dialog = $null
  for ($i = 0; $i -lt $TimeoutSec * 2; $i++) {
    Start-Sleep -Milliseconds 500
    try {
      $window = Get-MailcalWindow
      if ($window) { $dialog = $window.FindFirst($script:UiaChildren, $condition) }
    } catch { }
    if ($dialog) { break }
  }
  if (-not $dialog) { throw "no Save As dialog within ${TimeoutSec}s; it is a #32770 CHILD of the app window, so this is the picker not opening rather than the search missing it" }
  if ($SuggestedName) {
    # The filename box is #1001, under #FileNameControlHost, and the scoping is not tidiness: the
    # ADDRESS BAND carries a #1001 of its own ("Address: Documents"), and it sorts first in a walk
    # of the whole dialog. An unscoped read hands back the directory as a confident file name.
    $nameHost = Get-UiaTree -Root $dialog | Where-Object { $_.Current.AutomationId -eq 'FileNameControlHost' } | Select-Object -First 1
    $box = if ($nameHost) { Get-UiaTree -Root $nameHost | Where-Object { $_.Current.AutomationId -eq '1001' } | Select-Object -First 1 } else { $null }
    if (-not $box) { throw 'the Save As dialog exposes no #1001 filename box under #FileNameControlHost, so the pre-filled name cannot be read' }
    # Name, not ValuePattern: these controls are patternless Panes (see above), and the pre-filled
    # text is what the Pane reports as its Name.
    $SuggestedName.Value = $box.Current.Name
  }
  $handle = [long] $dialog.Current.NativeWindowHandle
  Send-WinappKeys 'ctrl+a' -Window $handle
  Start-Sleep -Milliseconds 300
  Send-WinappKeys $Destination -Window $handle -Text
  Start-Sleep -Milliseconds 400
  Send-WinappKeys enter -Window $handle
  for ($i = 0; $i -lt $TimeoutSec * 2; $i++) {
    Start-Sleep -Milliseconds 500
    $still = $null
    try {
      $window = Get-MailcalWindow
      if ($window) { $still = $window.FindFirst($script:UiaChildren, $condition) }
    } catch { }
    if (-not $still) { return }
  }
  throw "the Save As dialog was still up ${TimeoutSec}s after ENTER; the path may have been refused"
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
