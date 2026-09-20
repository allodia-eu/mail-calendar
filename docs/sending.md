# Sending: cross-platform contract

**Scope.** Who a message says it is from, what a forward takes with it, what every client shows
while one is going out, and what it does when one goes out but leaves no copy behind. Binding on
every platform that ships a composer.

**Principle.** *Delivering a message and keeping the sender's copy of it are two different
operations, and a client must never let the second one fail in silence.* A Sent copy is how a
person checks that a message really left; losing one without saying so is worse than most
failures that do interrupt, because nothing later recovers it.

## Why they cannot be one transaction

The obvious fix (treat "delivered **and** filed" as the unit of success and retry the pair)
is the one thing that must not happen. Retrying a send that already succeeded puts the message
in front of its recipients a second time, and no amount of missing-copy anxiety is worth that.
So the two stay separate:

- **Delivery is never repeated.** A submission that reached the server is final, whatever
  happens afterwards.
- **Filing alone is retried**, and made idempotent by the provider: it searches Sent for the
  message's own `Message-ID` before placing a copy, so a retry that races a first attempt which
  actually landed finds it instead of duplicating it.

On IMAP/SMTP these really are two round trips (SMTP dials fresh per send, the `APPEND` rides
the standing IMAP session), which is why only that transport can reach the failure at all. JMAP
files the copy with the submission's `onSuccessUpdateEmail`; Graph and Gmail file it server-side.

## A send that cannot go out is kept, not lost

**A message the app accepted is the user's, and it does not evaporate because a server was
not answering.** A send whose failure is worth retrying stays in the **Outbox**, a durable
queue the engine owns, and goes out by itself when it can.

- **`SendStatus::Queued` is not `Failed`.** Failed means nothing will retry it. Telling
  someone a queued send failed invites them to write the message a second time, and the
  first one then arrives too.
- **Which failures queue is the engine's decision, not a client's** and not this layer's: a
  retryable class parks the op and everything else settles it (`store-and-sync.md`). The core
  asks the queue whether the message is still there rather than re-reading the error, so
  there is one rule, in the one place that owns it.
- **A queued message is on screen the moment it queues, offline included.** The queue lives
  in the store, not on a server, so being offline is no reason not to show it, and offline is
  the ordinary reason a send queues in the first place. The follow-up after a write skips its
  sync while the device is offline, which is right (there is nothing to read), but it must
  still republish the list: without that the send hint says "waiting to send" while the pane
  offers nowhere to look, and reconnecting hides the evidence by fixing both at once.
- **The queue drains on three signals and no timer**: the device coming back online, the end
  of every sync pass, and the user pressing Send now. A timer would wake a dead network on a
  battery, and the reachability signal alone is not enough: a *server* outage with no device
  outage produces no transition at all, which is why a sync pass drains too.
- **Coming back online clears each message's backoff.** The backoff is the engine's guess at
  how long to wait for a server that was not answering, and reconnecting is the one fact that
  makes the guess obsolete. Without this a person watches their mail sit for up to half an
  hour after their network returns. Attempt counts are **not** reset: one more attempt now is
  not a fresh retry bound.
- **Only sends appear in the Outbox.** A queued archive or flag change is a write on a
  message that already lives in a folder, and the folder is where its owner will look for it.
  Those drain in the background and are never shown as unsent mail.

### What a queued message offers

| Action | What it does | When it is refused |
|---|---|---|
| **Send now** | Clears the backoff and drains immediately | While the send is in flight, or awaiting confirmation |
| **Cancel** | Withdraws it so it is never delivered | Same |
| **Edit** | **Withdraws it, then** hands it back to the composer | Same |

**Edit withdraws before it opens.** The other order leaves a window in which a drain delivers
the message being edited, and no part of this app can take that back. The withdrawn message
then exists *only* in `Surface::ComposeRequest`, which is why that request stands until the
host says its composer holds it rather than auto-clearing.

**A message awaiting confirmation offers no retry.** It may already be in front of its
recipients, so the one thing a client must not do is offer to send it again.

## The three surfaces

| | `Surface::Sending` → `SendStatus` | `Surface::UnfiledCopy` → `UnfiledCopy` | `Surface::ComposeRequest` → `ComposeRequest` |
|---|---|---|---|
| Lifetime | Transient; the core auto-clears it after 2.5s | **Standing**, until the user answers | **Standing**, until the host's composer holds it |
| Shape | An inline hint | A modal, or the loudest thing the client has | The client's own composer, opened |
| Answers | nothing: it is a status | "Save to Sent" / "Not now" | nothing: dismissing it acknowledges receipt |

