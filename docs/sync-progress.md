# Sync progress: cross-platform contract

**Scope.** What every Allodia Mail & Calendar client is allowed to say, on screen, about mail
arriving. There are exactly **two** things it may say, and which one it says is decided by who
started the sync, never by how much it is downloading.

**Principle.** *A pass the user started may take space. A pass they did not may not.* The whole
contract follows from that one line.

## The two surfaces

| Surface | Belongs to | May take layout | What it says |
|---|---|:---:|---|
| **The bar** | A download the user **awaits**: adding an account, opening an unsynced folder, an explicit refetch, a cold first paint | ✅ its own row, **below** the message list | "Downloading 1,200 of 3,387…" |
| **The hint** | A pass **nobody asked for**: a poll tick, an `IDLE`/push notification, a boot catch-up, and the body warm that follows a sync | ❌ never: it goes inside a status line the client already draws | "Syncing eva@example.com: 3 of 12 folders" |

They do not overlap. The core reports an awaited download in
[`SyncProgressSnapshot::active`](../crates/mailcal-viewmodel/src/sync_progress.rs) and its counts;
a background pass appears only in `accounts`, and an account is never in both at once. A client
that rendered both for one pass would be saying the same thing twice, in two places.

### Below the list, never above it

The bar had been in the banner stack above the list. Every appearance and disappearance resized
the list and moved every row under the pointer, for a background pass the user never started. It
now occupies its own row **between the list and the footer**: a row under the list can shrink the
viewport, it can never push the rows down.

The hint takes the rule further: it gets no row at all. On a quiet account a poll runs every few
minutes forever, and a strip that opened and closed on that timer would be the same defect on a
longer period.

### The hint waits for mail

An account reaches the hint only once its background pass has **actually committed a message**.
A poll that finds nothing says nothing: that is what keeps the hint meaning "something is
arriving" rather than "a timer fired". It is also why the follow-up sync after the user's *own*
archive, delete or send is excluded outright: that pass does download (the moved message is
re-committed in its new folder), and the row already left the list optimistically, so there is
nothing left to explain.

### Catching up has two phases

An account is in exactly one of them, in order:

1. **Folders**: the sync pass. `folders_done of folders_total`.
2. **Bodies**: warming the synced messages' bodies afterwards, so the mailbox reads offline
   (on a phone a message over 2 MB is left for the open that asks for it; a desktop warms
   every size). `warming_bodies` is set and
   `bodies_done` is what moves.

The second phase is in the contract because it was **invisible**, and it is the longer half.
Measured on a five-account first sync: the passes finished at 00:39, and the body warm of the
largest account ran until 06:34, nearly six minutes during which the app downloaded continuously
and said nothing at all. The user could tell only from CPU and network graphs.

There is no body **total**. The warm drains against "what is still missing" in batches rather than
walking a known list, so the honest figure is how many are down so far, the same indeterminate
case the bar handles with a `None` total. Do not invent a denominator.

The warm reports every 25 bodies, not every one: the hint is a line of text, and the signal it
raises costs every client a snapshot pull.

### Naming the account

The snapshot carries an **account id**, not an address. Each client resolves it against the
account list it already renders everywhere else, so one address is not formatted two ways in one
window. An id that no longer resolves (the account was removed while its pass wound down) is
shown as-is rather than dropped.

Several accounts share one line: a status line cannot name them all, so at two or more the
addresses give way to a plain count (`Syncing 2 accounts`) and **no** figures. One account on its
folders and another on its bodies have no shared unit to add up, and a number that silently means
two different things is worse than no number.

`folders_total` is the number of folders the pass set out to sync (**one**, for a push
notification that named its folder), so `folders_done` reaching it means the pass finished, not
that the mail ran out.

## Per-platform

| Platform | Bar | Hint | Where |
|---|:---:|:---:|---|
| macOS / iPadOS | ✅ under the list, above the footer | ✅ in the footer, beside the message count | `Mailcal.Detail.swift` |
| iPhone | ✅ a strip under the list | ✅ the same strip (there is no footer); the bar wins when both are up | `Mailcal.Layout.swift` |
| Windows | ✅ its own `Auto` row under the list | ✅ in the footer status line, between the message count and "Connected" | `Views/MailListView.xaml`, `Services/MailboxModel.SyncProgress.cs` |
| Android | ✅ a strip under the list, outside the pull-to-refresh box | ✅ the same strip | `MailboxScreenParts.kt` |
| Linux | ✅ a strip under the list; the bar wins over the hint | ✅ the mail list's bottom bar | `ui/shell.rs`, `ui/model.rs` |

Copy for both comes from the shared catalog (`sync_downloading*`, `sync_hint_*`) and is assembled
client-side like every other string ([`../AGENTS.md`](../AGENTS.md) → Client conventions).

## Proving it

