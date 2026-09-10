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

## The two surfaces

| | `Surface::Sending` → `SendStatus` | `Surface::UnfiledCopy` → `UnfiledCopy` |
|---|---|---|
| Lifetime | Transient; the core auto-clears it after 2.5s | **Standing**, until the user answers |
| Shape | An inline hint | A modal, or the loudest thing the client has |
| Answers | nothing: it is a status | "Save to Sent" / "Not now" |

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
4. **It is asked for once the account connects, never on the first screen.** The screen that
   adds an account is the address field and nothing else
   ([`onboarding.md`](onboarding.md)), so the ask is a step after the connection succeeds,
   prefilled from `suggested_sender_name`. Where the provider already knows the name the user
   confirms it rather than typing it. Skipping is one action and leaves the account nameless.
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

## Per-platform

| Platform | Send hint | Unfiled-copy question | Retry | Dismiss | Name asked at setup | Name in Settings | `Name <address>` in From |
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
