# Drafts: cross-platform contract

**Scope.** Keeping an unfinished message on the server so the user's other devices and their
webmail see it, what triggers a save, where the words live between one save and the next, and
how one is picked back up. Binding on every platform that ships a composer.

**Principle.** *A draft belongs on the server, and the copy there is named by a key the server
chooses, not by anything the message carries.* Everything below follows from those two facts.

The verbs are the engine's (`Engine::put_draft`, `Engine::delete_draft`), reached through this
core's `Intent::Drafts`. Both go through the durable outbox, so a save made with no network is
queued rather than lost.

## The key is the join, never the `Message-ID`

`put_draft` answers with the key the draft is now stored under, and **that key is what names the
draft next time**. It is not necessarily the key that went in:

| Transport | On a re-save | What moves |
|---|---|---|
| IMAP | writes the new message, removes the old | the key |
| JMAP | writes the new message, removes the old | the key |
| Graph | writes the new message, removes the old | the key |
| Gmail | updates the draft object in place | the `Message-ID` |

So three of the four move the key, and the one that does not moves the header instead. There is
no field that survives a save on all four, which is why the caller keeps what the save returned
and passes it back as `replacing`. A caller that matched on the `Message-ID` would work
everywhere except Gmail, and on Gmail it would store a second copy on every save.

The core holds that key per open composition and hands it to the next save. A client never sees
it and never needs to: it names its composition, and the core does the rest.

**A save the outbox stored hands its key back the same way.** A save made with no network
settles during a later drain, with nobody watching, and a settled op is not in the queue read:
the drain report is the only place that key is ever named. The core takes it from there and puts
it on the composition, so a composer left open through an outage supersedes its own stored copy
when the network returns rather than leaving a second one behind.

## A save supersedes; a send does not

Saving is the one mail write this app repeats on purpose. The engine makes that safe from both
ends: a queued save withdraws the one it supersedes, so five saves with no network leave the
server one write rather than five, and the provider call replaces the stored copy rather than
adding to it.

This is the opposite of [`sending.md`](sending.md)'s rule, and the two must not be confused. A
send that reached the server is final and is never repeated. A save is repeated every time the
user pauses.

**A send that was accepted takes the stored draft away.** Once the message is somewhere that
will deliver it, whether it has gone out or is in the Outbox, the copy in Drafts is a
duplicate of a message already on its way, and the user would find it there weeks later
unable to tell whether it went. The composer names its composition on the submit and the core
removes the draft; a client that does not name it leaves one behind.

**A send that failed keeps it.** That is the one outcome where the stored copy is the only one
left: nothing will retry the message, and the composer that held the words has already
closed. Removing it there would make sending a way to lose mail.

**The send owns the composition from the submit on, and a client must not close it.** A composer
is dismissed the moment its submit is accepted, which is validation only: the message has not
been anywhere yet. The close and the send reach the core as separate tasks, so a client that
forgot the composition on its way out would routinely win the race and leave the send with no
record to find, and the draft in Drafts for ever with the message already delivered. The core
finishes with the composition itself when the send settles, whichever way it went.

## When a save happens

**Autosave fires when the composer has been idle for `DRAFT_AUTOSAVE_IDLE`, not on a fixed
clock.** A client starts the interval again on every keystroke, so a user who is still typing is
not interrupted by an upload, and one who stops gets their words on the server a moment later.

The interval is the core's constant, read by every client, so the four cannot disagree about it.

**A keystroke in the message body is something the host has to ask for.** The editor is a web
view and the page has no channel back to its host, which is a security gate rather than an
oversight ([`composer-security.md`](composer-security.md)). So the shared editor bundle counts
its own changes and each host samples that count, at a third of the interval: a draft reaches the
server between one and one-and-a-third intervals after the last keystroke, and never during
typing. The header fields need no sampling on three of the four platforms, because they are the
host's own state and raise their own change.

**An unchanged draft costs no write.** The core compares what it is given against what it last
put on the server and returns without calling the provider when they match. Pressing "Save as
draft" twice, or an idle timer firing on a composer nobody touched, reaches no server. A
composition with a save still queued never matches: the drain is on its way to change what the
server holds, so nothing there is settled enough to compare against.

