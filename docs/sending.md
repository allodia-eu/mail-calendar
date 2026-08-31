# Sending: cross-platform contract

**Scope.** What every client shows while a message is going out, and what it does when one goes
out but leaves no copy behind. Binding on every platform that ships a composer.

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

| Platform | Send hint | Unfiled-copy question | Retry | Dismiss |
|---|---|---|---|---|
| macOS / iOS / iPadOS | ✅ banner | ✅ sheet, non-dismissible | ✅ | ✅ |
| Android | ✅ banner | ✅ `AlertDialog`, non-dismissible | ✅ | ✅ |
| Windows | ✅ InfoBar | ✅ InfoBar, `IsClosable=False` | ✅ | ✅ |
| Linux | ✅ banner | ✅ modal, non-dismissible | ✅ | ✅ |

## Known gaps

- **The question does not survive a restart.** The core holds it in memory, so quitting with
  one open loses the chance to retry: the message stays sent, and the copy stays missing.
  Making it durable means recording an outbox op for a submission that already succeeded,
  which is a larger change than the residual loss justifies today.
- **JMAP's filing is trusted, not checked.** The implicit `Email/set` that
  `onSuccessUpdateEmail` performs can report the Drafts→Sent move `notUpdated`, and that
  response is not read, so it would pass as filed. Unlike a lost IMAP `APPEND` the message is
  still in the account and still syncs, so the copy is misfiled rather than absent.
