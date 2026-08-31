# iMIP fixtures

Real meeting invitations, taken off the wire. Driven by
[`tests_invitation_fixtures.rs`](../../../src/tests_invitation_fixtures.rs).

Every other invitation test in this crate builds its iCalendar in Rust, which means it can only
prove the parser handles what the test author already imagined. These files exist because that is
the wrong shape of confidence for a format where the failures are all in the punctuation.

## Where each one came from

| File | Origin | Real? |
|---|---|---|
| `caldav-autoschedule-request.eml` | Stalwart's RFC 6638 auto-scheduler, over the dev harness: `bob@test.local` `PUT` an event naming `alice@test.local` as an `ATTENDEE`, and the server sent this | ✅ captured |
| `caldav-autoschedule-cancel.eml` | the same, after `bob` deleted the event | ✅ captured |
| `gmail-request.eml` | Google Calendar, from the live test account to one of its own `+alias` addresses | ✅ captured |
| `gmail-cancel.eml` | the same, after the organiser cancelled | ✅ captured |
| `outlook-request.eml` | Outlook.com (Exchange), from a live test account to a Gmail test account | ✅ captured |
| `outlook-cancel.eml` | the same, after the organiser cancelled | ✅ captured |
| `exchange-internal-request.eml` | two mailboxes in **one** M365 tenant, organiser and invitee both inside it, so the message never took an internet form | ✅ captured |
| `publish-attachment.eml` | hand-written | ⚠️ authored |

`publish-attachment.eml` is authored because no server emits one on request: a published `.ics`
is a file somebody attached, not a message a scheduler sends. It is also the least interesting
byte-wise, and the *only* thing it has to prove is that the RSVP gate rejects it.

## What each one is for

- **`caldav-autoschedule-request`**; three levels of MIME nesting (`mixed` → `related` →
  `alternative`) with an inline image, a quoted-printable calendar part, and `METHOD:REQUEST`
  carried under `Content-Disposition: attachment`. That last one is why the gate reads the
  **`METHOD` in the body** and not the disposition or the `method=` parameter: a real
  auto-schedule server dispositions a genuine invitation as a file.
- **`gmail-request`**: the belt-and-braces shape: the invitation as an alternative *body* part
  **and** a duplicate `application/ics` the sender attached, so this is the fixture that keeps
  the card and the attachment chip from being confused for each other. Times are
  `DTSTART;TZID=Europe/Amsterdam:…` wall clocks rather than instants, so a dropped `TZID` shows
  up as a two-hour error instead of passing.
- **Every `request` file** folds a `mailto:` URI **across a continuation line, mid-token** —
  `mai` / ` lto:bob@…` in the CalDAV and Gmail captures, and `CN=Alice Test` / ` er:mailto:…`
  in Exchange's. A parser that does not unfold reads a plausible-looking `to:bob@…` that
  matches nobody: an invitation that silently says "you are not invited to this". Three
  independent senders fold it, which is the point: this is not one server's quirk.
- **`outlook-request`**: the sender with the least guessable shape, and the reason this suite
  exists at all:
  - `DTSTART;TZID=W. Europe Standard Time:20260812T140000`. The zone id is a **Windows** name,
    not IANA; unquoted, containing spaces and a full stop. There is no `Europe/…` to look up,
    so the offset can only come from the message's own `VTIMEZONE`, whose `STANDARD` and
    `DAYLIGHT` parts start in the year **1601**. August is CEST, so a correct read is 12:00Z
    and an ignored `TZID` is 14:00Z.
  - The `Subject` is RFC 2047 in **Windows-1252** (`=?Windows-1252?Q?…=97…?=`) while the
    calendar part is base64 **UTF-8**; one message, two charsets, both of which must be read.
  - An **`OPT-PARTICIPANT`** beside the required one, so a card that treats optional invitees
    as absent tallies the meeting wrong.
  - **No attachment at all.** Exchange puts the invitation in `multipart/alternative` and
    attaches nothing beside it, so this is the one fixture whose attachment list must be
    *empty*: the mirror of Gmail's duplicate.
  - `X-MICROSOFT-LOCATIONS` carries a JSON blob containing **escaped commas** (`\,`) and
    quotes, folded across three lines. A property parser that mishandles the escape desyncs
    and eats the properties after it.
  - Its `To:` names the **required** attendee, but it was delivered to the **optional** one's
    alias: only `Delivered-To` says so. Matching on `To:` answers as the wrong person.