The Outbox itself is a fourth thing and not a surface at all: it rides the mailbox-list
snapshot (`outbox`, `showing_outbox`), because the pane row that counts it is on screen in
every view.

**`SendStatus::SentNotFiled` shows no hint of its own.** The standing question is already on
screen and says the same thing with a button; two notices for one event is noise. What the
variant exists for is to stop a client rendering the plain "Message sent" over a send that did
not leave a copy.

## The sender's name

Mail goes out as `Name <address>`, and the `Name` is the account's, set by the person who owns
it. One value per account, held by the core, put in the `From` of every send.

1. **The core's copy is the one that reaches the recipient.** Every provider assembles the
   message from the draft the core hands it, so the stored name is what goes on the wire. Three
   providers also keep a copy of their own; that copy is for the account's *other* clients and
   never decides what this app sends.
2. **Who may change it is read from the capability, never from the account's kind.**
   `AccountSyncRow::sender_name_editable` is `false` only where a provider holds the name and
   the account holder cannot change it: a Microsoft mailbox takes it from the organisation's
   directory. There the card shows the name and offers no field. A client that branched on
   "is this a Microsoft account" would be wrong the first time a provider changed its mind.
3. **Empty is a real answer, and it is the first-run state.** An account with no name sends as a
   bare address. No client may substitute the address, the login, or anything derived from
   either: inventing a name puts words in the sender's mouth.
4. **It is asked for once the account connects, never on the first screen, and only where the
   provider does not already know the answer.** The screen that adds an account is the address
   field and nothing else ([`onboarding.md`](onboarding.md)), so the ask is a step after the
   connection succeeds. Every add route puts one question to the core, `needs_sender_name`,
   and draws the step only on `true`. A provider that holds a name is already answering what
   the step asks, so the core **adopts** it and the step is not drawn: a field arriving filled
   in, over a name that is already this mailbox's everywhere else, asks the user to confirm
   something they never asked to revisit. Adopting is the half a client may not skip on its own; the
   `From` header reads the stored name, so a client that merely hid the step would leave the
   account sending as a bare address while the provider's own client shows a name. The
   adoption is local: the name came from the provider, and pushing it back would be a write
   that changes nothing. Where the step is drawn, skipping is one action and leaves the
   account nameless.
5. **The field is never validated by a client.** The core sanitises what it is given: control
   characters become spaces, runs of whitespace collapse, the value is trimmed and capped at
   128 characters. A pasted line out of a document is a `From` header with a second header in
   it, and one opinion about that is the only safe number.
6. **Settings shows the same value the switcher does.** `AccountRow::name` and
   `AccountSyncRow::sender_name` are the same string; a client keeps no copy of its own.
7. **A From field reads `Name <address>`.** The composer's From is where the sender chooses
   who a message comes from, so it shows what the recipient will see, not half of it. The label
   comes from `sender_label(name, email)`, which answers the address alone when no name is set:
   the empty case is the interesting one, and four hand-rolled versions of "unless it is blank"
   is four chances to show somebody a lone pair of angle brackets. The **sidebar** keeps showing
   the address, which is what an account is recognised by.

Where the account holder owns the provider's copy (JMAP, Gmail), a change is pushed there too,
best-effort. It is not reported: the user asked to be called something and, on this device,
they now are.

## What a forward carries

A forward passes the message on, so the recipient gets what the sender was sent, not a copy of
its text. **The composer opens holding the original's files**, as ordinary attachments in the
same list a picked file lands in: exactly the files the reading view shows, each keeping the name
and media type its sender gave it. The quoted body's inline `cid:` images are not among them;
they are re-attached as the parts the quote references, so a forwarded logo is a picture in the
message rather than a second file.

They are **removable**, like anything else in that list. A forward proposes the files, it does not
impose them, which is what a person forwarding one page of a long thread expects and what every
mail client they have used does.

Four rules hold it together:

1. **The core stages, the client shows.** `stage_forwarded_attachments` writes the files into a
   directory the client names and answers with a name, media type and path for each. From there
   they are indistinguishable from a picked file or a shared one: same list, same removal, same
   submit. No client decides which parts of a message are files, and none reads a MIME part.
