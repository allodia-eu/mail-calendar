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
not, and keeps whatever a double-click already did there (on macOS, expand and collapse).

**One window per message.** Double-clicking a row whose window is already open brings that window
forward rather than opening a second one on the same message.

**It leaves the pane alone.** The pane keeps the message it had and the list keeps its selection:
opening a second reader must not disturb the first, which is the whole point of the window. It
also means a double-click can never raise the unsent-draft prompt, because it takes nothing away.

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
never ended, so no account reconnects and no sync starts again.

**Windows are not restored across launches.** A window is a view onto a message this session
opened; at the next launch its header and its body are both gone, so it would open blank. Where a
platform restores windows by default, that is turned off for these two.

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
| One core per process (no second main window) | ✅ | ⬜ | ⬜ | — | — |
| Double-click a row → reading window | ✅ | ⬜ | ⬜ | — | — |
| "Open in new window" on the row's context menu | ✅ | ⬜ | ⬜ | — | — |
| Full action row in the window | ✅ | ⬜ | ⬜ | — | — |
| Reply / forward → composer window | ✅ | ⬜ | ⬜ | — | — |
| Main window closing sweeps both | ✅ | ⬜ | ⬜ | — | — |

## Known gaps

- **Windows and Linux have none of it yet.** The core half is done and is platform-neutral: the
  reader-keyed slot, `Intent::OpenMessage`'s `window` form, `reading_window_view` and
  `close_reading_window` are all on the FFI surface both clients already consume. What is missing
  is each client's windows.
- **Whether ⌘N (or its equivalent) built a second core on Windows and Linux has not been
  checked.** It was true on macOS and is fixed there; the two other desktops each need the same
  question asked of their own toolkit before they claim the first row above.
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