**Two saves of one composition never overlap.** A composer has two triggers, the idle timer and
the Save button, and nothing stops both firing. Each save reads the stored key before it writes,
so the second would read it before the first had recorded one, put with nothing to replace, and
leave the server holding two copies of the message still being written. The core serialises
them.

The engine keeps the two provider calls themselves from overlapping, because both saves of one
composition share a resource key and an op leases it. That covers the round trips and nothing
else: the stale read happens here, before the engine is asked anything, which is why this
failure would have been quiet rather than an error.

**A save carries the composer's files.** The write replaces the copy on the server, so a save
that left an attachment out would take it off a draft the user is still writing. A composer
holding files, whether the user picked them or a resume opened with them, saves through the
path that reads their bytes.

**Saving is never something the user waits for.** It has no modal, no blocking spinner and no
error the composer refuses to be dismissed over. `DraftStatus` is a hint, on the same footing as
the sending hint.

**The hint belongs to one composer, not to the app.** A `Surface::DraftStatus` signal says that
some composition's save moved, not which, so every open composer re-pulls its own by naming its
composition. One that has saved nothing yet, and one that has closed, both read `Idle`: never
the state of the composer beside it. This is the rule the reading windows follow
([`reading-window.md`](reading-window.md)), and it binds for the same reason: a desktop opens a
reply in a window of its own, so two composers saving different drafts is the ordinary case, not
an edge.

## Picking one back up

A draft in the Drafts folder opens into a composer, not into the reading view, and the
composer it opens saves **over** that copy rather than beside it. The core joins the two: the
composition takes the stored draft's key, so its first save supersedes and a discard removes,
and the draft's own `Message-ID`, so every save of it goes on naming the one message. A draft
written elsewhere may carry no `Message-ID`, and a new one serves as well: what that header
has to be is the same on every save of one composition, and it is the key that names the copy
being replaced.

**Everything the composer does not open with, its next save deletes.** That is what makes a
resume stricter than a forward, which only shows less when it carries less. Three gates follow
from it, and all three refuse rather than open a composer that would save over the draft:

- **The message must be a draft.** Asked of the message, never of the folder it was opened
  from: the engine normalises every transport's own tell onto one keyword, so the answer is
  the same in a search result and inside a thread. A composition that adopted an ordinary
  message's key would have its first save rewrite received mail and its discard delete it.
- **Its content must be readable.** A composer opened on an error and saved anyway replaces
  the draft with whatever was on screen.
- **Its files must be staged first.** They are written into the host's staging directory
  before the composer opens, exactly as a forward's are ([`sending.md`](sending.md)), and
  staging is all or nothing. A composer that opened without the file the user attached
  yesterday takes it out of their mailbox on the next save.

**Opening one can take a moment.** A draft is opened from a list row, not from the reading
view, so unlike a forward it cannot count on the message already being cached: the first open
fetches it. A client does what it does for a slow reading open rather than freezing, and the
composer appears when the answer does.

**The body comes back as text.** The composer's document model is not HTML, so restoring
formatting would mean a second markup parser in the editor; see the known gaps. The engine
derives the text from the HTML when the draft carries no text part, so an HTML-only draft
still opens with its words.

**A resumed composer seeds no signature.** The body comes back as the text of a message that
was signed when it was first written, so seeding one would put a second signature under it, and
the next save would store that. It is the same reason a message withdrawn from the Outbox seeds
none ([`sending.md`](sending.md)).

**The first save after a resume always writes.** The unchanged check compares against what
this app last put on the server, and a resumed body is text derived from the stored draft
rather than the draft itself, so claiming the two match would skip a save the user can see is
needed.

## The draft is never in three places

The core keeps **no copy of its own on disk**. A draft is either on the server, or in an outbox
op waiting to reach it, and both of those are durable already. A third copy under the app's data
directory would be a second place holding the user's mail, and it would buy nothing: the outbox
records the save before the provider call, so an attempted save survives a crash without it.

What the core does hold, for as long as the composer is open, is the small record joining a
composition to its stored key. That is session state, and losing it costs nothing: the draft is
in the Drafts folder, where it is opened again by the key it already has.

