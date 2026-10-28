# Shared mailboxes: cross-platform contract

**Scope.** A mailbox an account's sign-in opens besides its own: a team's or a group's mailbox, or
one a colleague shared. How one is found, chosen, stored and shown. It binds the core and every
client. What an account is, is [`accounts.md`](accounts.md); the folder pane is
[`folder-pane.md`](folder-pane.md); the Microsoft permissions are
[`provider-oauth.md`](provider-oauth.md) rule 10.

## 1. The rules

| # | Rule | Why |
|---|---|---|
| 1 | **The server says which mailboxes a sign-in opens.** A JMAP session lists its accounts that are not the person's own (RFC 8620 §1.6.2), and an IMAP server with `NAMESPACE` and `ACL` its shared namespaces (RFC 2342, RFC 4314): a list to choose from. **Microsoft lists none** and answers only for an address the person types, so a Microsoft account offers a field for one, and the server checks it can be opened and is not the account's own mailbox before anything is added. Gmail has no way for a user's own sign-in to open one. The account's own mailbox is never offered. | A person can only choose a mailbox the server opens for them; nothing typed or listed reaches the account unchecked. |
| 2 | **On Microsoft, shared mailboxes are a use of their own**, a choice beneath mail as colleagues are beneath contacts (`AccountCapability::SharedMailboxes`): offered in setup, on by default, and requesting `Mail.ReadWrite.Shared` and `Mail.Send.Shared` only while chosen and only beside mail. It is never offered on a personal account, which has no organisation to share a mailbox within, and an account stored before the choice existed is not used for it, so no existing account is asked for a permission. Switching it on later signs in again for it; switching it, or mail, off removes every shared mailbox of the account ([`accounts.md`](accounts.md) rule 13). | A permission is asked for only by someone who wants what it opens, and a strict organisation can still admit the app without it. |
| 3 | **Nothing is chosen for the person.** An account's page in Settings → Accounts asks the server when it opens (`shared_mailboxes`) and draws a switch per mailbox chosen or listed, named by its address with the server's label beside it when that says something else, and on Microsoft the address field (`resolve_shared_mailbox`). The section is absent for an account that cannot open one, a Microsoft account not used for them included. | A shared mailbox can be large and is often someone else's; downloading it unasked spends the person's disk and the server's patience. |
| 4 | **A chosen shared mailbox is stored with the account whose sign-in opens it** (`[[shared_mailbox]]`: its handle, label and address), and has no sign-in of its own. Choosing one pins the account's id, since the shared mailbox's id is derived from it (`<account id>#shared:<handle>`). | The sign-in is the account's; a second copy of it would be a second thing to expire. |
| 5 | **A chosen shared mailbox is a mail account of its own wherever mail is**: a top-level row in the folder pane after the account that opens it, with its own folders; All Inboxes; the From picker; search; sync settings and signatures. It **sends as its own address**, under the name its server holds for it, taken the first time it connects without one (`needs_sender_name`), so composing from it, or replying to a message in it, has From set to it, and the sent copy is filed in its own Sent. One without an address sends as the account that opens it. | A shared mailbox is read and answered as the team; mail from it signed as one person is the mistake a shared mailbox exists to prevent. |
| 6 | **It connects with the sign-in of the account that opens it**, and is dialled again whenever that account is: at launch, at a retry and after its password or sign-in is replaced. An IMAP shared mailbox draws on the account's connections rather than a budget of its own; a Microsoft one is the same token addressing `/users/{address}` rather than `/me`. Its rights are the server's: a folder shared read-only refuses writes there, as the server does. | A server limits connections per sign-in, not per mailbox. |
| 7 | **Switching one off deletes what the device holds of it** (`set_shared_mailbox`), after the person confirms; nothing changes on the server, and it can be switched on again. Removing the account removes its shared mailboxes; moving the account's mail server to another host drops them, since a handle names a mailbox on one server. | The same rule as switching a use off ([`accounts.md`](accounts.md) rule 13). |
| 8 | **A chosen mailbox the server stops listing is kept, and says so** ("No longer shared with this account"), until the person switches it off; and a server that does not answer still shows the chosen ones, so one can be switched off while it is away. | Access withdrawn for a moment must not delete a team's mail from the device; and an answer nobody got is not "nothing is shared". |
| 9 | **Nothing about a shared mailbox leaves for anyone but its provider**: the address typed on Microsoft goes to Microsoft alone, and an Allodia account's list of mail accounts carries none ([`privacy-policy.md`](privacy-policy.md)). | The mailbox is someone else's as much as the person's. |

## 2. The stored shape

Beside the keys every kind shares ([`accounts.md`](accounts.md) §2), written only when set:

```toml
capabilities = ["mail", "shared_mailboxes", "calendar"]   # Microsoft: the use, rule 2

[[shared_mailbox]]
handle = "Shared Folders/support@example.org"   # the server's own: a JMAP account id, an IMAP path,
name = "support@example.org"                    # a Microsoft address
address = "support@example.org"                 # absent when the server gives none
```

## 3. Per-platform matrix

| Capability | Shared core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| Listing what a JMAP or IMAP sign-in opens, checking a typed Microsoft address, storing the choice | ✅ | — | — | — | — | — |
| The Microsoft use: its scopes asked only while chosen, never on a personal account or one stored before it | ✅ | — | — | — | — | — |
| A chosen one as a mail account of its own: folder pane, All Inboxes, From under its server's name, sending as its address, its own Sent | ✅ | — | — | — | — | — |
| "Also mailboxes shared with you" beneath mail in a Microsoft setup | — | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| The Shared mailboxes section on an account's page, the Microsoft address field and the use's switch, switching one off confirmed | — | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |

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
