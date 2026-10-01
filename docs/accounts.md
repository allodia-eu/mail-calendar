# Accounts: cross-platform contract

**Scope.** What an account is: which of mail, calendar and contacts it is used for, how it is
stored, how it connects, and which surfaces it appears in. It binds the core and every client.
Setting accounts up is [`onboarding.md`](onboarding.md) and
[`account-autodetect.md`](account-autodetect.md); signing in is
[`provider-oauth.md`](provider-oauth.md) and [`mail-oauth.md`](mail-oauth.md).

## 1. The rules

| # | Rule | Why |
|---|---|---|
| 1 | **An account is one sign-in at one provider**, used for a non-empty subset of **mail, calendar and contacts**. Every endpoint in it presents the same credential: one password, or one grant (`[oauth]`, [`mail-oauth.md`](mail-oauth.md) rule 10). Endpoints that need a different sign-in are a second account. | A person thinks of "my Nextcloud" and "my mailbox", not of four endpoints; and a credential shared across accounts is one that expires in several places at once. |
| 2 | **What an account is used for is stored with it** (`capabilities`), and an account that stores nothing means what its kind always meant: a standards account its mailbox, plus its calendar and the address book beside it when it has a calendar endpoint; a JMAP account mail, calendar and contacts as far as its session offers them; a Microsoft or Google account all of them, colleagues included. | So no account stored before the choice existed changes meaning or is asked anything. |
| 3 | **Colleagues are a choice of their own**, beside contacts, on Microsoft and Google: the organisation's directory is bound only when the account is used for `colleagues`. | It is the permission a strict organisation is most likely to refuse, and it is a different thing from the person's own address book. |
| 4 | **A capability the account is not used for is never opened**: no connection, no sync, no watch. | Opening it anyway would spend a connection and a permission on a surface the person chose not to have. |
| 5 | **Mail decides for an account used for mail; otherwise, anything decides.** An account used for mail connects when its mailbox does, and its mailbox's failure is the account's. An account without mail connects when anything it is used for connects, and reports the first failure (calendar, then contacts) when nothing does. | A mailbox that cannot be reached is an account that cannot be reached. A calendar-only account must still be able to say its server is unreachable or its sign-in has expired. |
| 6 | **An account without mail is in no mail surface**: not the folder pane, the account switcher, the From picker, the sync settings, the signatures or the assistant's account list ([`folder-pane.md`](folder-pane.md) rule 22, [`mcp.md`](mcp.md) rule 2). Its calendars and contacts appear where every account's do. | A row that opens onto no mailbox, or a From address that cannot send, is a dead end. |
| 7 | **An account without mail learns its connectivity from its calendar**, or from its contacts when it has no calendar, and feeds the same "unreachable" and "sign in again" states a mailbox does. An account with mail keeps learning it from its mail alone. | Otherwise it could never say anything is wrong; and a calendar failure beside a working mailbox should empty the calendar, not badge the account. |
| 8 | **An account is named by its address wherever a person reads it**, never by its id: a calendar row carries `account_address`, and a client heads its calendar groups with it. | An id is `address@provider-host`, which is not something a person should read. |
| 9 | **An account's id is stable across edits of the settings it was derived from.** It is derived from those settings until it is pinned (`id`), and a pinned id wins. A standards account without a mailbox is `username@dav:host`. | Preferences, signatures, links and the store are keyed by the id; an edit that changed it would turn one account into a different one. |
| 10 | **A Microsoft or Google sign-in asks only for what the account is used for**, and a use whose scope the grant withholds is not opened, mail included: an account whose grant withholds mail opens as one without mail ([`provider-oauth.md`](provider-oauth.md) rule 10). Signing an existing account in again keeps its pinned id, its links and, unless the sign-in chose again, its capabilities. | A person who wants a calendar should not be asked for their mail, and an organisation that approved only some scopes should still be able to admit the app. |
| 11 | **Settings → Accounts lists every account**, mail or not, from one core snapshot (`accounts_snapshot`): its address, its kind, each use its kind can offer in one of three states (**on**, **off**, **needs permission**: chosen, and the provider has not granted it), and its links by address. An empty snapshot means the last account is gone, and the client returns to first-run setup. | Four clients deciding which accounts exist, or what state a use is in, would decide it four ways. |
| 12 | **A link fills a use the account is not used for itself**, and names an account that is: a mail account's `calendar` names a CalDAV calendar, its `contacts` an account used for contacts, and a calendar account's `mail` one of the mail accounts whose calendar it is, implied when there is only one. Any other stored link, dangling ones included, reads as none. **Removing an account clears every link to it** from the accounts that hold one, and the confirmation names them (`linked_from`). | A link is acted on without asking again, so one that no longer makes sense must stop counting at once; and a link left behind would attach itself to a different account added later under the same id. |

## 2. The stored shape

