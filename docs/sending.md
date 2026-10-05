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
- **The Outbox never waits on a server.** Once a send or a drain pass has an outcome, the list
  is republished from the store **before** anything that reaches a server: the discard of the
  composer's draft, and the follow-up sync. A server that accepts the connection and then says
  nothing holds that sync for as long as it stays silent, and the Outbox it would have
  republished is the one place the user can see and edit the message in the meantime.
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

### A send survives the app ending mid-attempt

The app can end at any point of a send: closed, killed, suspended, out of power. Whoever comes
back for the message has to know whether it may have reached the server, because the two answers
are opposite: one that cannot have left is sent again, and one that may have left must never be
sent again without asking.

- **The engine records a hand-over immediately before the point of no return**: the line ending
  SMTP's `DATA`, or the last piece of the submitting request on JMAP, Graph and Gmail. Everything
  before it (connecting, signing in, uploading) is safe to repeat. Where the line falls on each
  transport is the engine's (`providers.md` in the engine).
- **The core recovers the last run's sends at start-up, before anything can drain**
  (`App::recover_outbox`, called by the boot path). A send cut off before its hand-over is
  waiting again and goes out on the next drain. One cut off after it may already be with its
  recipients, so it awaits confirmation. Without the recovery the row reads *Sending* until the
  dead attempt's lease lapses, minutes into the next run.
- **A send that may have been delivered is never sent again unasked.** That covers one cut off
  after its hand-over and one whose answer never came back. Reconnecting, Send now, Cancel and
  Edit are all refused on it. Its copy appearing in a Sent folder resolves it; otherwise the user
  does, with **Mark as Sent** or **Send Again**, and the store refuses either answer on a send
  that is not awaiting one, so a row that changed state under the click cannot send twice.
- **A refused send is kept.** A refusal no retry fixes settles the send as not sent, and the
  Outbox keeps it, payload and all, until the user sends it again, edits it or discards it.
  Reconnecting never sends it again: it hurries what is waiting, not what was refused. The hint
  says where it is (`SendStatus::NotSent`, "Not sent. Your message is in the Outbox.").
- **Once Send is pressed the Outbox is the message's one place.** Whether it went, is waiting,
  awaits confirmation or was refused, the composer's stored draft goes ([`drafts.md`](drafts.md)):
  two copies would invite sending both. Only a send that never reached the Outbox (no account
  or provider to send through) leaves the draft, because nothing else holds the words. The user
  moves a message back into Drafts with **Edit**, and with nothing else.
- **Every step leaves a log line**, because a queued send goes out with nobody watching
  ([`logging.md`](logging.md)).

### What a queued message offers

| State | Says | Offers, in this order |
|---|---|---|
| Waiting | *Waiting to send* | Send Now · Edit · Cancel Send |
| Sending | *Sending…* | nothing: it cannot be called back |
| Awaiting confirmation | *Delivery not confirmed* | Mark as Sent · Send Again |
| Not sent | *Not sent* | Send Again · Edit · Discard |

| Action | Intent | What it does | When the core refuses it |
|---|---|---|---|
| **Send Now**, and **Send Again** on a refused send | `SendNow` | Clears the backoff, or puts a refused send back in the queue, and drains immediately | In flight, or awaiting confirmation |
| **Cancel Send**, **Discard** | `Cancel` | Withdraws it so it is never delivered | Same |
| **Edit** | `Edit` | **Moves it back into Drafts** and opens it in the composer, files included | Same, and on a message a composer cannot hold (`QueuedRow::editable`) |
| **Mark as Sent** | `ConfirmSent` | Settles it as delivered; it leaves the Outbox | On a send not awaiting confirmation |
| **Send Again** on an unconfirmed send | `ConfirmNotSent` | **After the user confirms**, puts it back in the queue and drains immediately | Same |