2. **The composer opens after staging, never before.** A composer on screen holding nothing can
   be sent in the window before the files arrive, which is exactly the forward-without-its-
   attachments this exists to prevent. Staging reads the raw source the reading view has already
   cached, so in the ordinary case there is nothing to wait for.
3. **Files that cannot be read are said out loud.** Staging is all or nothing, and a failure
   opens the composer with the error line set rather than with an empty attachment list. An empty
   list is a claim: *this message had nothing attached*. Making it silently while the files sit
   unreadable on a server is how a forward loses them without anyone noticing.
4. **Carrying them is not a draft.** The "Discard draft?" guard measures the attachment count
   against what the forward opened with, so abandoning one the user never typed into asks
   nothing: the files are still in the mailbox and nothing is lost. Taking one off, like adding
   one, is a decision about what goes out and does count. A **share**'s files count from the
   start, because those the user chose in their file manager and would have to share again.

A **reply** carries none of this. It answers the message rather than passing it on, and sending
someone their own file back is noise that repeats on every turn of a long thread.

## Rules

1. **Never word an unfiled copy as a failed send.** The message *was* sent and the recipients
   have it. A client that says "couldn't send" makes the user's next move "send it again", the
   one action that causes real harm here.
2. **The core owns both edges of the question.** It raises it, clears it the moment the copy is
   filed or the user dismisses it, and signals both times. A client mirrors what the core holds
   and never closes the question itself: a modal dismissed locally leaves a question standing
   that nobody can see or answer.
3. **The retry carries no handle.** `Intent::RetryUnfiledCopy` names no message: the core holds
   the one it is asking about. A double-tap therefore cannot file two copies, and a client
   cannot file something the core is no longer offering. A repair writes its outcome back only
   while the question it started from is still the one standing: a second send can fail to
   file while the first repair is out, and answering *its* question with the older message's
   result would lose the newer copy for good.
4. **Disable the buttons while `retrying` is set** rather than letting the user queue attempts.
5. **The provider detail is not user copy.** `UnfiledCopy::detail` is a failure class for the
   log and the diagnostics screen; the modal says what happened in plain language.
6. **A composer that could have saved a draft names its composition on the submit**, so the
   copy in Drafts goes when the message is accepted and stays when the send fails
   ([`drafts.md`](drafts.md)). A submit that leaves it out sends correctly and files a
   duplicate the user finds weeks later.

## Per-platform

| Platform | Outbox row | Queued list | Send now | Cancel | Edit |
|---|---|---|---|---|---|
| macOS / iOS / iPadOS | ✅ pane row, hidden at zero | ✅ | ✅ | ✅ | ✅ |
| Windows | ✅ pane row, hidden at zero | ✅ in the list's own column | ✅ row menu | ✅ row menu | ✅ row menu |
| Android | ✅ drawer row, hidden at zero | ✅ its own screen | ✅ row menu | ✅ row menu | ✅ row menu |
| Linux | ✅ pane row, hidden at zero | ✅ | ✅ row menu | ✅ row menu | ✅ row menu |

**The three actions are offered only on a message that is still waiting.** One in flight cannot be
called back, and one whose delivery could not be confirmed may already be in front of its
recipients. Windows draws them disabled rather than absent, so the menu is the same shape on every
row and the state beside it says why; Apple, Linux and Android leave them off, and the latter two
drop the row's overflow control with them rather than opening it onto nothing. Either answers the
rule, which is that neither row may offer a retry.

**Edit may not be refused.** The message has left the queue by the time a host is asked to open it,
so it exists nowhere else, and the request stays standing until a host says its composer has had
it. On Windows and Linux the composer's own discard guard still runs, because a half-written draft
in the pane is the user's too, but answering *Keep editing* opens the withdrawn message in a
composer window of its own rather than dropping it. Android has no second window and needs none:
its composer is a full-screen dialog over whichever list is behind it, so the withdrawn message
opens there and nothing of the user's is displaced.

| Platform | Send hint | Unfiled-copy question | Retry | Dismiss | Name asked at setup, only where the provider holds none | Name in Settings | `Name <address>` in From |
|---|---|---|---|---|---|---|---|
| macOS / iOS / iPadOS | ✅ banner | ✅ sheet, non-dismissible | ✅ | ✅ | ✅ | ✅ | ✅ picker and single-account row |
| Android | ✅ banner | ✅ `AlertDialog`, non-dismissible | ✅ | ✅ | ✅ | ✅ | ✅ field and menu items |
| Windows | ✅ InfoBar | ✅ InfoBar, `IsClosable=False` | ✅ | ✅ | ✅ | ✅ | ✅ picker and single-account row |
| Linux | ✅ banner | ✅ modal, non-dismissible | ✅ | ✅ | ✅ | ✅ | ✅ dropdown |

