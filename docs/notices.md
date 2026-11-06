# Notices: cross-platform contract

**Scope.** Every standing "something needs your attention" message: a state that holds until
something changes, often with a remedy. Not a *moment* (sent, queued, an undo after a swipe), which
is transient and never stacks, and not the *status line* ([`sync-progress.md`](sync-progress.md)),
which is ambient progress nobody asked about.

**Principle.** The core decides what is shown, in what order and for whom; a client decides only
where its platform puts it. Two platforms showing the same moment show the same notice first.

## The rules

| # | Rule | Why |
|---|---|---|
| 1 | **The core decides every notice**: whether it exists, its kind, severity, screens, account, action and rank, all in `NoticesSnapshot`, pulled after `Surface::Notices`. A client never infers a notice from an error, a field or a string. | Four clients deciding the same facts drew them with four severities, on different screens, and two facts on none. |
| 2 | **One notice is prominent per screen.** The first notice whose `screens` include the screen on show is drawn in the platform's prominent place; every other one is reachable from a single Issues entry, never stacked. | Six banners stacked above a list push the list off a phone, and the one that matters is no easier to find. |
| 3 | **Rank is severity, then scope, then age**: Blocked before Degraded before Informational; the whole app before one account; older before newer. The core sorts; a client draws in the order given. | So every platform shows the same notice first. |
| 4 | **Three severities, each with a meaning.** `Blocked`: something the person relies on has stopped. `Degraded`: part of it works. `Informational`: nothing is broken. Only Blocked uses the warning colour, and colour is never the only signal. | A permission gap and a dead sign-in drawn with the same triangle tell the person nothing about which to fix first. |
| 5 | **One notice per account and fact.** Three expired sign-ins are three notices, each with its own button, ranked together; two facts about one account are two notices. | Text naming three accounts above one button that fixes the first is a button that lies. |
| 6 | **The account carries its own notices.** Every notice with an `account` also marks that account's row in the sidebar with a glyph and a readable label, and opens at the top of that account's Settings page. Never a tooltip only. | A touch screen has no hover, so a tooltip is text nobody can read. |
| 7 | **One action, typed.** A notice has at most one `NoticeAction`, a closed enum the client maps onto a flow it already has: `SignInAgain` (the account's browser sign-in, which also grants what a narrower grant withheld), `OpenAccountSettings`, `SignInToAllodia`, `Retry` (a refresh of the notice's screen, which dials the account again). The core picks it: a dead sign-in on a password or token account opens Settings, because there is no browser flow to run. | A client that picks the remedy itself picks a different one per platform. |
| 8 | **A question is a notice, not a dialog.** A dialog is kept for a choice the person must make before they can continue. | A modal at an arbitrary moment interrupts whatever the person was doing, for something that can wait. |
| 9 | **Dismissing is for what already happened.** A notice about a state that holds has `dismissible` false and goes when the state does. One about an event (accounts skipped at launch, a refused folder change) can be closed with `NoticeIntent::Dismiss`, and the core remembers it until the fact goes. | Closing "your sign-in no longer works" leaves the sign-in dead and the person without the button. |
| 10 | **A failed remedy is said on the notice.** When the action does not complete, the client dispatches `NoticeIntent::RemedyFailed`; the core sets `remedy_failed` and every platform adds the same sentence beside the notice. It stands until the notice goes. A browser the person closed is not a failure and is not reported. | A sign-in that failed and a sign-in nobody tried look the same otherwise. |
| 11 | **Words come from the catalog, generated.** The core is locale-free and carries the kind and its arguments; `mailcal-l10n` generates the title and the action's label per kind for every platform, so no client keeps its own switch. | Seven locales times four clients is where a missing translation hides. |
| 12 | **Announced once.** A notice is announced to assistive technology when it is first raised (a new `id`), not on every redraw, and never at launch as a modal. | A screen reader that repeats a banner on each snapshot is a screen reader the person turns off. |

## What each fact becomes

