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
| 3 | **Colleagues are a choice of their own**, beside contacts, on a Microsoft or Google account an organisation administers: the organisation's directory is bound only when the account is used for `colleagues`. A **personal** account offers no colleagues at all, never "needs permission" for them: whether it is personal is asked of the provider at every sign-in (Graph's `/organization`, Google's `userinfo`) and stored with the account (`affiliation`), and an account stored without it is asked at its next connect. Before a sign-in, an address at a provider's personal domain (`gmail.com`, `outlook.com`, `hotmail.co.uk` and the like) is offered no colleagues either. A Google grant without `userinfo.email` cannot ask, and is offered colleagues as before. **Shared mailboxes** are the same kind of choice beneath mail on a Microsoft account, offered and withheld by the same rules ([`shared-mailboxes.md`](shared-mailboxes.md) rule 2). | It is the permission a strict organisation is most likely to refuse, and it is a different thing from the person's own address book. A personal account has no directory: Microsoft never grants it the scope, so a choice there would wait on a permission no sign-in can give, and Gmail would read an empty one. |
| 4 | **A capability the account is not used for is never opened**: no connection, no sync, no watch. | Opening it anyway would spend a connection and a permission on a surface the person chose not to have. |
| 5 | **Mail decides for an account used for mail; otherwise, anything decides.** An account used for mail connects when its mailbox does, and its mailbox's failure is the account's. An account without mail connects when anything it is used for connects, and reports the first failure (calendar, then contacts) when nothing does. | A mailbox that cannot be reached is an account that cannot be reached. A calendar-only account must still be able to say its server is unreachable or its sign-in has expired. |
| 6 | **An account without mail is in no mail surface**: not the folder pane, the account switcher, the From picker, the sync settings, the signatures, the name step that follows setting an account up ([`sending.md`](sending.md)) or the assistant's account list ([`folder-pane.md`](folder-pane.md) rule 22, [`mcp.md`](mcp.md) rule 2). Its calendars and contacts appear where every account's do. | A row that opens onto no mailbox, or a From address that cannot send, is a dead end. |
| 7 | **An account without mail learns its connectivity from its calendar**, or from its contacts when it has no calendar, and feeds the same "unreachable" and "sign in again" states a mailbox does. An account with mail keeps learning it from its mail alone. | Otherwise it could never say anything is wrong; and a calendar failure beside a working mailbox should empty the calendar, not badge the account. |
| 8 | **An account is named by its address wherever a person reads it**, never by its id: a calendar row carries `account_address`, and a client heads its calendar groups with it. **Where another account has the same address**, Settings names it with what tells them apart (`distinction` on `AccountEntry` and `LinkedAccount`): its kind where no other of them shares it, else its server's host. A link, a link's picker and a removal's confirmation, both the account it removes and those it unlinks, and a prompt or banner asking an account to sign in again read `settings_account_distinguished` ("alice@example.org (CalDAV/CardDAV)"); an account's own row and page already say its kind, and add the host only. | An id is `address@provider-host`, which is not something a person should read; and a mailbox whose calendar was split off from it (rule 18) links to an account a person would otherwise read as itself. |
| 9 | **An account's id is stable across edits of the settings it was derived from.** It is derived from those settings until it is pinned (`id`), and a pinned id wins. A standards account without a mailbox is `username@dav:host`. | Preferences, signatures, links and the store are keyed by the id; an edit that changed it would turn one account into a different one. |
| 10 | **A Microsoft or Google sign-in asks only for what the account is used for**, and a use whose scope the grant withholds is not opened, mail included: an account whose grant withholds mail opens as one without mail ([`provider-oauth.md`](provider-oauth.md) rule 10). Signing an existing account in again keeps its pinned id, its links and, unless the sign-in chose again, its capabilities. | A person who wants a calendar should not be asked for their mail, and an organisation that approved only some scopes should still be able to admit the app. |
| 11 | **Settings → Accounts lists every account**, mail or not, from one core snapshot (`accounts_snapshot`): its address, its kind, each use its kind can offer in one of four states (**on**, **off**, **needs permission**: chosen, and the provider has not granted it; **not offered**: the server has none, as a JMAP session says at every connect), and its links by address. A use the server does not offer counts as one the account is not used for: it cannot be switched on, does not count as the last use, and may be filled by a link. An empty snapshot means no account is left, mail or not, and the client opens first-run setup: at launch, and when the last one is removed. The mailbox snapshot cannot answer that, since an account without mail is in none of its rows. | Four clients deciding which accounts exist, or what state a use is in, would decide it four ways. |
| 12 | **A link fills a use the account is not used for itself**, and names an account that is: a mail account's `calendar` names a CalDAV calendar, its `contacts` an account used for contacts, and a calendar account's `mail` one of the mail accounts whose calendar it is, implied when there is only one. Any other stored link, dangling ones included, reads as none. A link is set only to one of the entry's `link_candidates` (`set_account_link`), and naming a calendar's mail account from the calendar links that mail account to it as well. **A candidate is suggested, never linked**, when the calendar server schedules as the mail account's address (`calendar-user-address-set`, RFC 6638 §2.4.1), from either end (`link_candidates.suggested`); the server is asked once per registration, when Settings first reads the snapshot, and Settings is signalled when it named addresses. Clearing a calendar's mail link unlinks its mail account when there is only one, and a second mail account linking a calendar leaves it sending through the first. **A mail account's calendar link is where its invitations are filed and answered**, and a calendar's mail link the account that emails the answers given from it ([`invitations.md`](invitations.md), "Which calendar an invitation uses"). **Removing an account clears every link to it** from the accounts that hold one, and the confirmation names them (`linked_from`). | A link is acted on without asking again, so one that no longer makes sense must stop counting at once; and a link left behind would attach itself to a different account added later under the same id. |
| 13 | **Switching a use off stops it and deletes what the device holds of it at once** (`set_account_capability`); the provider's permission stays as it was, because an app cannot narrow a grant, and the copy says so. **Switching a use on** opens it when the account can, and otherwise changes nothing and says what is missing: the provider's permission (`NeedsConsent`, answered by signing in again for it) or a server (`NeedsEndpoint`). A use that **needs permission** is drawn switched off, saying the provider has not granted it, and switching it on opens the account's sign-in for it straight away rather than asking the core. The last of mail, calendar and contacts cannot be switched off: the account is removed instead. The choice is stored with the account's id pinned. | Leaving a use's data behind after the person switched it off keeps what they asked to be rid of; and an account used for nothing is one nobody can find again in any surface. |
| 14 | **Editing an account's servers is test, then apply** (`update_account_endpoints`): the new servers are dialled first, and nothing changes unless they connect and are stored. The account keeps its id, uses, links, settings and signatures. A use whose server moved to **another host** has what the device holds of it deleted and synced again; a new port, security, login or password keeps it. An edit that would leave a use without its server, or make the account one already set up, is refused. | A typo in a server name must not cost a working account; and mail from one server kept under another's name is mail that server never had. |
| 15 | **An account already set up is not added again.** Its id is its mailbox, not how it signs in, so a password account and a sign-in account for the same address are one account. `add_account` refuses the second before anything is registered or stored, with `MailcalError::AlreadySetUp` naming the account already there, and a server edit that would make one account another is refused the same way. Each client says so in the person's language (`account_already_set_up`). | Registering the second would replace the first account's credential and providers in place, and its calendar and contacts would disappear with nothing to say why. |
| 16 | **A use that did not connect says why, on the account's page and where the use is shown, while the account itself did.** A calendar or an address book that fails beside a working account keeps its state (on) and carries a problem (`AccountUse.problem`): its server **needs a sign-in of its own** (the account's sign-in may not reach it, [`mail-oauth.md`](mail-oauth.md) rule 10; nothing was sent), **refused the sign-in**, **could not be reached**, or is **paused for a server found on another site** that the person has not yet allowed ([`account-autodetect.md`](account-autodetect.md) rule 14; nothing was sent to it). Each client says it beneath the use's switch in the person's language, with a sign-in of its own for a server that needs one or refused the account's (rule 18) and a **Review** of the server change for a paused one, and the calendar and contacts screens say it again above what they show of that use, which is what was last synced (`ConnectivitySnapshot.calendar_outages` and `contacts_outages`; [`calendar.md`](calendar.md) §4, [`contacts.md`](contacts.md) §3). Nothing offers to change or answer what a use that is not connected shows: the core writes only through a bound adapter, and a write that arrives anyway is not saved. An unreachable use is dialled again by the next retry (Refresh, or coming back online), never twice at once, and on its own: the account's mailbox is connected, so it is not dialled, synced or watched again, because signing in to it on every retry is what makes a provider limit the account's sign-ins. Every dial replaces what the one before it said, except that a paused use stays paused while it still does not connect, since only the person can answer that. A calendar that could not be reached has not reported "no calendar", so the calendar does not claim its week ([`calendar.md`](calendar.md) §4); one that needs its own sign-in, refused the account's or is paused will not arrive by waiting, so its week is answered and the account's page says why. The account's badge stays its mail's (rule 7). | A failure that is only in the log is one nobody can act on, and a calendar that silently stays empty reads as a calendar with nothing in it. |
| 17 | **An account switches how it signs in where its server takes the other way in, and keeps everything else.** `account_sign_in_options` asks the server: signing in through the provider is offered where setup would offer it ([`mail-oauth.md`](mail-oauth.md) rules 1 to 4 and 12; on JMAP, `jmap_oauth_available`), a password where the IMAP server names a password mechanism, or, on JMAP, everywhere setup offers its secret field ([`jmap.md`](jmap.md) rule 3). An account that signs in through its provider can sign in again (`begin_account_sign_in`), from its stored grant and with no second registration. Every route connects with the new credential before storing it, and keeps the account's id, uses, links, settings and mail; a password replaces the grant on every endpoint and a grant replaces the password, and the calendar and contacts rule ([`mail-oauth.md`](mail-oauth.md) rule 10) is applied again over whatever replaced it; a switch to the provider that it would cost the calendar or contacts asks first (rule 18). A credential is used only for the mailbox it was set up for (`use_credential_from`), and a sign-in for another account is refused: one whose server names another account in its token response (`username`, draft-ietf-mailmaint-oauth-public §3.5), or whose session sends as identities none of which is the account's address ([`provider-oauth.md`](provider-oauth.md) rule 14). That refusal is `MailcalError::DifferentAccount`, which each client says in the person's language (`signin_different_account`), on every route that signs in again, Microsoft's and Google's included. Microsoft and Google sign in again through their own consent (rule 10). The "already set up" refusal (rule 15) offers to put the credential just given on the account already there. | Changing how an account signs in by removing it and adding it back loses its settings, links and downloaded mail, and adding it the other way is refused (rule 15). |
| 18 | **A calendar or address book the account's sign-in does not reach becomes an account of its own, linked to it.** An account is one sign-in (rule 1), so a mailbox that signs in through its provider beside a calendar server that takes only a password is two accounts. Which uses a grant reaches is read before the browser opens, from the account that sign-in would store (`unreached` on `ImapLoginStart` and `AccountSignInStart`, by [`mail-oauth.md`](mail-oauth.md) rule 10), so what is said first is what the connect then finds. **Switching a password account to its provider asks first** when it would lose any: keep them on the password, as an account of their own linked to this one (`UnreachedUses::KeepOnPassword`, with the password they connect with now), stop using them here (`StopUsing`, which deletes what the device holds of them), or cancel; nothing is installed without an answer. **Setup and the account's page** ask for that server's password when a use needs a sign-in of its own or refused the account's (`AccountEntry.separate_server` names the uses, the server's host and the login it holds; `add_linked_calendar_account`), or, where that server's own issuer may be sent its token, offer signing in there instead ([`mail-oauth.md`](mail-oauth.md) rule 13). Either way the calendar account is added, and has connected, before the mailbox changes: it is used for exactly the uses that moved, its mail link names the mailbox, and the mailbox stops being used for them and links to it (rule 12). A refused password leaves both as they were, and so does a use that did not connect: that is `MailcalError::UseNotConnected`, naming the use and why (rule 16's problems), which each client says in the person's language (`separate_server_unreachable_calendar` and `_contacts`, or the use's own sign-in line), never as the core's text. An account used for nothing but these gives a slow contacts server 30 seconds rather than the 10 a mailbox waits for, since no mailbox waits on it. | A calendar that worked a minute ago must not be lost to a sign-in change, and a mailbox and a calendar that sign in differently are two sign-ins however the screen draws them. |

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

The mailboxes the account's sign-in opens besides its own that it is used for are a
`[[shared_mailbox]]` table each ([`shared-mailboxes.md`](shared-mailboxes.md) §2).

The core owns the shape (`mailcal_account::AccountShape`); no client reads or writes these keys.

## 3. Per-platform matrix

| Capability | Shared core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| Stored capabilities, pinned id and links read and kept | ✅ | — | — | — | — | — |
| Only the capabilities an account is used for are opened | ✅ | — | — | — | — | — |
| Sign-in asks for the chosen capabilities' scopes only, and opens what was granted | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Signing in again, or adding a capability, keeps the account and its mail | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| A standards account without a mailbox connects its calendar and contacts | ✅ | — | — | — | — | — |
| An account without mail is kept out of the mail surfaces | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar groups headed by the account's address | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| An account sharing its address with another is named with its kind or server | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| A calendar or address book that did not connect is said again on the calendar or contacts screen, with Account settings | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| A switch to the provider that would lose the calendar or contacts asks first: keep them on the password as a linked account, stop using them, or cancel | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| A calendar on a server the sign-in does not reach gets its own password, in setup and on the account's page, as a linked account | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Moving them that does not connect says which use and why, in words, and changes nothing | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Every account, its uses' states and its links in one snapshot | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| A use the server does not offer reads as such, and cannot be switched on | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| First-run setup when no account is left, an account without mail counting | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Removing an account clears the links to it, and the confirmation names them | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| A calendar or address book that did not connect says why beneath its switch | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Adding an account already set up is refused, and said in words | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| The refusal offers the credential just given to the account already there | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Switching between a password and signing in, where the server takes both | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Signing a standards or JMAP account in again from its stored grant | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Setting a link to one of the offered accounts | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| A link suggested when the calendar server schedules as the mail account's address | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Switching a use on or off, its data deleted when off | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Editing a password account's servers, tested before applied | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Setting up an account without mail, or with an address book of its own | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Choosing capabilities, and linking accounts, in Settings | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Linking a new account as the last step of setting it up ([`onboarding.md`](onboarding.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |

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

Switching how an account signs in is checked against the harness's sign-in server by
`crates/mailcal-bindings/tests/live_sign_in_switch.rs` (`scripts/dev/harness.sh test`): a JMAP
account goes from its password to signing in and back under one id, and an IMAP sign-in signs in
again on the client it registered and keeps its grant over a password the server refuses.

Link suggestions are checked against the harness by
`crates/mailcal-bindings/tests/live_link_suggestions.rs` (`scripts/dev/harness.sh test`), in the
shape of the `stalwart-linked` dev account: Stalwart lists each user's address in their principal's
`calendar-user-address-set`, so alice's calendar is suggested for her mailbox, and her mailbox for
it, while bob's calendar is offered without being suggested.

## 4. Known gaps

- **Rule 8 holds only for the calendar, except on Apple and Android.** Linux and Windows name an
  account in the connection and sign-in banners and in a contact's provenance ("Also in", the
  account under each value) by looking its id up in the switcher rows, and fall back to the id. An
  account without mail is not in those rows, so there it reads `alice@dav:cloud.example`. Apple
  and Android look it up in `accounts_snapshot` as well, which names every account. The fix that needs no
  second lookup is the calendar's: the core hands the address over with the id.
- **A suggestion compares the mail account's own address only.** An alias the mailbox sends as
  is not compared, so a calendar server that lists only the alias suggests nothing. Every client
  names the suggestion under the link's picker, with a button that links it.
- **A contacts link is stored but not acted on.** "Save contact" still writes to the account in
  view.
- **"Not offered" is known only once the account has connected.** The session is read at every
  connect and not stored, so an account that has not connected since the app started reads each
  chosen use as on.
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
