# The pledge to open-source users

**What Allodia absolutely promises to everyone who uses, builds, or contributes to the open-source
application.** This is a permanent public commitment, written to leave no room for interpretation:
the list below is a floor that only ever grows. The prices users see live on the website.
Pledged 2026-08-24.

## What "the application" means, exactly

The shared Rust product core, the product-neutral sync engine beneath it (already public, MPL-2.0),
and the native clients for these five platforms: **macOS**, **iOS/iPadOS**, **Windows**,
**Android**, and **Linux**. The pledge binds each of these five platforms today, and binds a
further platform from the day Allodia ships a client for it. It does not by itself promise that a
client for any further platform will be built.

Per-platform completion status for each row below is the [README](../README.md) capability matrix.
A platform still catching up on a row is a gap to close, never an opening to charge: the moment a
listed capability ships on a platform, it ships free there.

## The five promises

1. **Everything in the list below is free and open source, permanently.**
2. **A feature that has shipped free is never reclassified as paid.** Not renamed, not "moved to a
   plan", not degraded to push an upgrade. This holds for every item below and for every feature
   that ships in the open tree after this pledge: each one joins this list on the day it ships.
3. **The free application carries no commercial-use restriction.** Using it at work, alone or as a
   whole team of individual users, violates nothing. Companies pay only for capabilities that exist
   solely in the paid tiers (listed under "What may ever be paid"), never for permission.
4. **The open repository stands alone.** It compiles, tests, and runs with no reference to anything
   closed: no build step that reaches a private repository, no feature that needs an Allodia server
   to function. A from-source build keeps working even if Allodia and every Allodia service
   disappear.
5. **What the user's own mail server provides natively, the app does for free.** This rule decides
   every future boundary question; it is spelled out under "What may ever be paid".

## The free feature list

**Accounts and protocols**

- Connecting mail, calendar, and contacts accounts over IMAP, SMTP, JMAP (RFC 8620/8621), CalDAV,
  and CardDAV, plus the native Microsoft 365 (Graph) and Google (Gmail + Google Calendar) adapters,
  each with browser OAuth sign-in (PKCE) where the provider requires it.
- JMAP sign-in with a password or API token; JMAP OAuth discovered purely from the standards
  (RFC 9728, RFC 8414, RFC 7591, PKCE) with no per-provider code; in-place re-authentication when a
  grant expires, keeping the account's mail and settings.
- Email-first account setup: server settings detected from the address alone (JMAP probe, DNS SRV,
  autoconfig, the ISPDB, MX fallback, CalDAV discovery), with the approval gate on untrusted
  results ([`account-autodetect.md`](account-autodetect.md)).
- Any number of accounts; the unified inbox and account switcher; in-app account removal;
  credentials stored in the platform keystore.
- An expired or revoked sign-in reported as exactly that, with a working sign-in-again or
  re-consent prompt ([`provider-oauth.md`](provider-oauth.md)).

**Mail**

- The folder pane: every account's folders at once, server-side unread counts, role icons
  ([`folder-pane.md`](folder-pane.md)).
- The message list, flat or threaded, and the whole-conversation view across folders.
- The reading view: sanitised HTML, inline images, remote-image blocking, recipient headers
  ([`rendering-security.md`](rendering-security.md)); bodies downloaded after every sync so the
  synced window reads offline.
- Attachments: list, save, and open received files; attach files when composing.
- The rich HTML composer: new, reply, reply-all, forward; editable To/Cc/Bcc with recipient
  autosuggest and address pills; bold, italic, underline, font sizes, text and highlight colour,
  nested lists, tables; the From account picker with an app-level default; quoting styles; the
  editor chrome in the app language ([`composer-security.md`](composer-security.md)).
- Opening `mailto:` links as the system's mail app.
- Signatures: a reusable library with rich text and inline logos, two per-account slots, swapped
  when From changes, overridable per message ([`signatures.md`](signatures.md)).