Both surfaces are up for a fraction of a second against any local fixture, too short for a UI
suite to catch, which is how the bar's placement came to be checked by eye on every platform and
asserted on none. Two debug-only environment variables substitute **the snapshot the host reads,
and nothing else**, so the surface signal, the FFI record, the bound properties and the layout
pass beneath them are all real:

```sh
MAILCAL_FAKE_SYNC_PROGRESS=1200/3387          # the bar: 1,200 of 3,387 (omit "/3387" for indeterminate)
MAILCAL_FAKE_SYNC_HINT=eva@example.com:3/12   # the hint, folders: 3 of that account's 12
MAILCAL_FAKE_SYNC_HINT=eva@example.com:2022   # the hint, bodies: 2,022 warmed so far
MAILCAL_FAKE_SYNC_HINT=a@x.test:3/12,b@y.test:0/5
```

They are compiled out of a release build
([`sync_progress_staged.rs`](../crates/mailcal-app/src/sync_progress_staged.rs)), so no shipped
binary can be talked into reporting a download that is not happening, and each logs a warning
whenever it is in force: a run whose log does not say so was not staged.

A hook that instead set a client's own visibility flag would be a mock of the thing under test,
and would go on passing after the wiring between core and client was cut. Which pass raises which
surface is decided upstream of all of this, and is covered by
[`sync_progress_tests.rs`](../crates/mailcal-app/src/sync_progress_tests.rs) (the policy) and
`tests_sync.rs` (the wiring that reaches it).

## One signal per surface per 250 ms, and why it is a rule

Both surfaces are driven by a streamed pass, and every commit signals `MailboxList` and then
`SyncProgress`. Neither may reach a client per commit: each signal costs a full snapshot pull and
list reconcile on the thread the client renders on. `DebouncedObserver` coalesces **both**, to at
most one notification each per window.

Coalescing only one of them achieves nothing: an undebounced `SyncProgress` following each
`MailboxList` flushes the pending one on its way through, so every commit delivers both. That is
what a five-account first sync of 7,107 messages cost when a commit was one message: ~26 ms of
main-thread work per signal, one core saturated for three minutes, and a reading pane that had
been published in **1 ms** sat on its spinner for 2 min 24 s: the `Reading` signal was queued
behind the backlog in the host's FIFO.

A commit is a **chunk** of messages ([`STREAM_CHUNK_SIZE`](../crates/mailcal-app/src/sync.rs)),
which removes that rate rather than capping it, and removes the matching per-message cost inside
the core, which no client-side coalescing could reach. The debounce stays regardless: it bounds
the surface whatever drives it, and an IDLE burst, a folder switch and a reconcile pass each
signal on their own schedule.

A signal carries no payload; the host pulls current state when it arrives. So coalescing cannot
show a stale figure, the trailing fire after the last commit always delivers the final state, and
a download that finishes inside one window simply never raises a bar, which is the right answer
for a download that is already over.

## The reading spinner obeys the same rule

Opening a message is a wait too, and the rule above decides it identically: **a wait the user
does not notice is never announced.**

`open_message` publishes **once** (the body) when it resolves inside
[`READING_PENDING_AFTER`](../crates/mailcal-app/src/reading.rs) (500 ms). Only if it is still
running at that point does it publish a second snapshot first, carrying
[`ReadingSnapshot::pending`](../crates/mailcal-viewmodel/src/reading.rs) and no body, and only
then may a client draw its loading indicator.

**A client must not spin merely because it has no snapshot yet.** That was the defect: every
client derived "loading" from *"the published snapshot's key is not the message I opened"*, which
is true from the instant of the click. A stored body comes back in single-digit milliseconds, so
moving between messages raised an indicator and removed it inside one eyeblink, read as
flickering rather than as fast, on macOS, iOS, Windows and Linux alike. Until `pending` arrives a
client draws the body area **empty**; the reading header is already filled from the row that was
clicked, so the pane reads as the message opening rather than as broken.

The threshold covers the *whole* open, including the bounded retry a cold open does while its
account is still dialing: timing only the first fetch would leave a genuine multi-second wait
silent.

| Platform | Draws nothing until `pending` | Indicator |
|---|---|---|
| macOS · iPadOS · iOS | `Color.clear` in `ReadingView.content` | `ProgressView` + `reading_loading` |
| Windows | `ShowState()` with every state off | `LoadingRing` |
| Android | `body == null -> Unit` | `CircularProgressIndicator` |
| Linux | the `blank` stack page | the `loading` stack page |

`a_fast_open_never_announces_a_wait` and
`an_open_that_outlasts_the_threshold_announces_the_wait_first` (`mailcal-app`) hold both halves:
one publish for a fast open, and a `pending` one ahead of the body for a slow one.

## Known gaps

- **The hint is mail-only.** A calendar or contacts sync in the background says nothing, on any
  platform. Nothing reports per-account progress for those passes yet.
