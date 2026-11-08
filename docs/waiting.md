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
   write another's (a link is stored on both ends). A change that still arrives meanwhile, as a
   text field sends what was typed when the held page takes its focus, waits and runs next, drawn
   running from then; the same work asked for again is dropped. The exception is a browser
   sign-in, where a fresh press supersedes the attempt still out
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
the signal is raised when one starts, when one is queued behind it and when each ends. A use's switch carries `blocked`, why the
core would refuse pressing it, asked of the registry's `toggle`, which `set_use` acts on too, so a
switch is enabled exactly when nothing blocks it and no change runs; and `status`, what is wrong
with the use, said beside it whether or not it can be pressed. Only the page that started a change marks its
control running, drawn at the value it is moving to rather than the one it is leaving; every other
page holds its controls without a spinner. While the account is used for mail the page carries
its `sender_name` and its `mail` settings too (fetch depth, message size, push or poll, the
interval, the watched folders), each a control of its own, a switch to push drawn with the
folders it will watch: a change to one is stored before it ends, and what follows from it (telling the provider the name, reconciling the mail to a new depth
or size) runs on afterwards as sync work the page does not wait on. For an account whose servers
can be edited the page carries its `servers`: the form, drawn from the edit being saved while a
save runs, from the edit the last save refused (its password included, so one field is corrected
rather than all retyped), and otherwise from what is stored; and Save, held too while
`account_endpoints_gap` says the fields leave something out, which the page says beside it. The
fields are held with Save, without its spinner, so nothing typed during a save is drawn over when
it ends. The page's `notice` carries
an id that grows with each one, so a client says each notice once. A removal's notice is the
accounts list's (`MailcalApp::accounts_notice`), since the page goes with the account.

Each client draws `ControlState` through one set of components of its own (a switch row, a button,
a text field, a picker, a confirmation), in its platform's idiom, so a control drawn anywhere waits the same way:
[`controls.rs`](../clients/linux/src/ui/controls.rs) on Linux,
[`ControlComponents.kt`](../clients/android/app/src/main/java/eu/allodia/mailcal/ControlComponents.kt)
on Android,
[`ControlComponents.swift`](../clients/apple/Packages/MailcalKit/Sources/MailcalUI/ControlComponents.swift)
on macOS, iOS and iPadOS, and
[`ControlComponents.cs`](../clients/windows/Mailcal/Controls/ControlComponents.cs) on Windows, whose
confirmation is drawn inline because Settings is itself a dialog and a nested one is not allowed.
A text field sends what was typed on Return and when it loses focus, once for each edit, and a
redraw waits on it only while it holds text it has not sent; a form whose fields a button sends
together holds a redraw while it differs from what it was drawn with, wherever the caret is; a redraw also waits while a picker's
list is open, which it would close under the pointer, and comes once the list closes. Every control on a page the core draws goes through them, one that only opens something (a
question, a form) included: it is given a state that never runs, so it is held while a change runs
on the page and the spinner stays with the control that started it. A platform control of the
client's own is drawn only where the core gives no state yet, and the fix there is the core's
record, not a state the client keeps.

## Per-platform

| | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|
| An account's page: uses, links and removal drawn from `account_page` | ✅ | ✅ | ✅ | ✅ | ✅ |
| An account's page: its name and mail settings drawn from `account_page` | ✅ | ✅ | ✅ | ✅ | ✅ |
| An account's page: its servers drawn from `account_page`, saved as a page change | ✅ | ✅ | ✅ | ✅ | ✅ |

## Enforcement

- `cargo xtask check-control-components`
  ([`control_components.rs`](../xtask/src/control_components.rs)) refuses, outside each client's
  components file, a platform control held from a `ControlState`'s `enabled`: held right, and never
  showing its work running.

- [`account_page_tests.rs`](../crates/mailcal-bindings/src/account_page_tests.rs) pins the account
  page as it is put together: a refused switch held and saying why, a status said beside it, one
  Review beneath the paused uses, the link pickers, and running work holding every control and
  marking its own, drawn where it is going.
- [`account_work_tests.rs`](../crates/mailcal-bindings/src/account_work_tests.rs) pins the
  queue: a change asked for while another runs waits and runs next, the same work asked twice is
  dropped, and a removal drops what waited for its account.
- [`account_page_mail_tests.rs`](../crates/mailcal-bindings/src/account_page_mail_tests.rs) pins
  the page's mail half: a name the provider holds falls back to the address, a server that cannot
  push offers only the interval, a folder past the watch limit is held, and a running change is
  drawn where it goes.
- [`app_accounts_endpoints_tests.rs`](../crates/mailcal-bindings/src/app_accounts_endpoints_tests.rs)
  pins the servers on the page: what an edit leaves out is said by the rule its save refuses with,
  and a refused save says why and draws the form as it was typed. What the rule refuses is in
  [`config_edit_tests.rs`](../crates/mailcal-account/src/config_edit_tests.rs): a server the account
  has keeps an address, the address book's falling back to the calendar's.
- [`toggle_tests.rs`](../crates/mailcal-bindings/src/account_registry/toggle_tests.rs) pins the
  one decision `set_use` and the page share: the last use, a missing server, colleagues and shared
  mailboxes, a grant that lacks a use, and the page drawn from a real registry.
- [`app_account_page_tests.rs`](../crates/mailcal-bindings/src/app_account_page_tests.rs) drives a
  change through `dispatch`: it runs and the page comes back free, one the core refuses leaves a
  notice, and a mail setting and a name asked for while it runs are both on the page once they
  end.
- Each client's controls are tested where its toolkit runs: Linux's in the GTK widget test
  ([`controls.rs`](../clients/linux/src/ui/controls.rs), the name's row in
  [`sender_name_tests.rs`](../clients/linux/src/ui/settings/sender_name_tests.rs), and a redraw
  waiting on a field only until it has sent, and on a form until its button sent it, in
  [`redraw_tests.rs`](../clients/linux/src/ui/settings/redraw_tests.rs)), Android's in
  [`SettingsAccountsTest.kt`](../clients/android/app/src/test/java/eu/allodia/mailcal/SettingsAccountsTest.kt)
  and
  [`SettingsAccountMailTest.kt`](../clients/android/app/src/test/java/eu/allodia/mailcal/SettingsAccountMailTest.kt),
  each drawing a page the core would answer and asking the core for every change. Apple words the
  record in
  [`AccountPageWordsTests.swift`](../clients/apple/Packages/MailcalKit/Tests/MailcalKitTests/AccountPageWordsTests.swift);
  Windows redraws the page whenever a change starts or ends
  ([`AccountSettingsRulesTests.cs`](../clients/windows/Mailcal.Tests/AccountSettingsRulesTests.cs)),
  and its UI suites drive the switches, the suggested link and the removal
  ([`SettingsAccountsTests.cs`](../clients/windows/Mailcal.UiTests/Suites/SettingsAccountsTests.cs)),
  the name sent once with a pick made meanwhile landing too
  ([`SenderNameTests.cs`](../clients/windows/Mailcal.UiTests/Suites/SenderNameTests.cs)), and a
  redraw waiting on a field until it has sent
  ([`SettingsTypingTests.cs`](../clients/windows/Mailcal.UiTests/Suites/SettingsTypingTests.cs))
  against the running app.