| Platform | A forward opens holding the original's files | Removable | Failure said out loud | Not a draft on its own |
|---|---|---|---|---|
| macOS / iOS / iPadOS | ✅ | ✅ | ✅ composer error line | ✅ the guard watches for a *change* |
| Android | ✅ | ✅ | ✅ composer error line | ✅ counted against the seed |
| Windows | ✅ | ✅ | ✅ composer error line | ✅ counted against the seed |
| Linux | ✅ | ✅ | ✅ composer error line | ✅ counted against the seed |

## Known gaps

- **A newly added Microsoft mailbox can no longer be given a local sending name.** Rule 4's
  adoption takes any name a provider holds, a read-only one included, and rule 2 already refuses
  to offer Settings an editor for it, so the tenant directory's name is now the only answer for
  that account. Before the step became conditional it was the one place a Graph user could type
  a different one, which was an accident rather than an offer: the step never consulted
  `sender_name_editable`. Accounts that already carry a name are unaffected, since a stored name
  outranks the provider's. Narrowing the adoption to `IdentityControls::Writable` would restore
  it, at the cost of showing that user a step whose field is filled in with the answer they are
  about to be told they cannot change anywhere else.
- **An Apple row names its account only when it has nothing else to say.** `docs/folder-pane.md`
  rule 18 has every row naming the account it will go out from, because this is the one list
  holding every account's mail at once; the Apple row puts the account on the first line as a
  stand-in for recipients it has not got, and otherwise leaves it off. On a single-account device
  nothing is lost. Windows draws it on every row.
- **No automated suite watches a queued message appear.** Getting one takes a send that fails for
  a reason worth retrying, which means taking the mail server away between the connect and the
  send. The showcase seeds have no server to take away, and a Windows CI runner cannot run the
  harness at all, so what the suites gate is the half that holds at zero: that the pane draws no
  Outbox row when nothing is waiting (`FolderPane.Tests.ps1`). The rest is the core's own tests
  (`outbox_tests.rs`) plus, per client, the row rules a unit suite can reach
  (`OutboxRowTests.cs`, `SidebarTreeTests.cs`, `outbox_tests.rs` in `clients/linux`,
  `OutboxTest.kt`). Verify the running client by hand: bring the harness up, connect, stop its
  container, send, and act on the row.
- **A staged file outlives its composer.** The files are written into the client's own cache and
  nothing deletes them when a forward is sent or abandoned, exactly as for an attachment opened
  from the reading view. The OS reclaims that directory; until it does, a decoded copy of the
  files is on disk.
- **The question does not survive a restart.** The core holds it in memory, so quitting with
  one open loses the chance to retry: the message stays sent, and the copy stays missing.
  Making it durable means recording an outbox op for a submission that already succeeded,
  which is a larger change than the residual loss justifies today.
- **The name is per account, not per address.** An account that sends from an alias uses the
  same name for all of them, because the stored value is keyed by account. Gmail and JMAP both
  model a name *per identity*, so the shape to grow into exists; nothing asks for it yet.
- **A provider's copy is pushed, never pulled back.** A name changed in a webmail after setup
  does not reach this device: the seed is read once, when the field is offered. Re-reading it on
  every sync would let a server overwrite what the user typed here, which is the worse failure.
  The one place that does read it again is the **harness boot**, which injects a canned account as
  a stored config and so never runs the step: each client seeds that account's name from the
  provider on a dev launch, debug-only and never on a real-accounts launch.
- **Only the composer's From carries the `Name <address>` label.** Settings → Composing's default
  send account, and the account switcher, still show the address alone. Both name an account
  rather than a sender, which is the address's job; a label there would be a second answer to
  "which account is this" in a place nobody is choosing what a recipient sees.
- **JMAP's filing is trusted, not checked.** The implicit `Email/set` that
  `onSuccessUpdateEmail` performs can report the Drafts→Sent move `notUpdated`, and that
  response is not read, so it would pass as filed. Unlike a lost IMAP `APPEND` the message is
  still in the account and still syncs, so the copy is misfiled rather than absent.
