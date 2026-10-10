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
| 3 | **Colleagues are a choice of their own**, beside contacts, on a Microsoft or Google account an organisation administers: the organisation's directory is bound only when the account is used for `colleagues`. A **personal** account offers no colleagues at all, never "needs permission" for them: whether it is personal is asked of the provider at every sign-in (Graph's `/organization`, Google's `userinfo`) and stored with the account (`affiliation`), and an account stored without it is asked at its next connect. Before a sign-in, an address at a provider's personal domain (`gmail.com`, `outlook.com`, `hotmail.co.uk` and the like) is offered no colleagues either. A Google grant without `userinfo.email` cannot ask, and is offered colleagues as before. | It is the permission a strict organisation is most likely to refuse, and it is a different thing from the person's own address book. A personal account has no directory: Microsoft never grants it the scope, so a choice there would wait on a permission no sign-in can give, and Gmail would read an empty one. |
| 4 | **A capability the account is not used for is never opened**: no connection, no sync, no watch. | Opening it anyway would spend a connection and a permission on a surface the person chose not to have. |
| 5 | **Mail decides for an account used for mail; otherwise, anything decides.** An account used for mail connects when its mailbox does, and its mailbox's failure is the account's. An account without mail connects when anything it is used for connects, and reports the first failure (calendar, then contacts) when nothing does. | A mailbox that cannot be reached is an account that cannot be reached. A calendar-only account must still be able to say its server is unreachable or its sign-in has expired. |
| 6 | **An account without mail is in no mail surface**: not the folder pane, the account switcher, the From picker, the sync settings, the signatures or the assistant's account list ([`folder-pane.md`](folder-pane.md) rule 22, [`mcp.md`](mcp.md) rule 2). Its calendars and contacts appear where every account's do. | A row that opens onto no mailbox, or a From address that cannot send, is a dead end. |
| 7 | **An account without mail learns its connectivity from its calendar**, or from its contacts when it has no calendar, and feeds the same "unreachable" and "sign in again" states a mailbox does. An account with mail keeps learning it from its mail alone. | Otherwise it could never say anything is wrong; and a calendar failure beside a working mailbox should empty the calendar, not badge the account. |
| 8 | **An account is named by its address wherever a person reads it**, never by its id: a calendar row carries `account_address`, and a client heads its calendar groups with it. | An id is `address@provider-host`, which is not something a person should read. |
| 9 | **An account's id is stable across edits of the settings it was derived from.** It is derived from those settings until it is pinned (`id`), and a pinned id wins. A standards account without a mailbox is `username@dav:host`. | Preferences, signatures, links and the store are keyed by the id; an edit that changed it would turn one account into a different one. |
| 10 | **A Microsoft or Google sign-in asks only for what the account is used for**, and a use whose scope the grant withholds is not opened, mail included: an account whose grant withholds mail opens as one without mail ([`provider-oauth.md`](provider-oauth.md) rule 10). Signing an existing account in again keeps its pinned id, its links and, unless the sign-in chose again, its capabilities. | A person who wants a calendar should not be asked for their mail, and an organisation that approved only some scopes should still be able to admit the app. |
| 11 | **Settings → Accounts lists every account**, mail or not, from one core snapshot (`accounts_snapshot`): its address, its kind, each use its kind can offer in one of three states (**on**, **off**, **needs permission**: chosen, and the provider has not granted it), and its links by address. An empty snapshot means the last account is gone, and the client returns to first-run setup. | Four clients deciding which accounts exist, or what state a use is in, would decide it four ways. |
| 12 | **A link fills a use the account is not used for itself**, and names an account that is: a mail account's `calendar` names a CalDAV calendar, its `contacts` an account used for contacts, and a calendar account's `mail` one of the mail accounts whose calendar it is, implied when there is only one. Any other stored link, dangling ones included, reads as none. A link is set only to one of the entry's `link_candidates` (`set_account_link`), and naming a calendar's mail account from the calendar links that mail account to it as well. **A candidate is suggested, never linked**, when the calendar server schedules as the mail account's address (`calendar-user-address-set`, RFC 6638 §2.4.1), from either end (`link_candidates.suggested`); the server is asked once per registration, when Settings first reads the snapshot, and Settings is signalled when it named addresses. Clearing a calendar's mail link unlinks its mail account when there is only one, and a second mail account linking a calendar leaves it sending through the first. **Removing an account clears every link to it** from the accounts that hold one, and the confirmation names them (`linked_from`). | A link is acted on without asking again, so one that no longer makes sense must stop counting at once; and a link left behind would attach itself to a different account added later under the same id. |
| 13 | **Switching a use off stops it and deletes what the device holds of it at once** (`set_account_capability`); the provider's permission stays as it was, because an app cannot narrow a grant, and the copy says so. **Switching a use on** opens it when the account can, and otherwise changes nothing and says what is missing: the provider's permission (`NeedsConsent`, answered by signing in again for it) or a server (`NeedsEndpoint`). The last of mail, calendar and contacts cannot be switched off: the account is removed instead. The choice is stored with the account's id pinned. | Leaving a use's data behind after the person switched it off keeps what they asked to be rid of; and an account used for nothing is one nobody can find again in any surface. |
| 14 | **Editing an account's servers is test, then apply** (`update_account_endpoints`): the new servers are dialled first, and nothing changes unless they connect and are stored. The account keeps its id, uses, links, settings and signatures. A use whose server moved to **another host** has what the device holds of it deleted and synced again; a new port, security, login or password keeps it. An edit that would leave a use without its server, or make the account one already set up, is refused. | A typo in a server name must not cost a working account; and mail from one server kept under another's name is mail that server never had. |

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
keep `granted_scopes` beside the requested `scopes`, and both keep `affiliation`: `"personal"`, or
`{ organization = "<id>" }`, a Microsoft tenant id or a Google Workspace domain. A capability name this build does not know is
kept and counts for nothing, so an account a newer build wrote stays readable and keeps what it
said.

