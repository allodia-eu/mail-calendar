# Shared mailboxes and calendars: cross-platform contract

**Scope.** A store an account's sign-in opens besides its own, for its mail or for its calendars:
a team's or a group's mailbox and calendar, or a mailbox or calendar a colleague shared. How one is
found, chosen, stored and shown. It binds the core and every client. What an account is, is [`accounts.md`](accounts.md); the folder pane is
[`folder-pane.md`](folder-pane.md); the Microsoft permissions are
[`provider-oauth.md`](provider-oauth.md) rule 10.

## 1. The rules

| # | Rule | Why |
|---|---|---|
| 1 | **The server says which mailboxes a sign-in opens.** A JMAP session lists its accounts that are not the person's own (RFC 8620 §1.6.2), and an IMAP server with `NAMESPACE` and `ACL` its shared namespaces (RFC 2342, RFC 4314): a list to choose from. **Microsoft lists none** and answers only for an address the person types, so a Microsoft account offers a field for one, and the server checks it can be opened and is not the account's own mailbox before anything is added. Gmail has no way for a user's own sign-in to open one. The account's own mailbox is never offered. | A person can only choose a mailbox the server opens for them; nothing typed or listed reaches the account unchecked. |
| 2 | **On Microsoft, shared mailboxes and calendars are one use of their own**, a choice beneath mail and calendar as colleagues are beneath contacts (`AccountCapability::SharedMailboxes`, stored as `shared_mailboxes`): offered in setup, on by default, and requesting `Mail.ReadWrite.Shared` and `Mail.Send.Shared` only while chosen and only beside mail, and `Calendars.ReadWrite.Shared` only while chosen and only beside calendar. It is never offered on a personal account, which has no organisation to share a store within, and an account stored before the choice existed is not used for it, so no existing account is asked for a permission. It is withheld only when the grant holds neither permission, so a grant from before calendars keeps its shared mailboxes and finds no calendar in a store it adds until it signs in again. Switching it on later signs in again for it; switching mail or calendar off takes that much of every shared store of the account, and switching it, or both, off removes them ([`accounts.md`](accounts.md) rule 13). | A permission is asked for only by someone who wants what it opens, and a strict organisation can still admit the app without it. |
| 3 | **Nothing is chosen for the person.** An account's page in Settings → Accounts asks the server when it opens (`account_page_opened`), and again when the account changed since it was asked (a use switched, its mailbox or calendar connected); the core keeps the answer, so a page drawn again does not ask again. It draws one section, a switch per store chosen or listed (`AccountPage::shared`, `SetSharedStore`), named by its address with the server's label and what it holds beside it (`SharedStoreHolds`: mail, calendar, or both), and on Microsoft the address field (`AddSharedStore`, checked against the server before anything is added). The section is absent for an account that cannot open one: an account used for neither mail nor a calendar that opens them, such as a contacts account, and a Microsoft account not used for them. | A shared mailbox can be large and is often someone else's; downloading it unasked spends the person's disk and the server's patience. |
| 4 | **A chosen shared mailbox is stored with the account whose sign-in opens it** (`[[shared_store]]`: its handle, label, address and what it holds), and has no sign-in of its own. Choosing one pins the account's id, since the shared mailbox's id is derived from it (`<account id>#shared:<handle>`). | The sign-in is the account's; a second copy of it would be a second thing to expire. |
| 5 | **A chosen shared mailbox is a mail account of its own wherever mail is**: a top-level row in the folder pane after the account that opens it, with its own folders; All Inboxes; the From picker; search; sync settings and signatures. It **sends as its own address**, under the name its server holds for it, taken the first time it connects without one (`needs_sender_name`), so composing from it, or replying to a message in it, has From set to it, and the sent copy is filed in its own Sent. One without an address sends as the account that opens it. | A shared mailbox is read and answered as the team; mail from it signed as one person is the mistake a shared mailbox exists to prevent. |
| 6 | **It connects with the sign-in of the account that opens it**, and is dialled again whenever that account is: at launch, at a retry and after its password or sign-in is replaced. An IMAP shared mailbox draws on the account's connections rather than a budget of its own; a Microsoft one is the same token addressing `/users/{address}` rather than `/me`. Its rights are the server's: a folder shared read-only refuses writes there, as the server does. | A server limits connections per sign-in, not per mailbox. |
| 7 | **Switching one off deletes what the device holds of it** (`set_shared_mailbox`), after the person confirms; nothing changes on the server, and it can be switched on again. Removing the account removes its shared mailboxes; moving the account's mail server to another host drops them, since a handle names a mailbox on one server. | The same rule as switching a use off ([`accounts.md`](accounts.md) rule 13). |
| 8 | **A chosen mailbox the server stops listing is kept, and says so** ("No longer shared with this account"), until the person switches it off; and a server that does not answer still shows the chosen ones, so one can be switched off while it is away. When an account's mail and calendar are on two servers and only one answers, what it listed is shown, and a chosen store holding what the other one serves is not called withdrawn. | Access withdrawn for a moment must not delete a team's mail from the device; and an answer nobody got is not "nothing is shared". |
| 9 | **Nothing about a shared mailbox leaves for anyone but its provider**: the address typed on Microsoft goes to Microsoft alone, and an Allodia account's list of mail accounts carries none ([`privacy-policy.md`](privacy-policy.md)). | The mailbox is someone else's as much as the person's. |
| 10 | **One store is one switch, with everything it holds that the account opens.** Each use is asked of the part of the account that serves it, mail of its mailbox and calendars of its **calendar**: a JMAP session's accounts, with the mail and calendars the person may read in each (RFC 8620 §1.6.2, draft-ietf-jmap-calendars), and on a CalDAV server the principals it names whose calendar home the sign-in may read (RFC 3744 §5.8, RFC 6638). A store both answer for under one handle, a JMAP account or a Microsoft address, is **one** store holding both, such as a group's mailbox and calendar; each row says what it holds that the account opens now, so a group's row says mail alone once the account's calendar is switched off. An IMAP server and a CalDAV server name their stores their own way, so an IMAP account lists its shared folders and its shared calendars as stores of their own: nothing in either protocol ties one to the other, and a guess would open the wrong person's calendar. Gmail and Google Calendar have no way for a user's own sign-in to open another person's store: a calendar shared there arrives in the account's own list. | It is how people think of a team's mailbox, and how Outlook opens one: a mailbox and its calendar, not two things to find. |
| 11 | **A chosen store is one account, opening what it holds that the account is used for**: its mail as rule 5 says, and its calendars listed after the account that opens it, holding that store's calendars and none of the account's, with the rights its server gives, so a calendar shared read-only takes no new event. An event made in it is organised as the store's owner. One holding calendars alone is in no mail surface. Switching the account's mail or calendar off takes that much of each store with it and leaves the rest; one left opening nothing goes as rule 7 says, as one does when the server that found it moves to another host. A store the server later lists as holding more, or one added again by its address, gains it: a shared mailbox stored before calendars gains its calendar this way. | A colleague's calendar opened as one's own would put events under the wrong name; and a store that keeps what it held loses nothing the person chose. |
| 12 | **A Microsoft address is checked for mail and for calendars**: adding one asks the account's mailbox (an Inbox probe under `/users/{address}`) and its calendar (`GET /users/{address}/calendar`), and the store holds what answered: a shared mailbox's mail and calendar, a colleague's delegated calendar alone. Its calendar is opened as its owner's (`/users/{address}/calendars`), with the rights the owner gave. A shared mailbox added before calendars gains its calendar when it is added again. | Microsoft lists no store, so the address is the only handle; and what answers is what the person was given. |

