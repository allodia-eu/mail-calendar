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
   sign-in waiting on the person, where a fresh press supersedes the attempt still out
   ([`provider-oauth.md`](provider-oauth.md) rule 13); once the browser has returned, its
   completion is a change like any other.
3. **When the surface the control sat on closes, the progress goes on where the person lands.** A
   dialog that closes on Save hands its progress to the surface beneath it (the calendar's write
   badge, the send banner, the contacts list's "Saving…" line) on every surface it can close onto.
4. **Nothing that can reach the network or the host's store is called from a UI thread.** Work a
   press starts is dispatched and returns at once.
5. **What a change came to is the core's to decide and the client's to word.** A change that
   worked says nothing: the page shows its result. One that did not leaves a notice, raised once,
   which stands until it is closed or the page's next change starts.
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
`Surface::AccountPage` and `Surface::Settings` signal; its changes are dispatched (`Intent::AccountPage`) and return at once.
`Surface::AccountPage` is raised when one starts, when one is queued behind it and when each ends, and when a server answers what a page asked it; it moves nothing but the pages, the accounts list and the questions they share with other screens, so a client redraws only those. What a change stores and something outside the page reads raises `Surface::Settings` as it is stored. A use's switch carries `blocked`, why the
core would refuse pressing it, asked of the registry's `toggle`, which `set_use` acts on too, so a
switch is enabled exactly when nothing blocks it and no change runs; and `status`, what is wrong
with the use, said beside it whether or not it can be pressed. Only the page that started a change marks its
control running, drawn at the value it is moving to rather than the one it is leaving; every other
page holds its controls without a spinner. While the account is used for mail the page carries
its `sender_name` and its `mail` settings too (fetch depth, message size, push or poll, the
interval, the watched folders), each a control of its own, a switch to push drawn with the
folders it will watch: a change to one is stored before it ends, and what follows from it (telling
the provider the name, reconciling the mail to a new depth or size) runs on afterwards as sync work
the page does not wait on. For an account whose servers
can be edited the page carries its `servers`: the form, drawn from the edit being saved while a
save runs, from the edit the last save refused (its password included, so one field is corrected
rather than all retyped), and otherwise from what is stored; and Save, held too while
`account_endpoints_gap` says the fields leave something out, which the page says beside it. The
fields are held with Save, without its spinner, so nothing typed during a save is drawn over when
it ends. Its `shared` mailboxes are switches and an address field of the same kind: switching one
or adding one is a change on the page, and the server's answer is the core's to ask for and keep.
Its `sign_in` section is decided the same way: which ways in its server takes is asked by the core
as the page opens and kept, the section's line says how the account signs in or that its server
refused the stored sign-in, and a password typed into its field is a change on the page
(`UsePassword`), its Save running until the password has connected or been refused.
An account's browser sign-in is the core's too (`MailcalApp::browser_sign_in`), from the press
to the account it changes, whether its page's route, a use that asks its provider or a notice
started it: `SignIn` starts it in place of any still out, and the core asks the server to start
it, asks the question a switch would raise (`Asking`, [`accounts.md`](accounts.md) rule 18), waits
on the browser (`InBrowser`) for at most five minutes, asks a Nextcloud server itself whether the
person has finished, and completes it as a change on the page (`Completing`). A client opens the
browser once for each attempt, listens where it returns and hands back what came
(`SignInBrowserEnded`); it keeps no step, timer or handle of its own. The page draws it from the
section's `browser`: the control that started it running while the core starts it, for two
seconds after it sends the browser out (`OPENING`, the time a browser takes to come up), and while
it completes it; a line saying it waits on the browser with its Cancel (`CancelSignIn`); and the
password field held while it is out. Between those, the control can be pressed again, which
starts a fresh sign-in, as after a tab closed by accident. What it came to is the page's notice, a
failure or a use the provider still withholds, and a notice it answers is drawn running exactly
when that control is, and says when it failed. A separate server's browser sign-in, at its own
provider or with Nextcloud (`SignInSeparateServer`), is the same sign-in drawn on the separate
server's section: its `sign_in_control` runs as the route does, its `browser` draws the line and
the Cancel, Connect and Skip are held while it is out, and what it came to is the section's
`failed`, as Connect's is.
The question that allows a calendar or address book onto a server found on another site is the
core's too (`account_server_change`), whichever surface opened it: Allow is a change on the page
(`AllowServerChange`), the question stays open while it runs with "Not now" waiting for it, says
itself when the new server did not connect, asks again in place when a lookup replaced it, and
closes once the account waits on none. A client re-reads it on the Settings and Notices signals
and keeps only which question is open.
How the account is shared with the person's other devices is the page's `sharing`, for an account
they can hold while someone is signed in to an Allodia account, and moving it is a change on the
page (`SetSharing`). What the other devices have to say is the core's too
(`MailcalApp::allodia_devices`), read again on `Surface::OtherDevices`, which a pass raises as it starts and ends and which moves nothing else, and on `Surface::AccountPage` for each Keep: the core runs the pass whenever what it compares may have moved,
one asked for while another runs running once more after it, and a pass that ends after the
sign-in it ran for has gone says nothing. It is `None` until a pass has answered, and says `nothing_found` once one answered with nothing, on which Settings draws no card and the first-run card says there are no accounts. The card draws `checking` while one runs, and each
question's Keep is the same change on its account's page, running and held as one; what did not
take is said on the control and the question, as `failed`.
The Allodia account's sign-in and sign-out are the core's as well (`MailcalApp::allodia_card`,
read again on `Surface::Allodia`). The sign-in is the one browser sign-in the core holds, started
as `AllodiaIntent::SignIn` from Settings, the first-run card, a notice or another card, and opened
and handed back by the same driver as an account's; the card draws it from `browser` with its
Cancel. Sign-out is `AllodiaIntent::SignOut`, run off the UI thread, its button running meanwhile
and the service's own sign-out page named once in `end_session`. Whether someone is signed in or
their sign-in has ended is the card's `state`, what a sign-in or sign-out came to its `failure`,
and the first-run card's progress, its way back included, its `onboarding`.
The Allodia account's subscription is the core's too (`MailcalApp::allodia_subscription_card`,
read again on `Surface::Allodia`), kept for the account signed in and forgotten with it. The page
that draws it sends `AllodiaIntent::RefreshSubscription` when it opens, and a desktop again when
its window comes back from the checkout browser; a phone again when the app returns to the front.
A read asked for while one runs runs once more after it, since the one out may predate what
prompted the ask. Each write (`StartCheckout`, `CancelSubscription`,
`Resubscribe`, `SwitchInterval`) is an `AllodiaIntent` the core refuses unless the card offers it,
and its control runs, every other held, from the press until the read that follows it lands; what
it came to is the card's `outcome`, and a payment page it handed back its `checkout_page`, opened
once per id. Buying from a phone's own store stays the client's, since no Rust can call StoreKit
or Play.
The page's `notice` carries an id that grows with each one, so a client says each notice once. A removal's notice is the
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
| An account's page: its shared mailboxes drawn from `account_page`, switched and added as page changes | ✅ | ✅ | ✅ | ✅ | ✅ |
| An account's page: its Sign-in section drawn from `account_page`, a typed password sent as a page change | ✅ | ✅ | ✅ | ✅ | ✅ |
| A server found on another site: the question drawn from `account_server_change`, Allow sent as a page change | ✅ | ✅ | ✅ | ✅ | ✅ |
| A separate server's form, on the page and as setup's step: drawn from `account_page`, Connect and Skip sent as page changes, its browser sign-in run by the core (`SignInSeparateServer`) | ✅ | ✅ | ✅ | ✅ | ✅ |
| An account's browser sign-in, from its page, a use or a notice: run by the core (`browser_sign_in`), the browser opened once per attempt and its return handed back | ✅ | ✅ | ✅ | ✅ | ✅ |
| An account's sharing and the other devices' card: drawn from `account_page` and `allodia_devices`, a change sent as a page change, the pass run by the core | ✅ | ✅ | ✅ | ✅ | ✅ |
| The Allodia account's sign-in and sign-out: drawn from `allodia_card`, run by the core as `AllodiaIntent`s, the sign-in as its one browser sign-in | ✅ | ✅ | ✅ | ✅ | ✅ |
| The Allodia account's subscription: drawn from `allodia_subscription_card`, read and written by the core as `AllodiaIntent`s | ✅ | ✅ | ✅ | ✅ | ✅ |

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
- [`account_page_shared_tests.rs`](../crates/mailcal-bindings/src/account_page_shared_tests.rs)
  pins the shared mailboxes: the server asked once, and again only when the page opens or the
  account changed, an older answer never replacing a newer one, the rows and the line under them,
  and a running switch or add drawn where it goes.
- [`account_page_sign_in_tests.rs`](../crates/mailcal-bindings/src/account_page_sign_in_tests.rs)
  pins the Sign-in section: each kind's line and routes once its server has answered, a refused
  password typed where the page takes it, a password being tried held and running, and the
  server asked again when the page opens while an answer about a sign-in since changed is
  dropped;
  [`app_account_page_sign_in_tests.rs`](../crates/mailcal-bindings/src/app_account_page_sign_in_tests.rs)
  drives a typed password through `dispatch` and pins that it is never printed.
- [`app_server_change_tests.rs`](../crates/mailcal-bindings/src/app_server_change_tests.rs) pins
  the server-change question: an allow under way drawn with the change it allows and pressed once,
  a refused one said by the question with the account where it was, and one a lookup replaced
  moving nothing and asking the new change.
- [`account_page_separate_tests.rs`](../crates/mailcal-bindings/src/account_page_separate_tests.rs)
  pins the separate server's form: which uses need it and the login it starts from, the server's
  way in offered only for the uses it was asked about, and a Connect or Skip running drawn with its
  own control while the last failure waits;
  [`app_account_page_separate_tests.rs`](../crates/mailcal-bindings/src/app_account_page_separate_tests.rs)
  drives both through `dispatch`: placed under the use that needs it, a refused Connect said on
  the form with the login tried, and Skip switching the use off.
- [`account_page_browser_tests.rs`](../crates/mailcal-bindings/src/account_page_browser_tests.rs)
  pins the browser sign-in's steps: a fresh one superseding the one out and its server's late
  answer dropped, the question before the browser carrying its answer, a completion nothing
  supersedes or cancels, a wait ending on its cap only if nothing moved it, and a Nextcloud grant
  kept for its account's next sign-in and withdrawn by another's;
  [`app_account_page_browser_tests.rs`](../crates/mailcal-bindings/src/app_account_page_browser_tests.rs)
  drives one through `dispatch`: the browser returning where the stored grant was registered, a
  return from another request, a superseded attempt's return dropped, a cancel, a browser that
  did not open, one left until the cap, the notice it answers running and then saying it failed,
  and a Nextcloud server asked by the core with a grant for somebody else withdrawn.
- [`allodia_devices_tests.rs`](../crates/mailcal-bindings/src/allodia_devices_tests.rs) pins the
  other devices' card: nothing drawn before a pass has answered or once one found nothing, a pass
  asked for during another run once more after it, one that ends after a sign-out saying nothing,
  and a sharing change or a removal answering the questions about its account;
  [`app_allodia_devices_tests.rs`](../crates/mailcal-bindings/src/app_allodia_devices_tests.rs)
  drives it through the app: no card or control while nobody is signed in, a pass when the
  bookkeeping arrives, Keep pausing the account through `dispatch`, and no control for an account
  the other devices cannot hold.
- [`allodia_subscription_facts_tests.rs`](../crates/mailcal-bindings/src/allodia_subscription_facts_tests.rs)
  pins what the subscription section shows: who is billing and which store pages are offered, a
  status nobody can name granting nothing, renewal and grace, no write the service did not permit,
  and a running write holding every control;
  [`allodia_subscription_card_tests.rs`](../crates/mailcal-bindings/src/allodia_subscription_card_tests.rs)
  pins one read at a time, a write held until the read after it, an older read landing late
  dropped, and nothing landing for an account that has left.
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
