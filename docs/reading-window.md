# Reading and composer windows: cross-platform contract

**Scope.** Opening a message, or a draft, in a window of its own on a desktop. It binds every
client that can have more than one window: macOS, Windows and Linux. The phones and the iPad have
one window by construction and are out of scope, and nothing here may change what they do.

**Principle.** A window is a *view of the running app*, never a second instance of it. Everything
a window shows comes from the one core over the one store, and everything a window does goes
through the paths the pane already uses. What a window adds is a place to put a message, not a
second way to read one.

## One process, one store

**A client may hold exactly one core.** The core opens the SQLite store
(`Engine::open(mailcal.sqlite)`), so a second core is a second connection to the same file: two
writers, two caches, and whichever wrote last winning. Every window shares the one model and
through it the one core.

Two consequences, and neither is optional:

- **No "New Window" for the main window.** A toolkit that offers one by default has it removed:
  SwiftUI adds ⌘N to a `WindowGroup`, and that command builds a whole second app. The windows a
  client offers are the two named below.
- **A second launch of the app is not a second app.** Where a platform can be asked to hand a
  second launch to the running process, it is.

## The reading window

**A double-click on a message row opens that message in a window of its own.** The row is a
message: a flat row, or one message inside an expanded conversation. A **conversation header** is
not, and keeps whatever a double-click already did there (on macOS, expand and collapse; on Linux,
its own disclosure).

**One window per message.** Double-clicking a row whose window is already open brings that window
forward rather than opening a second one on the same message.

**It leaves the pane alone.** The pane keeps the message it had and the list keeps its selection:
opening a second reader must not disturb the first, which is the whole point of the window.

Where the toolkit delivers a double-click as its own event, nothing else happens first and the
gesture can never raise the unsent-draft prompt either, because it takes nothing away. Where the
list raises an ordinary click on the first press, the pane is put back instead, and that weaker
form is a **known gap** rather than a different rule: the end state is the same and the prompt is
the part that is lost.

**The window is a full reading view**, and `reading-actions.md` binds it entire: the same header,
recipients, invitation card, attachment bar and body, and the same action row in the same order,
overflow included. Two rules are the window's own:

- **Archive and delete close the window.** The message has left the folder, so there is nothing
  left for the window to be about. The pane advances to the next message down
  (`capabilities.md`) because it is a place in a list; a window is one message.
- **Reply, reply-all and forward open a composer window** (below), never the pane's inline
  composer: a draft that answers the window in front of you does not belong in a different window
  behind it.

**The core keeps one reading slot per reader.** `Intent::OpenMessage` names its reader, so a
window's open is the pane's open into a different slot: same fetch, same bounded retry for an
account still dialing, same threshold before a loading state is announced
(`sync-progress.md`), same mark-read. **No client may fetch a body for a window any other way.**
A second path is a second answer to "when is a message read", and the two will disagree.

**A closed window's body is freed**, on both sides: the host forgets it and tells the core to drop
its slot (`close_reading_window`). A sanitised body carries every inline image resolved into it,
so a session that opened twenty messages would otherwise hold twenty of them for as long as it
ran. Closing is not an `Intent`: it moves nothing, marks nothing and signals nothing.

**A window's reader id is the host's to mint, and cannot name the pane.** The pane's slot is a
different kind of thing in the core, not a reserved string, so no host bug can empty the pane by
closing a window.

## The composer window

**A reply, reply-all or forward raised inside a reading window opens the draft in a window of its
own.** It is the same composer the pane hosts, with the same seeding: the core's derived
`Re:`/`Fwd:` subject, the core's suggested recipients, the quoted original in the user's chosen
style, and a forward's staged attachments (`sending.md`, `signatures.md`,
`composer-security.md`, all of which bind it unchanged).

**Closing the window discards the draft, exactly as Cancel does**, and asks no more than Cancel
does. The two are the same act, a person deliberately abandoning what they were writing, and a
client that questioned one but not the other would be teaching two rules for one thing. The
prompt that does exist, Discard / Keep editing, belongs to something else: it fires when the app
is about to take a draft away that the user did **not** ask it to
(`capabilities.md`, "Composing keeps the mailbox live").