## What the body passes through

A saved draft is rendered by the same builder a sent message is, so the quoted original and the
signature are re-sanitised to the inert subset on the way to the server
([`composer-security.md`](composer-security.md)). A draft is read back later by a mail client,
this one included, which makes it exactly as much of a rendering surface as a received message.

## Discarding

Discarding removes the stored copy through the same outbox. A draft that is already gone settles
as done rather than failing, so a discard retried after a lost response does not park for ever.

Discarding a composition that was never saved reaches no server: there is nothing there to
remove.

**A discard withdraws a save still waiting for a network.** The queued op is a copy of the
message too, and the composer it belonged to is gone: left in the outbox it drains after the
discard and stores the draft the user threw away, under a key nothing holds any more. A
withdrawal can be refused, because an op mid-round-trip cannot be called back; that is logged
and the removal below runs on whatever key the composition had.

## Per-platform

| | Apple | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|
| Autosave while composing | ✅ | ✅ | ✅ | ✅ |
| "Save as draft" | ✅ | ✅ | ✅ | ✅ |
| Discard removes the server copy | ✅ | ✅ | ✅ | ✅ |
| Resume from the Drafts folder | ✅ | ✅ | ✅ | ✅ |
| Sending takes the draft away | ✅ | ✅ | ✅ | ✅ |

Where each puts the two controls is the platform's own answer, and each is where that platform
already puts an action on the message rather than a field you address it with: in the composer's
action bar on macOS, iOS and Linux, in the app bar on Android, and in the action row above the
editor on Windows ([`signatures.md`](signatures.md) settled the same question).

Discard is reached from the "Discard draft?" question every platform already raises, which is why
none of them grew a second control for it. On macOS, Windows and Linux that question is what a
click on another message asks; on Android it is what the back gesture asks.

## Known gaps

- **A composer nobody pauses in is never saved.** The trigger is idleness, so a user typing
  without a break for ten minutes has nothing on the server until they stop. A second trigger on
  elapsed time would close it, at the cost of uploading a draft mid-sentence.
- **Each save carries the whole draft, attachments included.** There is no protocol for amending
  a stored draft in place, so a message with a large file re-uploads it every time it is saved.
  The idle trigger and the unchanged check are what keep that from happening while someone
  types; a user who pauses repeatedly on a draft carrying 20 MB still pays it each time.
- **A queued save is invisible until it drains.** The Drafts folder lists what the server holds,
  and the Outbox shows sends only ([`sending.md`](sending.md)), so a draft saved with no network
  is in neither list until the network returns. It is not lost, and the composer's hint says so,
  but a user who closes the composer has nowhere to look.
- **A resumed draft opens as plain text.** The composer holds a block document, not HTML, so
  bold, lists and inline pictures made in an earlier session are not restored; the words and
  the files are. Closing it means teaching the editor to read a mail body back into its own
  blocks, which is a markup parser and belongs with the editor rather than here.
- **A client tells a draft by its folder, not by the row.** The core answers per message, but
  the mailbox list does not carry it, so a draft met in a search result, or in a thread shown
  from another folder, opens read-only rather than in a composer. Nothing is lost by it; the row
  simply does not offer what the Drafts folder's does. What the snapshot carries instead is
  whether the **open folder** is Drafts, decided by the folder's role and never by its name.
- **A resumed draft takes no signature.** The picker is not offered, because the body already
  carries whatever signature was on it, and the account's would go under it as a second one.
  Changing it means editing the text.
- **A composer left open through an autosave shows one hint for every save.** The hint does not
  auto-clear, so "Saved to Drafts" stands until the next save changes it. That is the standing
  truth about the draft rather than a notification, and it is why it is drawn quietly.
- **A discard racing a save that is mid-round-trip can leave a copy behind.** Withdrawing is
  refused for an op already in flight, and that op then stores the draft under a key minted
  after the removal has run, so the removal cannot have named it. The window is one round trip
  wide and the result is a stray draft rather than a lost one, which is why it is recorded here
  rather than closed by holding the discard until the save lands.