- **`exchange-internal-request`**: the one capture with **no delivery header at all**. There
  was no MTA hop to write one: organiser and invitee sit in the same tenant, so `Delivered-To`,
  `X-Original-To` and `Envelope-To` are all absent (so is `Return-Path`), and `To:` is the only
  header naming the recipient. `extract_delivery_recipients` falls through to `To:`/`Cc:` for
  exactly this reason, and this is the only fixture that pins the bottom of that chain; the
  other three all carry a delivery header, so none of them can fail if it breaks. It is also
  the *common* case rather than an exotic one: colleagues in one M365 company inviting each
  other is most corporate meeting mail, and reading it wrong disables the buttons on all of it.
  The capture keeps `X-MS-Exchange-Organization-AuthAs: Internal` as the evidence that it never
  left the tenant (`dkim=none`, and every `Received` hop an Exchange Online mailbox server).
  Its iCalendar is otherwise the same shape as `outlook-request`; same Windows `TZID`, same
  `X-MICROSOFT-*` set, same mid-token `ATTENDEE` fold, which is itself the finding: Exchange
  renders MIME from its MAPI item without inventing a different calendar dialect, so the
  *envelope* is the only thing that differs. Its `multipart/alternative` has no `text/html`
  part, which is why the row expects no attachment.
- **`outlook-cancel`**; Exchange also rewrites the `SUMMARY` on cancellation
  (`Geannuleerd: …`, in the *mailbox's* language, not the organiser's) and bumps `SEQUENCE`
  1 → 2. A reader that keyed off the summary rather than `METHOD` would mis-title the card.
- **Each `cancel` file** is its own `request` cancelled, so `METHOD:CANCEL` is pinned in all
  three senders' shapes rather than assumed to be the request with a different word.
- **No capture names the account's own identity** (`me@test.local`). Each is recognized
  through its own recipient headers; `Delivered-To: alice@test.local`, Gmail's `+invite`
  alias, Exchange's `+optional` one, and the internal capture's bare `To:`: so these also pin
  the zero-configuration alias path (§4 source 2) against real headers rather than a `To:`
  written to make it pass. The four together cover the whole fallback chain: three where a
  delivery header decides it, one where there is none to read.

## What was changed, and what was not

Untouched, because it is the whole point: MIME nesting, boundary strings, header order,
`Content-Transfer-Encoding`s, charsets, `Content-Disposition`s, `Content-ID`s, and the iCalendar
property set including its **line folding**.

Changed:

- **Identities** → `test.local`. For the CalDAV and Gmail pairs, replacements were chosen to be
  the **same length** as what they replaced, so re-folding at the original 75-octet width
  reproduces the original fold points exactly. A shorter replacement would have quietly
  un-folded the attendee line and deleted the property under test. The Outlook pair does not
  hold length constant (Exchange's `ATTENDEE` lines are long enough to fold either way) so
  there the fold point moves, and that a fold still lands **mid-token** was checked after the
  run rather than assumed. Same rule, discharged by inspection instead of by construction.
- **The human-readable bodies** (`text/plain`, `text/html`) → a one-line placeholder in the same
  encoding. They were 40 KB of layout that no parser under test reads.
- **The inline logo** → a 1×1 PNG, keeping the `multipart/related` structure.
- **Trace headers** (`Received`, `DKIM-Signature`, `ARC-*`, `X-Google-*`, SPF) → dropped. They
  carry the sending infrastructure and nothing a parser test needs.

Note that re-encoding a quoted-printable part is not byte-identical to the original: the capture
escaped the calendar's CRLFs as `=0D=0A` on one long logical line, and these files use real
line breaks. Both decode to the same iCalendar. (The first attempt at this produced `\r\r\n` —
Python's encoder escapes a bare CR *and* keeps the LF as a hard break, which is why the
generator now encodes from LF and lifts the breaks afterwards, and why the test asserts on
parsed output rather than on bytes.)

## Regenerating, or adding a sender

The captures are reproducible. The CalDAV pair needs only the dev harness
(`scripts/dev/harness.sh up`): `PUT` an event to `bob@test.local`'s calendar with `alice` as an
`ATTENDEE`, then read `alice`'s INBOX over IMAP; deleting the event produces the cancellation.
The Gmail pair needs the engine repo's Google test account and its `tools/google-oauth`.

The Outlook pair needs only the Microsoft test account and `tools/graph-oauth`: `POST /me/events`
with an attendee on the Gmail account (a `+alias`, so the delivery header carries it) and
`timeZone: "W. Europe Standard Time"`, then read the result from Gmail with `format=raw`;
`POST /me/events/{id}/cancel` produces the cancellation. Only **one** Microsoft mailbox is
needed: a sender, not a pair. (Two are needed to *answer* an invitation, which is why the
engine's `live_calendar_rsvp` takes two tokens; capturing what a sender emits is not that.)
Gmail files these as spam, so search with `includeSpamTrash=true`.

**A Graph account reads the sender's own bytes, not a re-rendering.** It was worth checking
whether `provider-graph` needed a fixture of its own: it fetches raw MIME from
`/messages/{id}/$value`, so a Microsoft-account user's parser sees whatever that returns. It
returns the **original message**; verified by capturing a Google-sent invitation from the
Microsoft test mailbox and finding Google's `Message-ID`, Google's intact DKIM signature and
Google's boundary format, with only Microsoft's `Received` hops prepended. So the shapes above
are what every account type sees, and no `$value` fixture is needed.

`exchange-internal-request` needs **two mailboxes in one M365 tenant**: the one case a pair of
consumer accounts cannot reach, because a plus-alias self-invite leaves the tenant and comes
back as ordinary internet mail (five `Received` hops and a DKIM signature). Sign both into
`tools/graph-oauth` with separate `GRAPH_TOKENS` files, confirm the `tid` claim matches and the
`oid` claims do not (a second `login` in the same browser silently re-uses the first account's
session and hands back its tokens), then `POST /me/events` from one naming the other and read
the result from the recipient with `/messages/{id}/$value`.

**What that capture settled:** Exchange renders MIME from its MAPI item without inventing a
different calendar dialect: the iCalendar is the same shape as `outlook-request`, down to the
`X-MICROSOFT-*` set and the mid-token `ATTENDEE` fold. The difference is entirely in the
envelope, and it is the one that matters: **no delivery headers at all**. The earlier worry
that a tenant-internal message might carry X.500 `/o=ExchangeLabs/…` addresses instead of
`mailto:` did not survive contact with one; it does not.

## Privacy

Nothing here is anyone's real mail. The CalDAV pair comes from synthetic harness accounts; the
Gmail and Outlook pairs from throwaway test accounts, scrubbed as above and grepped for
residual identifiers.

`exchange-internal-request` is the one exception, and deliberately so: no throwaway account can
produce it, because it requires two mailboxes inside one tenant. It was captured in a tenant the
developer administers, from a meeting created and cancelled for the purpose, with both
identities, the tenant domain, the Defender correlation id, and every `Received`/antispam
header replaced or dropped before it was committed. Do not repeat that against an employer's or
a client's tenant: the capture sends real mail that their journaling, DLP and retention will
keep, whatever this file ends up containing.

Every mailbox was left as it was found; events deleted, messages removed (from Deleted Items
too), counts re-checked against the pre-capture baseline (Outlook inbox 20 and 6 events, Gmail
39 messages; the tenant pair 27/2 and 213/49).