The core owns the shape (`mailcal_account::AccountShape`); no client reads or writes these keys.

## 3. Per-platform matrix

| Capability | Shared core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| Stored capabilities, pinned id and links read and kept | ✅ | — | — | — | — | — |
| Only the capabilities an account is used for are opened | ✅ | — | — | — | — | — |
| Sign-in asks for the chosen capabilities' scopes only, and opens what was granted | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| Signing in again, or adding a capability, keeps the account and its mail | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| A standards account without a mailbox connects its calendar and contacts | ✅ | — | — | — | — | — |
| An account without mail is kept out of the mail surfaces | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar groups headed by the account's address | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Every account, its uses' states and its links in one snapshot | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| Removing an account clears the links to it, and the confirmation names them | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| Setting a link to one of the offered accounts | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| A link suggested when the calendar server schedules as the mail account's address | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| Switching a use on or off, its data deleted when off | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| Editing a password account's servers, tested before applied | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| Setting up an account without mail, or with an address book of its own | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| Choosing capabilities, and linking accounts, in Settings | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| Linking a new account as the last step of setting it up ([`onboarding.md`](onboarding.md)) | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |

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

Link suggestions are checked against the harness by
`crates/mailcal-bindings/tests/live_link_suggestions.rs` (`scripts/dev/harness.sh test`), in the
shape of the `stalwart-linked` dev account: Stalwart lists each user's address in their principal's
`calendar-user-address-set`, so alice's calendar is suggested for her mailbox, and her mailbox for
it, while bob's calendar is offered without being suggested.

## 4. Known gaps

- **Only Linux creates an account without mail.** `account_config_toml` and the standards
  sign-in (`ImapLoginRequest`) take the person's choice of uses and a CardDAV URL. Linux offers
  both: its found card on the IMAP route and for a domain with a calendar and address book and no
  mail server, and its manual form as an address-book field and a "Calendar and contacts" type.
  The other clients' setup screens offer neither, so there every account is used for mail.
- **An account without mail cannot be removed, nor its password updated, except on Linux.**
  Apple, Windows and Android draw "Remove account" and the account's Settings card only on surfaces
  rule 6 keeps it out of (the folder tree, a Settings → Accounts built from the mail-only
  `sync_settings`), so the "sign in again" prompt rule 7 raises for it points at a card that is not
  there. Linux lists every account from `accounts_snapshot`, removes any of them, and edits a
  password account's servers and password on its page, which is where the expired-sign-in prompt
  sends it.
- **Rule 8 holds only for the calendar.** Every client names an account in the connection and
  sign-in banners and in a contact's provenance ("Also in", the account under each value) by
  looking its id up in the switcher rows, and falls back to the id. An account without mail is not
  in those rows, so there it reads `alice@dav:cloud.example`. The fix is the calendar's: the core
  hands the address over with the id.
- **A suggestion compares the mail account's own address only.** An alias the mailbox sends as
  is not compared, so a calendar server that lists only the alias suggests nothing. Linux names
  the suggestion under the link's picker, with a button that links it; the other clients do not
  draw the page yet.
- **Links are stored but not acted on.** An invitation still files into, and answers from, the
  account whose mail it arrived in, and "save contact" still writes to the account in view.
- **A use has three states, on, off and needs permission.** "Not offered" (the server has none)
  and "failing" have no representation yet: a JMAP session that lacks a chosen capability binds
  nothing for it and still reads as on. Linux draws "needs permission" on the account's page and
  in the list, and signs the account in again to ask for it; the other clients do not draw
  Settings → Accounts from the snapshot, so there a use a Microsoft or Google grant withholds is
  still closed without a word (Microsoft's calendar aside, which raises its re-consent prompt).
- **A JMAP account without mail still opens its session through the mail provider**, because that
  is what reads which calendars and contacts the account has; it binds no mail from it.
- **Analytics cannot yet tell a calendar-and-contacts account apart**: such an account counts under
  `has_imap`. A `has_dav` key needs the analytics relay to accept it first, since its context is a
  strict whitelist and an unknown key rejects the whole batch ([`analytics.md`](analytics.md)).
- **Only a standards account that signs in with a password has editable servers.** A JMAP
  account's session URL and an OAuth standards account's servers are not offered for editing:
  signing in again is their route, and a new token source over the stored grant during a test
  dial could rotate a refresh token the account then loses.
- **Switching colleagues off deletes the account's own contacts too**, until its next contacts
  sync brings them back: the store keeps directory cards beside the account's own with no line
  between them, so it can forget only both.
- **One calendar per CalDAV account.** The account binds the calendar discovery finds first, or the
  one it names.