## 2. The stored shape

Beside the keys every kind shares ([`accounts.md`](accounts.md) §2), written only when set:

```toml
capabilities = ["mail", "shared_mailboxes", "calendar"]   # Microsoft: the use, rule 2

[[shared_store]]
handle = "support-account-id"    # the server's own: a JMAP account id, an IMAP path,
name = "support@example.org"     # a CalDAV principal, a Microsoft address
address = "support@example.org"  # absent when the server gives none
holds = ["mail", "calendar"]     # what the servers that found it said it holds
```

A shared store is listed as `<account id>#shared:<handle>`. A `[[shared_mailbox]]` table, written
before a store could hold calendars, reads as a store holding mail and keeps its id; it is written
back as a `[[shared_store]]`.

## 3. Per-platform matrix

| Capability | Shared core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| Listing what a JMAP or IMAP sign-in opens, checking a typed Microsoft address, storing the choice | ✅ | — | — | — | — | — |
| The Microsoft use: its mail and calendar scopes asked only while chosen and beside their own use, never on a personal account or one stored before it | ✅ | — | — | — | — | — |
| A chosen one as a mail account of its own: folder pane, All Inboxes, From under its server's name, sending as its address, its own Sent | ✅ | — | — | — | — | — |
| Listing what each store a JMAP or CalDAV sign-in opens holds, one store for one handle, storing the choice, and a chosen one as one account opening its mail and its calendars with their rights | ✅ | — | — | — | — | — |
| "Also mailboxes and calendars shared with you" beneath mail and calendar in a Microsoft setup | — | ✅ | ✅ | ✅ | ✅ | ✅ |
| The Shared mailboxes and calendars section, its rows and what each holds, the line under them and when to ask the server again, decided for the page (`AccountPage::shared`) | ✅ | — | — | — | — | — |
| The Shared mailboxes and calendars section on an account's page, what each store holds beside its label, the Microsoft address field and the use's switch, switching one off confirmed | — | ✅ | ✅ | ✅ | ✅ | ✅ |

The mail surfaces need nothing of a client: each draws the core's account rows, and a shared
mailbox is one of them. A client without the section leaves the use's switch out of its "Used
for" list and offers no setup choice for it, so the account is never asked for a permission with
nothing on screen to use it.

**Verified live** through `crates/mailcal-bindings/tests/live_shared_mailboxes.rs`
(`scripts/dev/harness.sh test`), against the harness's `support` group mailbox, over JMAP and over
IMAP: listed, chosen, synced into its own folders and none of alice's, its name taken from the
server (JMAP), a message sent from it filed in its own Sent under its own address, stored, and
switched off again. On Microsoft 365, by hand on Linux on 2026-10-28 against a work tenant: signed
in with the use chosen, a shared mailbox added by address, its mail synced into a mailbox of its
own and From set to it when composing from it.

Shared calendars are verified live through `crates/mailcal-bindings/tests/live_shared_calendars.rs`,
against the harness's `support` group and the calendar dave shares with alice read-only, over JMAP
and over CalDAV: listed with what each holds, the group's mailbox and calendar as one store over
JMAP, chosen from the page's switch, synced as an account of its own with its owner's calendars and
rights, in the mail surfaces only when it holds mail, stored, switched off, and the group's
calendars gone with the account's calendar while its mailbox stays.