- Mail actions: read/unread, flag, archive, delete, trash, mark as spam; configurable swipe actions
  with undo.
- Search: the `from:`/`to:` query language, unified across accounts, newest first, with the scope
  filter and the statement of how far back it looked ([`search.md`](search.md)).
- Sender avatars: a monogram, or the photo from an account's address book
  ([`avatars.md`](avatars.md)).

**Calendar**

- Agenda, the day / 3-day / work-week / week time grid, and the month grid; all-day and multi-day
  banners; pinch zoom; week numbers; jump to today; the layout you left restored on launch; the
  diary readable offline ([`calendar.md`](calendar.md)).
- Event create, view, edit, and delete, with the calendar picker, all-day, notes, and location;
  attendee lists with each answer shown; drag to create, move, or resize your own events;
  per-calendar visibility and colour; write affordances honestly gated on what the server allows.
- Meeting invitations end to end: the card above the message, unanswered meetings drawn as holds,
  Accept / Maybe / Decline with a note to the organiser, replies emailed where the calendar server
  cannot send them (so IMAP + CalDAV accounts can answer at all), superseded invitations marked out
  of date, and an undeliverable answer disclosed instead of dropped
  ([`invitations.md`](invitations.md)).

**Contacts**

- The unified A to Z people list from every account's address books (CardDAV and JMAP), search, the
  detail view, and one merged row per person across accounts. Read-only in the current release, as
  [`contacts.md`](contacts.md) records; when editing ships, it ships free.

**Delivery and background work**

- The live runtime: IMAP IDLE push where the server offers it, polling every 15 to 120 minutes
  where it does not, configurable per mailbox.
- Background delivery while the app is not foregrounded, and local new-mail notifications, on every
  platform's native mechanism ([`background-sync.md`](background-sync.md)), including the promise
  recorded there: **local background sync stays free and always-on for everyone.**
- Connection resilience: auto-reconnect after network loss, the offline banner, per-account outage
  badges; sync-progress reporting ([`sync-progress.md`](sync-progress.md)); per-account fetch
  depth.

**Agent access**

- The local MCP server on desktop (macOS and Windows today): opt-in, off by default, empty account
  allow list, direct send behind its own toggle, everything over a local socket
  ([`mcp.md`](mcp.md)).

**Settings, languages, diagnostics**

- The full Settings surface ([`settings.md`](settings.md)): light/dark appearance, first day of the
  week, 12/24-hour clock, time zones with the device-change prompt, conversation grouping, swipe
  actions, per-account sync behaviour, quote style, default send account, reset.
- All seven shipped languages: English, Dutch, German, French, Spanish, Italian, Portuguese, with
  dates following the app language ([`timestamps.md`](timestamps.md)).
- The privacy-safe rotating diagnostic log and the in-app Diagnostics screen
  ([`logging.md`](logging.md)).

**Allodia account**

- Signing in to an Allodia account, and keeping your list of mail accounts the same on every device
  you use: for each account the address and the server settings, and never the password, which
  stays in that device's own keystore and is entered once per device.
- This group is different from every other one above in two ways, both stated here rather than
  discovered later. It needs a service Allodia runs, so it is the one group missing from a build
  that carries no Allodia registration, on the same absent-is-supported rule that decides the
  Google and Microsoft sign-in routes. And while the app's half of it is GPL like the rest of the
  application, the service it talks to is a service: that code is not published.
- Everything else on this list works with no Allodia service in the path, which is what promise 4
  protects. Free here means free permanently, on promise 2, exactly as it does above.

## What may ever be paid

Only services that run on infrastructure Allodia operates, and business capability that has no
single-user equivalent. The boundary rule, applied per provider: **what the user's own mail server
provides natively, the app does for free; a price may sit only where a service Allodia runs is
required.** Needing such a service is a precondition for charging, never a reason: a service
Allodia runs is free unless it is named in the table below, and one named in the free list above
can never move down to it (promise 2). Cross-device account sync is the standing free example.
The canonical paid example is send-later: JMAP has scheduled submission in the protocol
(`sendAt`), so on a JMAP account the feature is free core functionality; IMAP has no such
capability, so there it needs a server Allodia operates to hold and submit the message, which is
the paid version.