| Fact | `NoticeKind` | Severity | Screens | Account | Action | Closes |
|---|---|---|---|---|---|---|
| `signin_expired_accounts` | `SignInExpired` | Blocked | Everywhere | ✓ | `SignInAgain`, or `OpenAccountSettings` for a password or token account | No |
| `mail_reauth_accounts` | `MailReauth` | Blocked | Mail | ✓ | `SignInAgain` | No |
| `calendar_reauth_accounts` | `CalendarReauth` | Degraded | Calendar | ✓ | `SignInAgain` | No |
| `calendar_outages` | `CalendarOutage { problem }` | Degraded | Calendar | ✓ | `Retry` when unreachable, else `OpenAccountSettings` | No |
| `contacts_outages` | `ContactsOutage { problem }` | Degraded | Contacts | ✓ | as above | No |
| `unreachable_accounts` | `Unreachable` | Degraded | Mail | ✓ | `Retry` | No |
| `offline` | `Offline` | Informational | Everywhere | | none | No |
| `allodia_sign_in_ended` | `AllodiaSignInEnded { address }` | Degraded | Everywhere | | `SignInToAllodia` | No |
| `account_connect_error` | `AccountsSkipped { count }` | Degraded | Mail | | none | Yes |
| `folder_notice` | `FolderChangeRefused { folder, action, problem }` | Degraded | Mail | ✓ | none | Yes |

- **`calendar_connect_error` raises nothing of its own.** Every calendar that did not connect is
  already a `calendar_outages` entry with its account and its reason, which the free text lacks.
- **`AccountsSkipped` counts stored accounts that could not be read**, which is what
  `account_connect_error` holds in the app. Such an account has no id, so the notice is the whole
  app's and has no page to open. An account that was read and then failed to connect is an
  `Unreachable` or `SignInExpired` notice instead.
- **A refused folder change is closed where it is held.** Dismissing it clears the pane's notice,
  the same as `FolderIntent::DismissNotice`, so the mailbox snapshot's `folder_notice` and this
  list never disagree.
- **The Allodia notice is Degraded**: mail, calendar and contacts all work without the account.
  It becomes Blocked while a stored entitlement answer grants a paid capability whose grace ends
  within seven days ([`entitlement.md`](../allodia_license/entitlement.md)), because then signing
  in again is what keeps that capability. That rise is read from the stored answer, so it arrives
  with the entitlement read on the device.

**The questions move here next.** The unfiled Sent copy (`Surface::UnfiledCopy`), an invitation
reply the server could not deliver (`Surface::InvitationReply`) and a changed time zone
(`TimeZoneSnapshot`'s pending change) become notices with two actions under rule 8, and
[`sending.md`](sending.md) changes with them. Until then each is answered through its own surface,
as that surface's contract says.

**The fields this list is built from stay.** `ConnectivitySnapshot`'s prompt fields,
`account_connect_error`, `calendar_connect_error` and `folder_notice` are what every client draws
today; they go once no client reads them, and every contract that names them moves in that change.

## Signals

`Surface::Notices` follows every `Surface::Connectivity` signal, because most notices are read
from that snapshot, and is raised on its own when the folder pane's refusal changes, when a notice
is dismissed and when a remedy is reported as failed. A client that does not draw notices yet
ignores the signal and must not let it reach a heavier reload.

## Per-platform

| | Core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| The snapshot, its ranking and both intents (rules 1, 3 to 5, 7, 9, 10) | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| One prominent notice and an Issues entry (rule 2) | | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| The account row's glyph and label, and its Settings page (rule 6) | | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| The three questions as notices (rule 8) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| Generated words per kind (rule 11) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| Announced once (rule 12) | | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| The Allodia notice raised to Blocked near a lapse | ⬜ | | | | | |

Where each platform puts the prominent notice: macOS and iPadOS, a card at the top of the sidebar,
above the accounts it is about; iPhone, the first inset row of the list, opening a sheet with the
rest; Windows, one `InfoBar` above the list, severity mapped to Error, Warning and Informational,
`IsClosable` only when `dismissible`; Android, one Material outlined card at the top of the list
(Material 3 has no banner component, and a snackbar disappears), the rest in a bottom sheet;
Linux, one `AdwBanner` under the header bar, the rest behind an Issues button that appears only
when there is more than one.

Legend as [`README.md`](../README.md): ✅ shipped · 🚧 in progress · ⬜ planned.

## Enforcement

- [`notices_tests.rs`](../crates/mailcal-bindings/src/notices_tests.rs) pins the table above,
  the ranking, one notice per account and fact, dismissal and the failed remedy;
  [`app_notices_tests.rs`](../crates/mailcal-bindings/src/app_notices_tests.rs) pins the pull, the
  signal and the intents through the FFI.
- A new fact a client would otherwise draw becomes a `NoticeKind` here, with its row in the table,
  before any client draws it.
- A client that draws notices draws them from `NoticesSnapshot` alone, and its matrix cells move
  in that change.