**The window is named after the draft**, so two open drafts are distinguishable in the window list
the OS draws.

**The main window's composer does not move.** Where a client renders the composer inline in place
of the reading pane, it keeps doing so for a reply raised there: that is a shipped capability
(`capabilities.md`) and this contract adds to it rather than replacing it.

## Closing

**Closing the main window closes its reading and composer windows.** They were opened out of the
mailbox, and leaving them behind leaves the app running as a scatter of message windows with no
way back to the list.

**Reopening the app brings the mailbox back on the same core**, without a relaunch: the process
never ended, so no account reconnects and no sync starts again. Only where the platform keeps an
app alive with no windows; where closing the last window ends the process, there is nothing to
reopen and the rule that binds is the one above, that a second launch is not a second app.

**Windows are not restored across launches.** A window is a view onto a message this session
opened; at the next launch its header and its body are both gone, so it would open blank. Where a
platform restores windows by default, that is turned off for these two.

## Staying in front

**A window opens in front of the mailbox and stays there until the reader asks for the mailbox.**
It was asked for; a reader who double-clicks a message and watches the window slide behind the list
has been given nothing. And **the mailbox can always be raised over it**, the moment the reader
clicks it: these are peer windows, as a message window is in every other mail client on the desktop.

Every toolkit here leaves a new top-level window in front on its own. What breaks the rule is an app
that activates its **main** window afterwards, and on Windows this one did: a mailbox activation
measured at 7 to 22 ms after the window appeared, on 64 opens out of 64. The cause was a focus move
made through the focus manager rather than through an element, which the ⚠️ below states as a rule.
A second activation, tens of milliseconds later, has no identified cause and is what the correction
below exists for.

So the Windows client puts the window back in front if the mailbox takes it while the window is
still opening, and stops as soon as the reader presses anything in the mailbox, or after three
seconds, whichever comes first. The boundedness is the point: it corrects the app's own interference
and never the reader's intent.

⚠️ **A window that a toolkit stacks above the mailbox breaks the second half of the rule**, and
this is the shape it takes: GTK's `transient_for`, which is `xdg_toplevel.set_parent` on Wayland,
tells the compositor the child belongs above its parent and it is honoured as a constraint, not a
starting order. Such a window never falls behind, which reads as the rule above being satisfied
rather than broken. The mailbox and a message window are peers, so neither names the other as its
parent, and each client's own sweep is what closes them together.

⚠️ **Nothing here may reach for a focus manager's "move focus" call.** Those act on whichever
element holds focus *now*, which is in the window being moved away from, so a call meant to put
focus in a new window focuses the old one and activates it. That was this contract's own bug on
Windows, at 64 opens out of 64. A window that wants focus somewhere names the element.

## One application's windows

**Every window a client opens is the app's own window in the desktop's window list, switcher and
dock.** A window filed under a second application is one the reader has to hunt for: it carries
neither the app's name nor its icon, and the switcher the desktop offers for moving between one
application's windows will not reach it. macOS and Windows get this from the toolkit, where a
window belongs to the running application by construction. GTK does not: a window that is in no
`GtkApplication`, which both of these deliberately are not, carries the **process name** as its
Wayland `app_id`, so the process claims the application id as its program name instead
(`client-traps.md`).

## An action reaching more than one window

**A body that changes underneath its readers reaches every reader showing that message.**
Answering an invitation rewrites its card, and a window still offering Accept over times the user
has already agreed to is the failure this exists to prevent. The core republishes by message, so
the pane and every window on it move together, and a reader on a different message is untouched.

## Reachable without a pointer

**A double-click is not an affordance for everyone.** It cannot be reached from the keyboard or
invoked by a screen reader, so every client also offers the same thing as a **named item on the
message row's context menu** (`action_open_in_window`). That item, not the gesture, is what the
capability matrix claims.

## Per-platform