Every paid service has a free counterpart in the open build, so promise 4 stays true:

| Paid service | What every open build keeps, free |
|---|---|
| Real-time push on mobile (an Allodia relay wakes the device; the signal is content-free) | Local background sync, always on ([`background-sync.md`](background-sync.md)) |
| Send-later on providers whose protocol lacks it | JMAP-native `sendAt`; best-effort client-side scheduling |
| Hosted AI, when it ships (metered per the suite entitlement model) | The local MCP server; the AI posture beyond that is decided when AI lands, within these promises |

The business tier adds centralized deployment and administration, CRM/ERP integrations, and support
with prioritised (never guaranteed) feature-request handling.

**Naming: paid offerings are named as services, never as an app edition.** No "Pro", no "Premium":
an edition name makes the free app read as the lesser one, which promise 1 says it is not.

## Licensing and repository policy

- **The application** (this core and all five clients) is **GPL-3.0 with a CLA**, so contributions
  can also ship in Allodia's store builds (which take the proprietary side of the dual grant, as
  Apple's store terms require). Reusable libraries stay **MPL-2.0**, like the engine.
- **The code that speaks to Allodia's own services** (the account sign-in exchange, the entitlement
  read, and whatever later drives an Allodia-run service) lives in one clearly marked
  **source-available directory** (`allodia_license/`) in the same public repository. The client
  screens that draw those services stay GPL in each client's own tree, and are absent from a build
  carrying no Allodia registration, exactly as the Google and Microsoft screens are; the directory
  holds the part that would otherwise be a second implementation of Allodia's own protocol. It is
  published to be read and audited, not reused: using it needs a current subscription, and it may
  not be redistributed or built into anything else
  ([`allodia_license/LICENSE.md`](../allodia_license/LICENSE.md)). This is the one part of the
  repository that is not free to use, and it holds no capability from the list above.
  The directory is **excluded from the default build**: a GPL build never links it, contains no
  Allodia-account surface, and never depends on an Allodia service. The free counterparts in the
  table above are all GPL in the open tree, and a paid entitlement never resides on the device
  Publicly funded work is always in the GPL tree, never in the source-available directory.
- **Server-side code** (the services behind the paid tier) is closed source, in its own repository.
- The open build is **unbranded by default**: neutral name, icon, and application id. The Allodia
  name and marks are reserved by a trademark policy beside the license, never by a license clause;
  a fork is rebranded by omission.

## Sovereignty constraints on paid services

A paid service is an external dispatch like any other. Each one follows the pattern the analytics
relay established ([`analytics.md`](analytics.md), "Sovereignty scope"): operated by Allodia in the
EU, destination fixed at build time, and payloads that are content-free or end-to-end encrypted.
The push relay never sees mail content, the settings vault holds only ciphertext, and platform push
transports (APNs/FCM) carry nothing readable. Every service passes the `JurisdictionGate` or earns
its own dated, condition-bounded carve-out. **No hosted service ships before
[`privacy-policy.md`](privacy-policy.md) describes it**: the current policy promises that no
Allodia backend holds user content, and that promise is kept by design, not weakened. The policy
gains the service's exact data handling first, in every locale.

## Enforcement

When a change touches the free/paid boundary:

1. It must satisfy all five promises. A change that moves an existing free feature behind a paywall
   is rejected outright, whatever it is called.
2. This document, the capability matrix, and the affected feature's contract doc are updated
   in the same change; user-visible changes write their changelog fragment.
3. A new paid service documents its free counterpart in the table above and its sovereignty posture
   in its own doc before it ships.
4. A new free feature needs no edit here: promise 2 covers it automatically. Editing this list to
   remove or narrow an item is not a change, it is a breach.
