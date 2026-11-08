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
| 6 | **The account carries its own notices.** Every notice with an `account` also marks that account's row in the sidebar: on one line under the address, never wrapping, the first notice's glyph and its `mark`, a few words with no address (the row already shows it), or `notices_account_count` ("2 issues") when there are several. The full sentences are for the notice itself, the Issues list, the account's Settings page, at whose top they all appear, and the screen reader. Never a tooltip only. | A touch screen has no hover, so a tooltip is text nobody can read; a sentence that repeats the address wraps over three lines of a narrow sidebar. |
| 7 | **One action, typed.** A notice has at most one `NoticeAction`, and a question (rule 8) a second, its other answer (`second_action`); both come from a closed enum the client maps onto a flow it already has: `SignInAgain` (the account's browser sign-in, which also grants what a narrower grant withheld), `OpenAccountSettings`, `SignInToAllodia`, `Retry` (dispatched as `NoticeIntent::Retry`, for which the core refreshes the notice's screen, which dials the account again), `Review` (the question whether to allow an account's new server), and the questions' answers, each an intent the client already sends: `FileSentCopy` and `LeaveSentCopy` (`Intent::RetryUnfiledCopy`, `Intent::DismissUnfiledCopy`), `SendReply` and `DontSendReply` (`Intent::AnswerReplyPrompt`, never with `remember`; the subject composed from the notice's `response`), `UseDeviceTimeZone` and `KeepTimeZone` (`Intent::AcceptTimeZoneChange`, `Intent::DismissTimeZoneChange`). While `running` is set an answer is being carried out, and both buttons are held. The core picks the action: a dead sign-in on a password or token account opens Settings, because there is no browser flow to run. | A client that picks the remedy itself picks a different one per platform. |
| 8 | **A question is a notice, not a dialog.** A dialog is kept for a choice the person must make before they can continue. A question is answered, never closed, and the button for its other answer is drawn as the lesser of the two. | A modal at an arbitrary moment interrupts whatever the person was doing, for something that can wait. |
| 9 | **Dismissing is for what already happened.** A notice about a state that holds has `dismissible` false and goes when the state does. One about an event (accounts skipped at launch, a refused folder change) can be closed with `NoticeIntent::Dismiss`, and the core remembers it until the fact goes. | Closing "your sign-in no longer works" leaves the sign-in dead and the person without the button. |
| 10 | **A failed remedy is said on the notice.** When the action does not complete, the client dispatches `NoticeIntent::RemedyFailed`; the core sets `remedy_failed` and every platform adds the same sentence beside the notice. It stands until the notice goes. A browser the person closed is not a failure and is not reported. A **Retry** is the core's to judge, because only the core knows when the dial it starts has ended: it marks the notice when that dial ends and the notice still stands. | A sign-in that failed and a sign-in nobody tried look the same otherwise. |
| 11 | **Words come from the catalog, generated.** The core is locale-free, so it names each notice's words rather than writing them: `title`, `detail` (why, under the title, where there is room), `mark` (the account row's few words) and `action_label` are a `NoticeText`, a catalog key and its string arguments, the account named by its address. `mailcal-l10n` generates one `notice(key, args)` lookup per platform over every `notice_` key, so no client keeps a switch of its own. A `notice_` message takes strings only and has no plural form. The few fixed words around a notice (`notices_more`, `notices_account_count`, `notices_remedy_failed`, `notices_issues`, `notices_dismiss`) are ordinary typed accessors. | Seven locales times four clients is where a missing translation hides. |
| 12 | **Announced once.** A notice is announced to assistive technology when it is first raised (a new `id`), not on every redraw, and never at launch as a modal. | A screen reader that repeats a banner on each snapshot is a screen reader the person turns off. |

## What each fact becomes

| Fact | `NoticeKind` | Severity | Screens | Account | Action | Closes |
|---|---|---|---|---|---|---|
| `signin_expired_accounts` | `SignInExpired` | Blocked | Everywhere | ✓ | `SignInAgain`, or `OpenAccountSettings` for a password or token account | No |
| `mail_reauth_accounts` | `MailReauth` | Blocked | Mail | ✓ | `SignInAgain` | No |
| `calendar_reauth_accounts` | `CalendarReauth` | Degraded | Calendar | ✓ | `SignInAgain` | No |
| `calendar_outages` | `CalendarOutage { problem }` | Degraded | Calendar | ✓ | `Retry` when unreachable, `Review` when the server moved, else `OpenAccountSettings` | No |
| `contacts_outages` | `ContactsOutage { problem }` | Degraded | Contacts | ✓ | as above | No |
| `unreachable_accounts` | `Unreachable` | Degraded | Mail | ✓ | `Retry` | No |
| `offline` | `Offline` | Informational | Everywhere | | none | No |
| `allodia_sign_in_ended` | `AllodiaSignInEnded { address }` | Degraded | Everywhere | | `SignInToAllodia` | No |
| `account_connect_errors` | `AccountsSkipped { count }` | Degraded | Mail | | none | Yes |
| `folder_notice` | `FolderChangeRefused { folder, action, problem }` | Degraded | Mail | ✓ | none | Yes |
| `unfiled_copy` | `SentNotFiled { subject }` | Degraded | Mail | ✓ | `FileSentCopy`, or `LeaveSentCopy` | No |
| `reply_prompt` | `ReplyNotDelivered { summary, organizer, response }` | Degraded | Everywhere | ✓ | `SendReply`, or `DontSendReply` | No |
| `pending_device` | `TimeZoneChanged { zone }` | Informational | Everywhere | | `UseDeviceTimeZone`, or `KeepTimeZone` | No |

- **`AccountsSkipped` counts stored accounts that could not be read**, which is what
  `account_connect_errors` holds in the app. Such an account has no id, so the notice is the whole
  app's and has no page to open. An account that was read and then failed to connect is an
  `Unreachable` or `SignInExpired` notice instead.
- **A refused folder change is closed where it is held.** Dismissing it clears the refusal the app
  holds, so the two never disagree; but only while the app still holds that refusal. A later refusal on the same
  account is a notice with an id of its own, which a dismiss aimed at the earlier one leaves
  standing.
- **A failed remedy belongs to the action that failed.** When a notice comes to offer another
  action (an outage that was unreachable is now refused), it has not failed yet.
- **The Allodia notice is Degraded**: mail, calendar and contacts all work without the account.
  It becomes Blocked while a stored entitlement answer grants a paid capability whose grace ends
  within seven days ([`entitlement.md`](../allodia_license/entitlement.md)), because then signing
  in again is what keeps that capability. The core reads and stores the entitlement itself, and
  the rise is read from the stored answer, so it holds while the grant is refused.

**A question's notice is the only place it is asked.** The core no longer offers the questions
through surfaces of their own: `Surface::UnfiledCopy` and `Surface::InvitationReply` reach a host
as `Surface::Notices`, and a time zone change signals it too. A question raised again for something
else (another message, another meeting, another zone) is a new notice and is announced. Whether an
account's invitation replies are emailed without asking is no longer a tick on the question: it is
the account's page in Settings ([`invitations.md`](invitations.md)).

## Signals

`Surface::Notices` is signalled whenever what the app can reach changes, when the folder pane's
refusal changes, when a notice is dismissed and when a remedy is reported as failed. It is the only
signal for any of these: the facts the notices are read from are the core's own, and no client
reads them, so a status a client draws from them (the Windows connection label, an account page's
expired sign-in, the Allodia card) is drawn from the notices too. The signal must not reach a
heavier reload.

## Per-platform

| | Core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| The snapshot, its ranking and both intents (rules 1, 3 to 5, 7, 9, 10) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| One prominent notice and an Issues entry (rule 2) | | ✅ | ✅ | ✅ | ✅ | ✅ |
| The account row's glyph and label, and its Settings page (rule 6) | | ✅ | ✅ | ✅ | ✅ | ✅ |
| The three questions as notices (rule 8) | ✅ | ✅ | ✅ | 🚧 | ✅ | ✅ |
| Generated words per kind (rule 11) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Announced once (rule 12) | | ✅ | ✅ | ✅ | ✅ | ✅ |
| The Allodia notice raised to Blocked near a lapse | ✅ | | | | | |

Where each platform puts the prominent notice: macOS and iPadOS, a card at the top of the sidebar,
above the accounts it is about; iPhone, one row under the bar of the tab on show, opening a sheet
with the rest; Windows, one `InfoBar` above the list, severity mapped to Error, Warning and Informational,
`IsClosable` only when `dismissible`; Android, one Material outlined card at the top of the list
(Material 3 has no banner component, and a snackbar disappears), the rest in a bottom sheet;
Linux, one `AdwBanner` under the header bar, the rest behind an Issues button that appears only
when there is more than one.

Windows: [`NoticeBar`](../clients/windows/Mailcal/Views/NoticeBar.xaml.cs) above every screen,
[`NoticeRules`](../clients/windows/Mailcal/Services/NoticeRules.cs) (pinned by
`NoticeRulesTests`), each action in
[`MailboxModel.Notices.cs`](../clients/windows/Mailcal/Services/MailboxModel.Notices.cs), and the
account's page in
[`SettingsDialog.AccountNotices.cs`](../clients/windows/Mailcal/Dialogs/SettingsDialog.AccountNotices.cs).

Linux: [`NoticeBar`](../clients/linux/src/ui/notice_bar.rs) across the top of the window,
[`notice_rules`](../clients/linux/src/ui/notice_rules.rs) (pinned by `notice_rules_tests.rs`),
each action in [`notices.rs`](../clients/linux/src/ui/notices.rs), and the account's page in
[`account_notices.rs`](../clients/linux/src/ui/settings/account_notices.rs). An `AdwBanner` holds
one sentence and one button, so the strip around it carries the severity's glyph, the spinner while
the button's sign-in runs and the Issues button; the detail shares the banner's sentence, a line
below the title. A question's banner draws neither answer: the strip draws both after the sentence,
the lesser first, because a banner's own button can be neither held nor given a sibling. The account row's glyph sits at the row's end, the one place an `AdwActionRow`
takes a widget, with the mark as its subtitle.

Android: [`NoticeArea`](../clients/android/app/src/main/java/eu/allodia/mailcal/NoticeViews.kt)
under each screen's bar, [`NoticeRules`](../clients/android/app/src/main/java/eu/allodia/mailcal/NoticeRules.kt)
(pinned by `NoticeRulesTest`, `NoticeViewsTest` and, for the questions, `QuestionNoticesTest`), each action in
[`MainActivityNotices.kt`](../clients/android/app/src/main/java/eu/allodia/mailcal/MainActivityNotices.kt),
and the account's page through `AccountNoticesGroup`. The account's mark is a second line under its
address in the folder drawer, and its glyph leads its entry in the account switcher. A notice is
announced through a polite live region, because a view's own announcement is deprecated.

On iPhone the row sits under each tab's bar rather than inside the list, so the calendar, which has
no list, draws it in the same place. An iPad whose sidebar column is hidden, as a split view hides it in
portrait, draws that same row over its detail column, so a notice is never out of sight. On Apple the rules are `NoticeRules.swift`, tested in
`NoticeRulesTests`; the views are `NoticeViews.swift`, the model's pull, announcement and remedies
`MailcalModel.Notices.swift`, and `BannerPlacementTests` holds the iPhone row under the bar.

Legend as [`README.md`](../README.md): ✅ shipped · 🚧 in progress · ⬜ planned.

## Enforcement

- [`notices_tests.rs`](../crates/mailcal-bindings/src/notices_tests.rs) pins the table above,
  the ranking, one notice per account and fact, dismissal and the failed remedy;
  [`app_notices_tests.rs`](../crates/mailcal-bindings/src/app_notices_tests.rs) pins the pull, the
  signal and the intents through the FFI.
- [`notice_words_tests.rs`](../crates/mailcal-bindings/src/notice_words_tests.rs) holds every key
  the core names to the catalog, with its arguments, and every `notice_` message to a key the core
  names; [`notice_lookup_tests.rs`](../crates/mailcal-l10n/src/notice_lookup_tests.rs) holds each
  platform's generated lookup and the strings-only rule.
- A new fact a client would otherwise draw becomes a `NoticeKind` here, with its row in the table,
  before any client draws it.
- A client that draws notices draws them from `NoticesSnapshot` alone, and its matrix cells move
  in that change.