| | macOS | Windows | Linux | iOS/iPadOS | Android |
|---|:---:|:---:|:---:|:---:|:---:|
| One core per process (no second main window) | ✅ | ✅ | ✅ | — | — |
| Double-click a row → reading window | ✅ | ✅ | ✅ | — | — |
| "Open in new window" on the row's context menu | ✅ | ✅ | ✅ | — | — |
| Full action row in the window | ✅ | ✅ | ✅ | — | — |
| Reply / forward → composer window | ✅ | ✅ | ✅ | — | — |
| Main window closing sweeps both | ✅ | ✅ | ✅ | — | — |
| A window stays in front of the mailbox | ✅ | ✅ | ✅ | — | — |
| The mailbox can be raised back over it | ✅ | ✅ | ✅ | — | — |
| The windows are all one application's | ✅ | ✅ | ✅ | — | — |

**How each desktop satisfies the first row**, the question this contract opened and each client
has now answered of its own toolkit. macOS had a real second core behind SwiftUI's ⌘N and that
command is removed. WinUI offers no such command, the shell creates exactly one `MainWindow`, and
a second launch redirects to the running process (`AppInstance.FindOrRegisterForKey`). GTK offers
none either, and `GApplication` hands a second launch to the process already running.

## Known gaps

- **On Linux nothing gates the stacking itself.** The widget tests assert that neither window names
  the mailbox as its parent, which is the mechanism, but the acceptance suite runs on a **tiling**
  compositor, where a second window is placed beside the first and "in front" means nothing. That a
  window opens in front and that the mailbox can be raised back over it are hand-verified on a
  GNOME session; what a capture there can show is the `app_id` each window carries
  (`swaymsg -t get_tree`), which is the other half of the rule.
- **On Windows one activation after a window opens is still unexplained, and the correction is a
  timer rather than a cure.** Three seconds covers what was measured and a press in the mailbox ends
  it sooner, but neither proves the mailbox has stopped: an activation arriving later would put a
  window behind again. The reading pane's WebView2 was the obvious suspect and is not it, measured:
  its navigation completes more than a second before the activation arrives. Whatever it is,
  removing it is better than correcting it, and `MainWindow.WindowOrder.cs` is the instrument.
- **On Windows the first press of a double-click still opens the row in the pane, and is undone.**
  The list raises its own click on that press, before anything can know a window was wanted, so the
  pane is put back once the double-tap arrives and the trailing click is refused. The end state is
  the contract's, and the pane usually never repaints, because it goes on drawing what it had while
  the correction runs. What this does not give is macOS's stronger property: a double-click *can*
  raise the unsent-draft prompt there, because the first press is an ordinary click. Anyone who
  finds a way to defer that press without adding latency to every single click should take it.
- **Closing the mailbox ends the app on Windows and Linux**, so "reopening on the same core" does
  not arise on either: GTK quits with its last application window and Windows has no state where an
  app outlives its windows, and both are their desktop's own convention. The sweep still matters,
  because it is what stops the message windows outliving the list they were opened from, and a
  second launch is still not a second app.
- **Archive and delete closing the window is hand-verified on Windows, not gated.** Proving it
  automatically spends a seeded message on the shared harness and nothing puts one back, so the UI
  suite covers every other rule and leaves that one to a person
  (`clients/windows/uitests/ReadingWindow.Tests.ps1` says so in place).
- **A reading window does not follow the message.** Moving or deleting the message from somewhere
  else leaves the window showing what it had; only an invitation's own card is republished. The
  window is closed by the archive and delete on *its* action row, not by the same action taken
  elsewhere.
- **No "New message" window.** A new message opens where it always did. Only a reply or a forward
  raised inside a reading window gets a window, because that is the case where the inline composer
  would land somewhere the user is not looking.
- **Nothing asks before a deliberate discard, in a window or in the pane.** Cancel throws the
  draft away without a word and so does closing the window, and there is no saved-draft folder
  behind either. If that changes it changes for both together: a rule that applies to one of two
  identical acts is the shortfall, not the missing prompt.

  ⚠️ On macOS this is also the *safe* answer, not only the consistent one. Intercepting a
  SwiftUI window's close means taking over its `NSWindowDelegate`, which is SwiftUI's own: doing
  that was tried, and it broke `dismiss()` so thoroughly that a discarded draft left an empty
  window on screen. Any client tempted to add the prompt should reach for its toolkit's supported
  hook or leave it alone.