**Edit saves, then withdraws, then opens.** The message is saved into Drafts first, through the
outbox, so it is durable before it leaves the queue; then the send is withdrawn; then
`Surface::ComposeRequest` offers the composer. An app that ends part way leaves the message in
both places, never in neither, and a drain cannot deliver the copy being edited. If the
withdrawal is refused the draft is taken away again and nothing opens. The request names the
**composition** the draft was saved under and carries the message's **files**, already staged
where the host named on `Edit`: the composer opens on that composition holding all of them,
exactly as a resumed draft does, so its saves replace the draft rather than add one and its
first save cannot take a file off it. A reply stays a reply: the composition keeps the message's
`In-Reply-To` and `References` for every save and for the send. A message carrying an
invitation's answer is not editable, because no composer holds its calendar part, so no client
offers Edit on it.

**Send Again on an unconfirmed send asks first.** It is the one action here whose mistake lands
in other people's inboxes: the message may be there already. The question names the risk and
where to look (the Sent folder, or a recipient); Cancel is the default. Send Again on a refused
send does not ask, since that message did not go.

**A message awaiting confirmation offers only the user's answer.** It may already be in front of
its recipients, so no client offers Send Now, Edit or Cancel on it, and Send Again there sends
`ConfirmNotSent`, never `SendNow`. The two Send Again items read the same because to the user they
are the same request; the intent is what keeps the unconfirmed one from going out on any other
path.

## The three surfaces

| | `Surface::Sending` → `SendStatus` | `Surface::UnfiledCopy` → `UnfiledCopy` | `Surface::ComposeRequest` → `ComposeRequest` |
|---|---|---|---|
| Lifetime | Transient; the core auto-clears it after 2.5s | **Standing**, until the user answers | **Standing**, until the host's composer holds it |
| Shape | An inline hint | A modal, or the loudest thing the client has | The client's own composer, opened |
| Answers | nothing: it is a status | "Save to Sent" / "Not now" | nothing: dismissing it acknowledges receipt |

The Outbox itself is a fourth thing and not a surface at all: it rides the mailbox-list
snapshot (`outbox`, `showing_outbox`), because the pane row that counts it is on screen in
every view.

**`SendStatus::Unconfirmed` is a warning, never a failure and never a wait.** The message may be
with its recipients, so "couldn't send" invites a second copy, and "waiting to send" promises an
attempt that will not happen on its own. The hint points at the Outbox, where the question is.

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

| Platform | Mark as Sent · Send Again on an unconfirmed send | Send Again asks first | Send Again · Edit · Discard on a refused send | Edit opens on the draft, holding its files | Unconfirmed and not-sent hints |
|---|---|---|---|---|---|
| macOS / iOS / iPadOS | ✅ context menu | ✅ alert | ✅ context menu | ✅ | ✅ banner |
| Windows | ✅ row menu | ✅ `ContentDialog` | ✅ row menu | ✅ | ✅ InfoBar |
| Android | ✅ row menu | ✅ `AlertDialog` | ✅ row menu | ✅ | ✅ banner |
| Linux | ✅ row menu | ✅ modal | ✅ row menu | ✅ | ✅ banner |

**Each state offers what the first table says and nothing else.** A row in flight offers nothing:
Apple, Linux and Android leave its menu off, the latter two dropping the overflow control rather
than opening it onto nothing, and Windows draws its items disabled, so its menu keeps one shape
and the state beside it says why. An unconfirmed row offers only its two answers on every
platform.

**A compose request may not be refused.** The message is in Drafts by then, but the request is
how the user sees the Edit they asked for land, so it stays standing until a host says its
composer has had it. On Windows and Linux a composer already in the pane is left first, which
keeps what it holds in Drafts without asking ([`drafts.md`](drafts.md)), so the edited message
takes the pane and nothing of the user's is lost. Android has no second window and needs none:
its composer is a full-screen dialog over whichever list is behind it, so the edited message
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
- **Edit opens the words as text.** A queued send moved back into Drafts opens with its files and
  its conversation, but its formatting and inline pictures do not come back into the composer,
  for the reason a resumed draft's do not ([`drafts.md`](drafts.md), known gaps); the draft keeps
  them until the composer's first save. Nor does an alias it was sent from: the composer opens on
  the account.
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