One TOML document per account in the platform keystore, through `AccountCredentialStore`. Beside
each kind's own section (`[imap]`, `[microsoft]`, `[google]`, `[jmap]`) sit the keys every kind
shares, each optional and written only when set, so a document from before them is rewritten byte
for byte:

```toml
id = "alice@imap.soverin.net"          # pinned id; absent, derived (rule 9)
capabilities = ["mail"]                # absent: what the kind always meant (rule 2)

[links]
calendar = "alice@dav:cloud.example"   # the accounts that supply what this one lacks
contacts = "alice@dav:cloud.example"
```

A standards account's `[imap]` is optional; `[caldav]` and `[carddav]` are optional too, and at
least one of the three is present or the document is refused. Its contacts come from `[carddav]`
when present, otherwise from `[caldav]` ([`contacts.md`](contacts.md)). Microsoft and Google also
keep `granted_scopes` beside the requested `scopes`. A capability name this build does not know is
kept and counts for nothing, so an account a newer build wrote stays readable and keeps what it
said.

The core owns the shape (`mailcal_account::AccountShape`); no client reads or writes these keys.

## 3. Per-platform matrix

| Capability | Shared core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| Stored capabilities, pinned id and links read and kept | ✅ | — | — | — | — | — |
| Only the capabilities an account is used for are opened | ✅ | — | — | — | — | — |
| Sign-in asks for the chosen capabilities' scopes only, and opens what was granted | ✅ | — | — | — | — | — |
| Signing in again, or adding a capability, keeps the account and its mail | ✅ | — | — | — | — | — |
| A standards account without a mailbox connects its calendar and contacts | ✅ | — | — | — | — | — |
| An account without mail is kept out of the mail surfaces | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar groups headed by the account's address | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Every account, its uses' states and its links in one snapshot | ✅ | — | — | — | — | — |
| Removing an account clears the links to it | ✅ | — | — | — | — | — |
| Setting up an account without mail | — | — | — | — | — | — |
| Choosing capabilities, and linking accounts, in Settings | — | — | — | — | — | — |

A client ✅ in the mail-surfaces row reads the rows the core already filters; it holds no rule of
its own.

**Verified live** on 2026-10-01 through `crates/mailcal-bindings/tests/live_provider_consent.rs`,
with a person at each consent screen: a Microsoft account signed in for its calendar alone asked
for `offline_access`, `User.Read` and `Calendars.ReadWrite`, opened its calendar and no mail, and
signing it in again to add contacts bound its address book in place; a Google account signed in for
its calendar alone was named from `userinfo` and opened no Gmail; and a Google account signed in for
mail and calendar with the calendar unticked on the consent screen opened its mail and no calendar;
and the same with Gmail unticked instead was named from `userinfo` and opened as an account without
mail, its calendar working.
A Microsoft grant names every scope the app was ever granted for that account, not only those asked
for, so what opens is decided by the choice and never by the grant alone.

## 4. Known gaps

- **Nothing creates an account without mail yet.** Setup offers no calendar-and-contacts account
  and no capability choice, so every account on a device is used for mail; the core paths above
  run only for a stored document that says otherwise.
- **An account without mail cannot be removed, nor its password updated.** Every client draws
  "Remove account" and the account's Settings card only on surfaces rule 6 keeps it out of (the
  folder tree, Settings → Accounts), both built from rows the core filters, so the "sign in again"
  prompt rule 7 raises for it points at a card that is not there. Whatever creates such an account
  gives it a place to be removed and signed in again from, in the same change.
- **Rule 8 holds only for the calendar.** Every client names an account in the connection and
  sign-in banners and in a contact's provenance ("Also in", the account under each value) by
  looking its id up in the switcher rows, and falls back to the id. An account without mail is not
  in those rows, so there it reads `alice@dav:cloud.example`. The fix is the calendar's: the core
  hands the address over with the id.
- **Links are stored but not acted on.** An invitation still files into, and answers from, the
  account whose mail it arrived in, and "save contact" still writes to the account in view.
- **A use has three states, on, off and needs permission.** "Not offered" (the server has none)
  and "failing" have no representation yet: a JMAP session that lacks a chosen capability binds
  nothing for it and still reads as on. No client draws Settings → Accounts from the snapshot yet,
  so a use a Microsoft or Google grant withholds is still closed without a word (Microsoft's
  calendar aside, which raises its re-consent prompt).
- **A JMAP account without mail still opens its session through the mail provider**, because that
  is what reads which calendars and contacts the account has; it binds no mail from it.
- **Analytics cannot yet tell a calendar-and-contacts account apart**: such an account counts under
  `has_imap`. A `has_dav` key needs the analytics relay to accept it first, since its context is a
  strict whitelist and an unknown key rejects the whole batch ([`analytics.md`](analytics.md)).
- **One calendar per CalDAV account.** The account binds the calendar discovery finds first, or the
  one it names.
