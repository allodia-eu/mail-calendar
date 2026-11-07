# Waiting: cross-platform contract

**Scope.** What a client shows while work the person started is running: a press of a button, a
switch, a picker or a dialog's action that waits on the host's store or a server. Mail arriving
has its own contract ([`sync-progress.md`](sync-progress.md)), and so does a message being opened
(its reading spinner); this one covers every control besides.

**Principle.** *The core says what is running; a client draws it.* A client keeps no state of its
own about whether a change is under way, so it cannot forget to show one, or show one that stops
before the work does.

## The rules

1. **A press that starts work the person waits for shows that it is working, from the press until
   the work ends,** on or beside the control that started it, as the platform's own spinner.
   Greying the control alone is not that: it reads as broken.
2. **A running control cannot start the same work again,** and while one account change runs,
   every account page holds its controls, so two changes never race: one account's change can
   write another's (a link is stored on both ends). The exception is
   a browser sign-in, where a fresh press supersedes the attempt still out
   ([`provider-oauth.md`](provider-oauth.md) rule 13).
3. **When the surface the control sat on closes, the progress goes on where the person lands.** A
   dialog that closes on Save hands its progress to the surface beneath it (the calendar's write
   badge, the send banner, the contacts list's "Saving…" line) on every surface it can close onto.
4. **Nothing that can reach the network or the host's store is called from a UI thread.** Work a
   press starts is dispatched and returns at once.
5. **What a change came to is the core's to decide and the client's to word.** A change that
   worked says nothing: the page shows its result. One that did not leaves a notice, raised once.
6. **What a control will need is said before it is pressed, by the decision that would refuse
   it.** A switch that would go to the provider's sign-in, or that cannot work until a server is
   given, says so on the page rather than failing when pressed, and what the page predicts is
   asked of the same function the change itself acts on, so the two cannot disagree.
7. **A change the core applies at once needs nothing.** Archive, delete, flag and the other message
   actions update the list optimistically; a folder change shows its pending row
   ([`folder-pane.md`](folder-pane.md)).

## How the core says it

Every control a person can act on carries a `ControlState`: `enabled`, and `running` while work it
started is under way. A running control is never enabled. A Settings page is a record the core
produces (`MailcalApp::account_page` for an account's page), read again on every
`Surface::Settings` signal; its changes are dispatched (`Intent::AccountPage`) and return at once, and
the signal is raised when one starts and when it ends. A use's switch carries `blocked`, why the
core would refuse pressing it, asked of the registry's `toggle`, which `set_use` acts on too, so a
switch is enabled exactly when nothing blocks it and no change runs; and `status`, what is wrong
with the use, said beside it whether or not it can be pressed. Only the page that started a change marks its
control running, drawn at the value it is moving to rather than the one it is leaving; every other
page holds its controls without a spinner. The page's `notice` carries
an id that grows with each one, so a client says each notice once. A removal's notice is the
accounts list's (`MailcalApp::accounts_notice`), since the page goes with the account.

Each client draws `ControlState` through one set of components of its own (a switch row, a button,
a picker, a confirmation), in its platform's idiom, so a control drawn anywhere waits the same way:
[`controls.rs`](../clients/linux/src/ui/controls.rs) on Linux,
[`ControlComponents.kt`](../clients/android/app/src/main/java/eu/allodia/mailcal/ControlComponents.kt)
on Android,
[`ControlComponents.swift`](../clients/apple/Packages/MailcalKit/Sources/MailcalUI/ControlComponents.swift)
on macOS, iOS and iPadOS, and
[`ControlComponents.cs`](../clients/windows/Mailcal/Controls/ControlComponents.cs) on Windows, whose
confirmation is drawn inline because Settings is itself a dialog and a nested one is not allowed.

## Per-platform

| | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|
| An account's page: uses, links and removal drawn from `account_page` | ✅ | ✅ | ✅ | ✅ | ✅ |

## Enforcement

- [`account_page_tests.rs`](../crates/mailcal-bindings/src/account_page_tests.rs) pins the account
  page as it is put together: a refused switch held and saying why, a status said beside it, one
  Review beneath the paused uses, the link pickers, and running work holding every control and
  marking its own, drawn where it is going.
- [`toggle_tests.rs`](../crates/mailcal-bindings/src/account_registry/toggle_tests.rs) pins the
  one decision `set_use` and the page share: the last use, a missing server, colleagues and shared
  mailboxes, a grant that lacks a use, and the page drawn from a real registry.
- [`app_account_page_tests.rs`](../crates/mailcal-bindings/src/app_account_page_tests.rs) drives a
  change through `dispatch`: it runs and the page comes back free, and one the core refuses leaves
  a notice.
- Each client's controls are tested where its toolkit runs: Linux's in the GTK widget test
  ([`controls.rs`](../clients/linux/src/ui/controls.rs)), Android's in
  [`SettingsAccountsTest.kt`](../clients/android/app/src/test/java/eu/allodia/mailcal/SettingsAccountsTest.kt),
  each drawing a page the core would answer and asking the core for every change. Apple words the
  record in
  [`AccountPageWordsTests.swift`](../clients/apple/Packages/MailcalKit/Tests/MailcalKitTests/AccountPageWordsTests.swift);
  Windows redraws the page whenever a change starts or ends
  ([`AccountSettingsRulesTests.cs`](../clients/windows/Mailcal.Tests/AccountSettingsRulesTests.cs)),
  and its UI suite
  ([`SettingsAccountsTests.cs`](../clients/windows/Mailcal.UiTests/Suites/SettingsAccountsTests.cs))
  drives the switches, the suggested link and the removal against the running app.
