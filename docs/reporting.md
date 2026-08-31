# Reporting a message: cross-platform contract

Marking a message as spam is a **report to the provider**, not a folder move. Both file the
message under Junk; only one of them trains the filter that decides where the *next* one lands.
The difference is invisible from the row, which is why it is written down here rather than left
to each client to rediscover.

The verb is the engine's (`Engine::report_message`), reached through the core's
`Intent::MarkAsSpam` / `Intent::MarkAsNotSpam`. A client sends the same intent it always did.

## The report files the message: never move it as well

Every transport files the message as part of the report: to the account's Junk for a junk or
phishing verdict, back to the Inbox for not-junk. They differ only in *who* moves it: Graph and
Gmail do it server-side and cannot be told not to, while IMAP and JMAP need the destination
named. The core resolves that destination either way (RFC 6154 role, conventional-name fallback).

So a client must not follow a report with a move of its own, and the core must not either. A
second move files the message twice.

## What a transport can express is read, never assumed

`Capabilities::mail_report` is `None` for a provider that cannot report at all, and otherwise
carries which of the three verdicts it has and how much it tells us back.

| Transport | Junk | Not junk | Phishing | Evidence |
|---|:---:|:---:|:---:|---|
| IMAP | ✅ | ✅ | ✅ | convention (`$Junk` / `$NotJunk` keyword) |
| JMAP | ✅ | ✅ | ✅ | convention (RFC 8621 `$junk` / `$notjunk`) |
| Graph | ✅ | ✅ | ✅ | **acknowledged** (`reportMessage` answers) |
| Gmail | ✅ | ✅ | ❌ | convention (`SPAM` label) |

**Gmail has no phishing verdict at all**, so an adapter asked for one refuses rather than filing
it as junk. A client that offers "Report phishing" builds that item from the capability, never
from a constant; the same rule the calendar's per-row `can_write` follows.

## What a client may say about it

`ReportEvidence` is the difference between a claim we can back and one we cannot. Only Graph
answers whether a report was accepted; the other three set a flag or a label and the protocol
offers no way to ask what the server did with it.

So no client may say **"reported to your provider"**, or anything else asserting the provider
acted, unless the account's evidence is `Acknowledged`. Naming what the user did ("Mark as spam",
"Report phishing") is always fine: that describes their action, not the server's.

Today no client says anything at all, which is why `ReportEvidence` is not yet on the FFI. It is
the field to reach for when one wants to.

## A provider that cannot report still files the message

When `mail_report` is `None`, or has no such verdict, the core falls back to moving the message
itself. The user asked for it out of their inbox; a provider that cannot be told is no reason to
leave it there. This is the path the debug fixtures and the showcase engine take.

## Per-platform

| Platform | Mark as spam | Mark as not spam | Report phishing | Where |
|---|:---:|:---:|:---:|---|
| Android | ✅ | ✅ | ⬜ | `clients/android/.../MailRows.kt` |
| Linux | ✅ | ✅ | ⬜ | `clients/linux/src/ui/mail_actions.rs` |
| macOS / iOS / iPadOS | ⬜ | ⬜ | ⬜ | n/a |
| Windows | ⬜ | ⬜ | ⬜ | n/a |
| MCP | ✅ (`mark_as_spam`) | n/a | ⬜ | `crates/mailcal-mcp/src/tools` |

## Known gaps

- **Apple and Windows ship no spam affordance at all.** Both carry the catalog keys
  (`action_mark_as_spam` / `action_mark_as_not_spam`) and neither calls them, so the action a
  user has on Android and Linux is simply absent there. The capability matrix claimed otherwise until
  this contract landed.
- **No client offers a phishing verdict**, though three of the four transports have one. Adding it
  means a new intent, and each client's menu item gated on
  `ReportControls::verdicts.phishing`, which is why the capability is plumbed before the UI is.
- **`ReportEvidence` does not reach the clients.** Nothing says anything about a report yet, so
  the field would be a snapshot payload with no reader. The rule above binds the first client that
  wants to speak.
- **`mark_as_not_spam` is not an MCP tool.** No read tool surfaces Junk, so an agent has no way to
  name a message to un-spam ([`mcp.md`](mcp.md)).

## Enforcement

When you change what reporting does:

1. Never pair a report with a move. `mail_ops/report.rs` owns the one path; `tests_report.rs`
   asserts the provider received a report **and no edit**.
2. Gate any new verdict on the capability, in the core *and* in every client that offers it:
   `ReportControls::accept` refuses a verdict the transport lacks, and a client that offered it
   anyway has already lied to the user by the time that error arrives.
3. Keep the copy rule above: no claim the provider acted, over `Convention` evidence.
