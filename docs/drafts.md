# Drafts: cross-platform contract

**Scope.** Keeping an unfinished message on the server so the user's other devices and their
webmail see it, what triggers a save, and where the words live between one save and the next.
Binding on every platform that ships a composer.

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

## When a save happens

**Autosave fires when the composer has been idle for `DRAFT_AUTOSAVE_IDLE`, not on a fixed
clock.** A client starts the interval again on every keystroke, so a user who is still typing is
not interrupted by an upload, and one who stops gets their words on the server a moment later.

The interval is the core's constant, read by every client, so the four cannot disagree about it.

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
| Autosave while composing | ❌ | ❌ | ❌ | ❌ |
| "Save as draft" | ❌ | ❌ | ❌ | ❌ |
| Discard removes the server copy | ❌ | ❌ | ❌ | ❌ |
| Resume from the Drafts folder | ❌ | ❌ | ❌ | ❌ |

## Known gaps

- **No client ships any of it yet.** The matrix above is empty on purpose: the core surface
  exists and nothing calls it. Every row is filled by the change that ships the client.
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
- **Nothing removes the stored draft when the message is sent.** A draft that is composed, saved
  and then sent leaves its copy in Drafts beside the sent message.
- **A discard racing a save that is mid-round-trip can leave a copy behind.** Withdrawing is
  refused for an op already in flight, and that op then stores the draft under a key minted
  after the removal has run, so the removal cannot have named it. The window is one round trip
  wide and the result is a stray draft rather than a lost one, which is why it is recorded here
  rather than closed by holding the discard until the save lands.
