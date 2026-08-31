# What each client ships

The at-a-glance truth for the whole product: every user-facing capability, and which clients have
it. It is the document copy is measured against, so a store listing, a release note or a README
sentence may never claim more than a row here does.

Add or change a capability and this moves in the same change. A shortfall is a row, never a silence:
`⬜` says the work is planned and not done, and that is more useful to a reader than an absent line.
The per-surface rules behind each row are the contracts in [`README.md`](README.md).

Where each client stands today, and the per-platform completion status
[`docs/pledge.md`](docs/pledge.md) points at: a platform still catching up on a row is a gap to
close, never a reason to charge for it.

**Linux ships from this release.** Its column below is what the Flatpak does; the rows still marked
⬜ are the ones it does not claim, and a listing may not out-run them.

Legend: ✅ shipped · 🚧 in progress · ⬜ planned · — not applicable.

| Capability | Shared core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| Reactive Rust ↔ native binding (dispatch → snapshot) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Account setup wizard (IMAP / SMTP / CalDAV) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Auto-detect server settings from email (JMAP probe +`_jmap._tcp` SRV · autoconfig · ISPDB · IMAP/SMTP SRV [implicit-TLS + STARTTLS] · host-DNS MX · CalDAV follow-on), implicit-TLS **and STARTTLS** connections, untrusted-approval gate | ✅ | ✅ | ✅ | 🚧 | ✅ | ✅ |
| Microsoft 365 accounts: browser OAuth sign-in (PKCE), mail read/sync + mail actions (read/flag, archive/move, delete) + send + calendar read/sync + write (default calendar) | ✅ | ✅ | ✅ | 🚧 | 🚧 | ✅ |
| Microsoft re-consent prompt: a reconnect banner when a connected Graph account is missing a needed permission (calendar, or mail write/send, e.g. consent revoked server-side); one tap re-grants the full scope set, clearing both ([docs](docs/provider-oauth.md)) | ✅ | ✅ | ✅ | 🚧 | 🚧 | ✅ |
| Expired sign-in prompt: a credential that is *gone* (expired/revoked grant, refused password) is reported as such and offers to sign in again, instead of being badged as a server outage; fires for Microsoft, Google, JMAP **and** password accounts ([docs](docs/provider-oauth.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ (all four remedies are wired; Microsoft/Google/OAuth JMAP are runtime-verified, stored-secret repair is not) |
| Google accounts (Gmail + Google Calendar, **native APIs**, not IMAP/CalDAV): browser OAuth sign-in (PKCE), mail read/sync + mail actions (read/flag, archive/move, delete) + send + calendar read/sync + write (primary calendar); **Early Access**-gated ([`docs/provider-oauth.md`](docs/provider-oauth.md)) | ✅ | 🚧 | 🚧 | 🚧 | 🚧 | ✅ |
| JMAP accounts (RFC 8620/8621): one secret (password *or* API token), mail read/sync + read bodies + send + calendar read | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| JMAP **OAuth sign-in**: "Sign in with your provider", discovered entirely from the standards (RFC 9728 → 8414 → 7591 → PKCE) with no per-provider code; offered only where the server advertises it, and the password/API-token path always remains ([`docs/jmap.md`](docs/jmap.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| JMAP **re-authentication**: an expired or revoked JMAP sign-in is renewed from the prompt, in place, against the account's own persisted grant; the account keeps its mail, folders and settings instead of needing a remove-and-re-add ([`docs/jmap.md`](docs/jmap.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Multi-account + unified inbox + account switcher | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Remove an account: per-account credential storage + in-app removal (right-click / long-press the account) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Your account list on every device**: with an Allodia account signed in, the mail accounts you add on one device are offered on the next: the address, the server names and ports, never a password and never your mail. An offer opens the ordinary setup screen with the typing done, so you enter the password once per device. Offered on the first screen and in Settings → Accounts ([docs](docs/onboarding.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Sign in to, create, manage or delete an **Allodia account**: the account for the services Allodia runs, in its own Settings category. It is not a mail account: it holds no mailbox, appears in no switcher, and its token cannot reach anyone's mail. Present only in a build carrying the registration, so a build from source has no such screen ([docs](allodia_license/entitlement.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Per-account folder sidebar | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Folder pane: every account's folders on screen at once, each account expandable independently of what's selected and **remembered across restarts**; per-folder unread counts (the server's, so they cover mail older than the synced window) and an All Inboxes total; role icons for Inbox/Drafts/Sent/Archive/Junk/Trash ([docs](docs/folder-pane.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Folder pane: **drag its edge to widen it** for long account addresses, remembered across launches (desktop only; a drawer has no width to drag) | — | ✅ | — | ✅ | — | ✅ |
| Message list: flat + threaded | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Folder and message rows expose a named native action a screen reader can invoke | — | ✅ | ✅ | ✅ | ✅ | ✅ |
| Threaded conversation view: the whole conversation (received + your Sent replies, across folders): inline on desktop, a conversation reading screen on Android + archive conversation | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Reading view (sanitised HTML, inline CID images, recipient headers, remote-image gating, retry) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Offline-first bodies: synced message bodies auto-download after **every** sync (add, refresh, IDLE push), as wide as the transport allows, so opens are instant, the synced window reads offline, and body search covers it. Each account chooses its own cap (2/5/10 MB or no limit), defaulting to 2 MB on a phone and no limit on a computer | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Reading **pane**: 3-pane desktop layout (sidebar \| list \| reading) | ✅ | ✅ | ✅ (iPad) | ✅ | — | ✅ |
| Reading-view actions: reply / reply-all / forward / archive / delete | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Archive/delete **advances the reading pane** to the next message down (the one above, at the end of the list) instead of emptying it, where a pane exists beside the list; the iPhone still pops back | — | ✅ | ✅ (iPad) | ✅ | — | ✅ |
| Email attachments: list/save/open received files (open via the OS default handler, or the OS's own viewer where there is none, Quick Look on iPhone/iPad) + attach files in composer | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Rich HTML composer: new / reply / reply-all / forward (editable To/Cc/Bcc); **inline in the reading pane** on macOS + Windows, full-screen/modal on iPhone, iPad + Android | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Composing keeps the mailbox live: the composer replaces the reading pane rather than blacking it out; clicking another message mid-draft prompts Discard / Keep editing | — | ✅ | — | ✅ | — | ✅ |
| Composer **From** account picker + app-level default send account | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Composer formatting: bold/italic/underline, font size, **text + highlight colour**, bullets and numbering **nested to any depth** (Tab / Shift+Tab), tables with **add/remove row + column** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Composer toolbar in your language: the editor chrome reads the shared catalog ([docs](docs/composer-security.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Open `mailto:` links: the OS offers Allodia Mail & Calendar as a mail app, and a tapped mail link opens the composer pre-filled (To/Cc/Bcc/Subject/Body, decoded in the core; every other header a link may name is dropped; [`docs/composer-security.md`](docs/composer-security.md) Gate 12) | ✅ | ⬜ | ⬜ | ✅ | ✅ | ✅ |
| Reply/forward quoting: original quoted below (indented / line + header, app default with previews + opt-in per-message override) | ✅ | ✅ | ✅ | 🚧 | 🚧 | ✅ |
| Signatures: a reusable library (rich text **+ an inline logo**), a per-account signature for new messages and for replies/forwards, seeded editable into the composer, auto-swapped when From changes, overridable per message; sent as `cid:` parts ([docs](docs/signatures.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Mail actions: read/unread, flag, archive, delete, trash | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Mark as spam / not spam: **reported to the provider**, which trains its filter, not only filed under Junk; the verdicts offered come from the transport's own capability, and a provider that cannot be told still gets the message filed ([docs](docs/reporting.md)) | ✅ | ⬜ | ⬜ | ⬜ | ✅ | ✅ |
| AI assistant access (MCP server): opt-in, off by default, desktop-only; read and act on your mail from an MCP client over a local socket (Windows: a named pipe), with an empty account allow list and direct send behind its own toggle ([docs](docs/mcp.md)) | ✅ | ✅ | — | ✅ | — | ✅ |
| Configurable swipe actions: Trash / Archive / Star per direction, with an undo toast | ✅ | ✅ | ✅ | ✅ | ✅ | ⬜ |
| Calendar: agenda + create / delete event | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar: write affordances gated on per-row `can_write` (a read-only row offers no delete; New event disabled without a writable calendar) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar: write-status feedback (spinner while saving, warning on unconfirmed, retry) | ✅ | ✅ | ✅ | 🚧 | ✅ | ✅ |
| Calendar: **time grid** (day / 3-day / work-week / week), now line, week numbers, jump-to-today | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar: all-day / multi-day banner, capped with a per-day "+N" that expands | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar: **month grid** (6×7 cells, "+N more") | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar manager: per-calendar visibility + colour override, persisted ("Agenda's beheren") | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Display settings: first day of the week, 12/24-hour clock (mail **and** calendar), **light or dark appearance** (follows the system by default, or pick one), calendar horizon | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar, pinch-to-zoom: hours, days, and **diagonal** (both at once) | — | ✅ | ✅ | ✅ | ✅ | ⬜ |
| Calendar, **continuous day strip**: free horizontal scroll *across* weeks (trackpad/wheel), pinned hour ruler, coming to rest on a **day** at every zoom rather than paging by the week; a wheel notch asks for eased travel, so a mouse and a trackpad scroll alike ([`docs/calendar.md`](docs/calendar.md)) | — | ⬜ | ⬜ | ✅ | ⬜ | ⬜ |
| Calendar, trackpad / mouse-wheel scrolling of the grid: hours **and** days, *within* the week (`Shift`+wheel pans days on a plain mouse) ([`docs/calendar.md`](docs/calendar.md)) | — | ✅ | ⬜ | ✅ | — | ⬜ |
| Calendar: `< Today >` header navigation, stepping by the visible span (the work week steps a week) | — | ⬜ | ⬜ | ✅ | ⬜ | ✅ |
| Calendar: the shape and horizon you left it in, restored on launch | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar: your diary on screen from the store at launch, filled without opening it, so it is there offline | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar: refreshes periodically while the app remains open, without requiring another visit to the tab | ✅ | ⬜ | ⬜ | ⬜ | ⬜ | ✅ |
| Calendar: create with a **calendar picker** (grouped by account) · all-day · notes · location | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar: tap an event to **view** its detail and **edit** it (title, time, notes, location) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar: an event's **attendee list** (who called it, and who accepted / declined / answered maybe / hasn't replied), on the detail **and** read-only in the editor | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar, **drag on the grid**: draw out a new slot on empty space (opening the editor prefilled), and move or resize an event by dragging it. Only your own: your appointments and the meetings you organise; a repeating event asks *This event · All events* first ([`docs/calendar.md`](docs/calendar.md)) | ✅ | ✅ | ✅ | ⬜ | ✅ | ✅ (create only) |
| Calendar: deleting one occurrence of a repeating event asks *This event · All events* first, so cancelling one Tuesday does not cancel the standup; an agenda row stands for the whole series and says so ([`docs/calendar.md`](docs/calendar.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar: editing a whole repeating event **warns first** where the account's server would throw away what you changed on a single occurrence (a time you moved, a name you gave it) and says which of the two it would lose ([`docs/calendar.md`](docs/calendar.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar, a repeating event's rule read back **in full** on its detail: every second Tuesday, on the last day of the month, until 3 June, rather than one word ([`docs/calendar.md`](docs/calendar.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar: opening one occurrence of a repeating event shows **that occurrence's** date and time, not the first one's, so a September standup no longer reads as August's ([`docs/calendar.md`](docs/calendar.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar: **editing** one occurrence of a repeating event asks *This event · All events* first, so changing one Tuesday does not rewrite every Tuesday ([`docs/calendar.md`](docs/calendar.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Meeting invitations, an invitation email shows a card above the body: who is asking, when, where, how everyone answered, how many other things are in your calendar then, and that day's grid ([`docs/invitations.md`](docs/invitations.md)) | ✅ | ✅ | ✅ | ✅ | ✅ |✅ |
| Meeting invitations: the invitation's own `.ics` body part no longer shows as a junk attachment; a calendar file the sender really attached still does | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Meeting invitations: a meeting you have not answered is drawn as a **hold** (dashed, hatched) on the grid, month and agenda, and its spoken label says "Awaiting your response" | ✅ | ✅ | ✅ | ✅ | ✅ |✅ |
| Meeting invitations: a meeting you **declined** leaves the calendar, on every provider; the invitation email is the way back to it, and still shows your answer | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Meeting invitations: **answer** one (Accept / Maybe / Decline), with a note to the organiser and an "email them" toggle where the account's server supports them | ✅ | ✅ | ✅ | ✅ | ✅ |✅ |
| Meeting invitations: an invitation that arrived only as **email** is put on your calendar when you answer it, and the reply is **emailed to the organiser** where the calendar server will not send it, so an IMAP + CalDAV account can answer at all ([`docs/invitations.md`](docs/invitations.md)) | ✅ | ✅ | ✅ | ✅ | ✅ |✅ |
| Meeting invitations: when the organiser has re-sent a meeting, the **older** email says it is out of date and stops offering an answer, so you cannot agree to times that have changed | ✅ | ✅ | ✅ | ✅ | ✅ |✅ |
| Meeting invitations: when the calendar server says it **could not** pass your answer to the organiser, you are told and offered the email instead, naming who it would go to; the choice can be remembered per account ([`docs/invitations.md`](docs/invitations.md)) | ✅ | ✅ | ✅ | ✅ | ✅ |✅ |
| Contacts: a unified A–Z list of people from every account's address books (CardDAV · JMAP), with search and a detail view; **read-only** ([`docs/contacts.md`](docs/contacts.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Sender **avatars**: a coloured monogram beside every sender, replaced by their photo where an account's address book has one ([`docs/avatars.md`](docs/avatars.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Contacts: the same person in several accounts is **one** row (joined on a shared address, never on a name), and says so: "In N accounts" + the accounts named in the detail | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Composer **recipient autosuggest**: ranked addresses from synced contacts **and** from people you have written to, so it works on an account with no address book | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Composer recipients as **pills**: each finished address is its own removable chip, the one being typed stays editable text | — | ✅ | ✅ | ✅ | ✅ | ✅ |
| Search: `from:`/`to:` DSL, unified across accounts; **newest first**, every folder but Trash ([`docs/search.md`](docs/search.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Search scope filter: narrow an active search to the current folder (or, in the unified view, the inboxes) and back to all mail | ✅ | ⬜ | ⬜ | ⬜ | ✅ | ✅ |
| Search says **how far back it looked**: results state the sync depth they cover ("Searching the last 3 months"), with a link to change it, so an empty answer never reads as "there is no such message" ([`docs/search.md`](docs/search.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Configurable **per-account** fetch depth and **message size** + on-demand folder sync + progress | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Sync progress: a download you started gets a bar **under** the list (never over it, so no row moves); a background pass gets a subtle status-line note naming which accounts are catching up, through **both** phases (its folders, then the message bodies that follow) and only once mail is actually arriving ([docs](docs/sync-progress.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Unified, categorised **Settings screen** (language, appearance, time zone, conversation grouping, swipe actions, per-account fetch depth + message size + sync behaviour, quote style, default send account, reset, About) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Per-mailbox sync behaviour: push (IMAP IDLE, capability-gated) / 15–120 min polling | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Background mail delivery: syncs when the app isn't foregrounded (desktop: while running; Android: WorkManager ~15 min; iOS/iPadOS: BGAppRefreshTask). Mobile cadence is **best-effort**: Android asks the user to exempt it from battery optimisation, without which Doze defers a pass by hours ([docs](docs/background-sync.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| New-mail notifications: local, sender + subject, from the background sync | ✅ | ⬜ | ✅ | ⬜ | ✅ | ✅ |
| Connection resilience: auto-reconnect after network loss (working Refresh / Try again), offline banner + per-account outage badge | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Time-zone-aware display + device-zone change prompt | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Localisation: English, Nederlands, Deutsch, Français, Español, Italiano, Português; the choice drives **dates** as well as copy (weekday/month names follow the app language, not the host's format locale; [`docs/timestamps.md`](docs/timestamps.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Diagnostic file log: rotating (1 MB × 3, privacy-safe), attachable for support | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| In-app **Diagnostics** (Settings): log viewer, share/export with privacy note, size + copy path, persisted DEBUG toggle ([docs](docs/logging.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Consented product analytics: first-boot welcome screen, **opt-in, default off**; install id minted only at consent; payload structurally cannot carry mail content; "see exactly what we send" preview; Settings → Privacy one-click withdrawal + backend erasure ([`docs/analytics.md`](docs/analytics.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Showcase (store-screenshot) mode: `MAILCAL_SHOWCASE` boots a seeded in-memory dataset, no real account ([`docs/debugging.md`](docs/debugging.md)) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Scripted screenshot capture: 6 screens (incl. calendar and the meeting-invitation card) × 7 languages, store-valid sizes; Android covers Play's phone **and** 7-/10-inch tablet slots, booting the tablet AVD itself (`scripts/dev/showcase.sh`) | — | ✅ | ✅ | ✅ | ✅ | ✅ |

### Notes on the matrix

**Swipe actions reach a mouse too.** The gesture answers to touch, pen and a precision touchpad: a
two-finger trackpad swipe on macOS or Windows reveals the configured action, exactly as a touch
swipe does on a phone, but not to a mouse, and plenty of desktops have neither a touch screen nor a
precision touchpad. So the row's context menu is an equal path rather than a fallback: it runs the
same deferred dispatch and raises the same undo bar, so a mouse-only user gets identical behaviour,
undo included.

**Known gap, on all four clients:** a swiped action is held in memory until its undo window elapses,
so killing the app inside that window loses it: the message is neither archived nor visibly still
there until the next sync puts it back. The window is seconds long, so this is accepted; it is not
silent.

**Archiving a conversation never moves a sent copy out of Sent.** It moves every message on the
thread except those in the Sent folder, so the archived conversation still shows both sides when it
is reopened.

**Unread mail is bold on the subject and the sender**, and on nothing else (not the preview line,
the date or the badges), because bolding every line of an unread mailbox distinguishes nothing.

Not yet started, and on the roadmap: the `JurisdictionGate`, and AI features of our own.

> Keeping this matrix current is a hard rule: see [`AGENTS.md`](AGENTS.md).

