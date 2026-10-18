# Mailcal.UiTests

The Windows client's UI suites that reach the app through its **test agent**: assertions against
the running app, made from inside it. They answer the questions the PowerShell suites in
[`../uitests`](../uitests) answer, at a fraction of the cost, because finding an element is a call
on the app's UI thread rather than a cross-process walk of the UI Automation tree.

```powershell
scripts/dev/harness.sh up                         # the seeded Stalwart harness
clients/windows/build-and-run.ps1 -NoRun          # a Debug build: the agent is not in a release one
dotnet test clients/windows/Mailcal.UiTests       # launches the app itself, and stops it after
```

[`run-ui-tests.ps1`](../uitests/run-ui-tests.ps1) runs them too, after its own suites and under the
same `-Filter` (`Drafts*` selects `DraftsTests`), `-Dataset` and `-RequireHarness`, so one command
still runs every UI suite before a mail write is pushed.

A case **skips**, naming why, when the harness is down or there is no Debug build. Set
`MAILCAL_UITESTS_REQUIRE_HARNESS=1` and it fails instead, so a host meant to have both cannot pass
by having neither. `MAILCAL_UITESTS_TRACE=1` writes every agent call and its answer, timed, to
`%TEMP%\mailcal-uitests-trace.log`; lined up against the app's own log, it says which step a server
write followed. A failing case leaves a screenshot and the element tree the agent saw in
`failures/`.

## Launches

The app reads its `MAILCAL_*` switches once, at startup, so a launch is a fixture in
[`Suites/HarnessApp.cs`](Suites/HarnessApp.cs): one per set of switches (which accounts over which
transport, a pinned appearance, a server verdict faked in the core) shared by every suite in its
collection, which puts back what each case leaves. A suite that sets an account up through the form
gets a launch of its own on an emptied first-run store, because what a first run asks is asked once
a store; so does one whose case quits the app, as a notification click with the app closed does.

## The agent

[`Mailcal/Services/TestAgent*.cs`](../Mailcal/Services) listens on the named pipe
`MAILCAL_TEST_AGENT` names, one JSON object a line each way, and only in a Debug build:
`cargo xtask check-test-agent` fails any change that would let it reach a release one. It walks
each window's **automation peers**, which are the objects UI Automation reads, so an AutomationId or
an accessible name a suite finds is the one a screen reader is given. It acts through the same
providers UI Automation calls: Invoke, SelectionItem, Value, Toggle, ExpandCollapse, ScrollItem
and Scroll.

Where the peers and UI Automation differ, the agent makes them agree, and each of these is a reason
an obvious reimplementation finds nothing:

- **A layout panel has no peer.** A window's content is usually one, so the walk starts at the
  highest descendants that have a peer.
- **A dialog's AutomationId is on its popup**, and a popup's peer reports no children in-process:
  the walk starts each popup at the popup and descends into its content.
- **A text box's value is not on its peer.** WinUI implements the value pattern of a `TextBox` and a
  `PasswordBox` inside the framework, so the agent reads and sets the box itself, which is what
  UI Automation's value pattern does to it.
- **A text box's placeholder is a text under it.** The agent sees the box's template, so an empty
  box with a placeholder holds one Text element and one with a header holds another; count against
  a box known to have the one being asked about.
- **A WebView2's content is not walked.** It is the browser's own tree, and searching across the
  composer's editor costs UI Automation seconds a call; nothing a suite looks up lives in it.
- **A peer measures from its window's client area.** The agent reports physical screen pixels, the
  unit a press is aimed in.
- **A peer can refuse its children.** The invitation card's Expander, while offscreen, answers
  `E_OUTOFMEMORY` (raised as `OutOfMemoryException`) every time it is asked; the walk passes over
  it, as UI Automation passes over a subtree it cannot read.

## Writing a case

- **Wait for the thing, never for a time.** `Agent.Wait` checks on every turn of the app's UI
  thread; a sleep is a guess that is either too long or occasionally too short. `Agent.Settle` is
  for the one question a condition cannot ask: that something did *not* happen, such as a dialog
  not being raised. Give it the floor the app needs to have raised it.
- **A moment the app knows and the screen does not show** gets a signal:
  `TestAgent.Signal(element, "state")` in the app, `Filter { Signal = "state" }` in the suite. The
  composer's editor signals `seeded` once its page is loaded and filled; typing before then lands
  in a page that is still being set up.
- **A dialog has not closed until its dimming layer has.** A ContentDialog's own popup goes the
  moment it is dismissed, but the layer behind it stays until the close completes, and the app
  opens no other dialog before then: `App.CloseDialogs` waits for both.
- **A row far down the message list has not been built.** The list is virtualised, so a row
  scrolled well out of view is in no tree at all: `App.RowTitled` scrolls down the list to find
  it. And a case that leaves the calendar on screen leaves no list: `App.RowsList` puts the mail
  back.
- **A folder's list is not that folder's until its header names it.** The rows on screen in the
  meantime are the last folder's, and a press meant for the new list lands on one of them.
- **A text box has the keyboard before it can take characters.** It reports focus before Windows'
  text input has attached to it, and keys typed in between are dropped without a trace, usually
  the first two or three. Type with `App.TypeInto`, which waits past the focus and checks the box
  holds what was typed; and press Enter only once it does, or the Enter commits nothing.
- **A pressed element is read once it has stopped moving.** A scroll is animated, so a rectangle
  read mid-scroll aims the press where the element was: `App.Steady` returns it once two reads
  agree. Beneath the fold, a peer still answers but reports no size; scroll first.
- **What the system draws is not in the agent's tree.** The print dialog, a classic Save As and a
  window's caption buttons are read through UI Automation ([`Driver/PrintDialogs.cs`](Driver/PrintDialogs.cs),
  [`Driver/SaveDialogs.cs`](Driver/SaveDialogs.cs), [`Driver/Desktop.cs`](Driver/Desktop.cs)). UI Automation files an owned window under its owner,
  so a search of the desktop's children never meets a dialog the app's own window owns; the window
  list does.
- **Real input only for a press that has to be a press**: a double-click, a click into the
  editor's web page, typing into it or into a field whose own key handling the case asserts. Everything else goes through the agent, which needs no window
  in front and cannot be intercepted by one.
- **Quote an id where it is looked up** (`Filter.ById("SendButton")`): `cargo xtask
  check-automation-ids` reads the quoted ids and fails one the app no longer carries.
- **Cleanup goes to `Case`'s cleanup**, not to a `finally`: a cleanup that fails after the case
  failed would replace the case's own failure with its own.
