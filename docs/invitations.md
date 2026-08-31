# Meeting invitations: cross-platform contract

**Scope.** When a message counts as a meeting invitation, what the card above it shows, how an
unanswered hold and a declined meeting appear on the calendar, and which address counts as "me".
The point is that a support answer like *"open the invitation email and change your answer there"*
must be true on every platform, and an invitation that shows an RSVP on macOS must show one on
Android for the same account.

**Principle.** The core decides **whether there is a card, what it says, and every number on it**;
each client decides how it looks and formats the times. The core is display-tzdata-free: it emits
UTC instants and the host localises them ([`timestamps.md`](timestamps.md)).

## Why an invitation "sometimes has an .ics file and sometimes doesn't"

It always carries the calendar data. What differs is **where in the MIME tree it sits**, and that
one fact explains every difference users notice between senders.

| Source | Where the calendar data lives | `METHOD` | Recipient an `ATTENDEE`? | We show |
|---|---|---|---|---|
| Published `.ics` (e.g. a residents'-association notice) | `text/calendar` with `Content-Disposition: attachment` | **`PUBLISH`** | **no `ATTENDEE` at all** | attachment chip, **no card** |
| Outlook → Outlook | `text/calendar; method=REQUEST` **inside `multipart/alternative`**: no disposition, no filename, `base64`, `iso-8859-1` | `REQUEST` | yes, `PARTSTAT=NEEDS-ACTION;RSVP=TRUE` | card, **no chip** |
| Sabre/CalDAV (RFC 6638 auto-schedule) | same shape, `8bit` | `REQUEST` | yes | card, no chip |
| Gmail | **both**: the body part *and* a duplicate `application/ics; name=invite.ics` marked as an attachment | `REQUEST` | yes | card **and** an `invite.ics` chip |

"No .ics file" means **the calendar part is an alternative *body* part**: a sibling of
`text/plain` and `text/html` (iMIP, RFC 6047 §2.4). A client renders it as a representation *of the
message*, not as a file, so no paperclip belongs on it. This is standards-based iMIP, not an
Outlook-proprietary format. Gmail is belt-and-braces: it also attaches a copy so non-iMIP clients
get something, which is the entire reason it is the one sender showing both a card and a chip.

**Rule.** A `text/calendar` part with **no `Content-Disposition` at all** is an invitation body: it
is consumed into the card and **never** listed as an attachment. A part the sender explicitly
dispositioned as a file keeps its chip. The listing and the by-id download agree, so a suppressed
part cannot be reached another way.

## The RSVP gate: two conditions, and neither alone is enough

1. **A scheduling `METHOD`.** `REQUEST` (plus `CANCEL`, which gets its own wording). `PUBLISH` means
   "informational copy, no reply expected" (RFC 5546 §1.4).
2. **An `ATTENDEE` matching one of the account's own addresses**, aliases included.

A published `.ics` fails both: it is `PUBLISH` *and* carries zero `ATTENDEE` lines, which is why
it can never offer RSVP. But attendee-matching alone is **not** sufficient either: a `PUBLISH` that
happens to list your address must still not offer a reply, because no organiser is waiting on one.

| `METHOD` | An `ATTENDEE` is us | Card |
|---|---|---|
| `REQUEST` | yes | **RSVP**: Accept / Maybe / Decline |
| `REQUEST` | no | **Informational** (a forwarded invitation): details, no reply |
| `CANCEL` | yes | **Cancelled**: say so, and offer to clear the hold |
| `CANCEL` | no | Informational |
| `PUBLISH` | either | **none** (the chip stays) |
| `REPLY`, `COUNTER`, `DECLINECOUNTER`, `ADD`, `REFRESH`, unknown | either | **none**: each needs its own UI to mean anything |

Two details that are easy to get wrong:

- **The `METHOD` inside the body is authoritative, not the `method=` Content-Type parameter**
  (RFC 6047 §2.4). Every real fixture agrees, but the body wins.
- **An unanswered invitation is not a hard conflict.** An Outlook organiser says so explicitly,
  sending `X-MICROSOFT-CDO-BUSYSTATUS:TENTATIVE` beside `INTENDEDSTATUS:BUSY`: hold it tentatively
  until answered, busy once accepted.

The gate is a pure, table-driven function with unit tests
(`crates/mailcal-app/src/invitations.rs`, `invitations_tests.rs`). That is deliberate: a rule that
could only be exercised through a live provider and an open message is a rule nothing verifies.

## A newer invitation retires the older one

An organiser who moves a meeting does not send a diff; they re-send the **whole invitation**, and
both copies stay in the mailbox. The older one goes on offering Accept / Maybe / Decline over times
that are no longer the meeting's, so answering it agrees to a slot that no longer exists.

RFC 5546 §2.1.5 orders the revisions of one `UID` by **`SEQUENCE`**, which an organiser bumps on
every significant change (RFC 5545 §3.8.7.4). From a captured Exchange pair sent 54 seconds apart:

| | first | second |
|---|---|---|
| `UID` | identical | identical |
| `SEQUENCE` | `0` | **`1`** |
| `DTSTAMP` | `…T084143Z` | `…T084238Z` |
| `DTEND` | 14:15 | **14:00** |

**Rule.** A `REQUEST` whose `UID` sits on the account's calendar at a **higher `SEQUENCE`** is
`Superseded`: the card still shows every detail, **says the mail is out of date**, and offers no
answer. The write refuses it as well, and the two are not redundant: the RSVP resolves its event by
`UID`, so it would land on the meeting *as it now is* while the user was reading the old copy's
times. Agreeing to a slot you were never shown is worse than a visible refusal.

**The calendar is the authority, not the mailbox.** Comparing a message against its siblings would
need a `UID`-keyed index over mail, because the two copies are **not threaded**: Exchange sends no
`References` and no `In-Reply-To`, only its own `Thread-Index`, and the two values differ, so
nothing links them without one. The stored event, by contrast, is a lookup the card already makes for
the tally and `my_response`, so this rule costs no extra read.

**`SEQUENCE` alone: the `DTSTAMP` tie-break is deliberately *not* applied here.** That tie-break
orders two *scheduling messages*, and both of those carry a `DTSTAMP`. A stored event carries none;
it has `LAST-MODIFIED`, a different property that moves whenever anything touches the object,
**including our own RSVP write**. Tie-breaking on it would mark an invitation superseded moments
after the user answered it, hiding the very card that reports their answer.

The two ways this can be wrong are not symmetric, so it is conservative by construction: **missing** a
supersession leaves a stale card exactly as it is today, while **inventing** one hides a reply the
organiser is still waiting for. So *no* stored event means **not** superseded: "we have not looked"
is not evidence of staleness, the same distinction `conflicts_known` draws over the same lookup.

## Identity is a set, not one address

An invitation to `info@example.com` on an account whose primary identity is `alice@example.com` is
still an invitation to **me**, and Outlook treats it as one. Matching an `ATTENDEE` against a single
identity silently answers *"you are not invited to this"*, which hides the RSVP the user is
waiting for. A wrong non-match is therefore much worse than a wrong match.

Three sources, in the order they are trusted:

| # | Source | Available to | Configuration needed |
|---|---|---|---|
| 1 | The account's **primary identity** | card **and** grid | none |
| 2 | A persisted per-account **alias list** (`account_aliases` in `preferences.toml`) | card **and** grid | the user adds them |
| 3 | The addresses the message was **delivered to**: `Delivered-To` / `X-Original-To` / `Envelope-To`, then `To:`/`Cc:` | card only | **none** |

Source 3 is what makes the common alias case work with no setup at all: the message reached this
mailbox, so an `ATTENDEE` matching an address it was delivered to is the user. It is only available
on the mail path, so **the grid depends on sources 1–2**: a grid has no message to read headers
from. That asymmetry is the reason the alias list exists at all.

Matching is case-insensitive and `mailto:`-scheme-insensitive, through the engine's
`addresses_match`, **never** `==`. iCalendar cases domains freely and writes the scheme
inconsistently. There is exactly one implementation of "is this cal-address me?", shared with the
engine's iMIP trust decision.

The **matched** address is what an RSVP is written with, not the account's primary: the CalDAV write
primitive patches a *named* `ATTENDEE` line, so an alias invitation answered as the primary address
finds no line and fails.

**Known false positive, accepted:** if a mailing-list address is an `ATTENDEE` and the list
delivered to this mailbox, source 3 yields a card. Outlook behaves the same way.

**Provider alias discovery** (an enhancement, not uniformly available): JMAP `Identity/get` is
clean; CalDAV principals expose `calendar-user-address-set` (verified live against
`caldav.soverin.net`); Google needs the `gmail.settings.basic` scope; Graph's `otherMails` is
partial and `proxyAddresses` may need directory permissions. None of these ship yet.

## Conflicts, and the three things that are not conflicts

The card states how many **other** commitments the user already has in the meeting's window, and
draws that window (the meeting and everything overlapping it) so the clash can be *seen* rather
than described (below: "…and it shows the meeting and its clashes, not the whole day").

Excluded from the count, each for a reason that produces a wrong number if dropped:

1. **The invitation's own event.** Every provider in use auto-schedules, so the tentative hold is
   already on the calendar by the time the mail is read. Counting it reports *every* invitation as
   clashing with itself, which trains the user to ignore the number.
2. **Other unanswered holds**: see the `X-MICROSOFT-CDO-BUSYSTATUS` note above.
3. **Declined events.** The user said no; it is not a commitment, and it is already hidden from the
   grid, so counting it would contradict what is on screen.

Overlap is half-open on both sides, so back-to-back meetings do **not** clash: that is the normal
way a diary is packed, and flagging it would make the number useless.

**The count covers the invitation's own account only**, a known shortfall recorded below rather
than left silent: the diary read is scoped to the account the mail arrived on, while the grid the
user knows draws every account. On a two-account setup the card can therefore say "Nothing else in
your calendar then" over a clash sitting in the *other* calendar. So can the preview, which draws
the same single-account day.

A client **must state the count in words** beside the preview. [`calendar.md`](calendar.md) §4:
nothing is hidden without saying so, and a grid the user has to read carefully is not a disclosure.

### "None" and "we have not looked" are different sentences

**`conflicts_known: false` is not zero.** The core reports it whenever it could not read the calendar
over the meeting's window: the diary read failed, the day would not resolve, or the engine has simply
**not expanded the calendar that far yet**. A client must then say so and must **not** print the
count, which is zero only because nothing was counted.

This is not a theoretical branch. **Mail syncs before calendars**, so an invitation opened on a cold
start reaches the card builder before any occurrence exists, and the first version of this shipped
"Nothing else in your calendar then" onto a Monday that in fact held two overlapping meetings. It is
the same distinction the grid draws with `is_materialized`, computed from the same window, for exactly
the same reason: an empty answer that looks like a real one is worse than no answer. A client also
**withholds the preview grid** while the count is unknown, because an empty grid over an unread
calendar is indistinguishable from a free day.
The preview is built by the same `calendar::grid::build` every calendar surface uses, so the core
keeps emitting unit-free geometry and a client only multiplies ([`calendar.md`](calendar.md) §1).
The number and the picture come from one pass over one window, so they cannot disagree.

### The preview opens expanded: on every platform

**Whenever `conflicts_known` is true, the grid is open.** Not "open when the count is non-zero": that
was the first rule, and it is wrong about what the preview is *for*. The question a person answering
an invitation is asking is **"what does my day look like"**, and the answer to that is the picture,
not the number, so a free day is exactly when the picture settles it fastest. "Nothing else in your
calendar then" over a drawn, visibly empty day is a *stronger* statement than the same words over a
collapsed row the reader has to trust.

It stays a disclosure control, not a preference: it is still gated on `conflicts_known`, because an
empty grid drawn over a calendar we have not read is indistinguishable from a free day (above). The
user can collapse it; nothing persists that choice, and nothing should: the next invitation is a
different day.

### …and it shows the meeting and its clashes, not the whole day

**The preview's hour band is the meeting, everything that overlaps it, and an hour of air**:
`invitationPreviewSpan` / `PreviewSpan`, with a six-hour floor so a short meeting on an empty
afternoon still has context around it.

It used to span the whole day's blocks, and that was wrong for the same reason a wall-sized map is
wrong: a normal working day runs 08:00–22:00, so fourteen hours were squeezed into the preview's
box, an hour came out around ten points, and every client's "is this tall enough for a title" rule
said no. The meeting **the card is about** drew as an *unnamed* rectangle beside a named one. A
picture that shows *that* your afternoon is taken but not *by what* answers the wrong question:
the reader's next move is deciding whether the clash matters, and they cannot without the title.
Growing the box instead was tried and is worse: it pushes the message itself off the screen.

**Nothing the card counts can fall outside the band.** A conflict is *by definition* an event
overlapping the meeting's own window, so every one of them widens the band and is drawn **whole**:
a booking that starts hours earlier drags the band back with it rather than being cut off at the top
edge with its title off-screen. What is left out is the rest of the day, and it is not left out
silently: the count is stated in words above the grid, and the disclosure label says what the
picture is (`invitation_conflicts_preview`: "Around this meeting", never "Your calendar that
day"). That is `docs/calendar.md` §4 satisfied, not waived.

The box itself stays fixed in the ordinary case and grows only when the band *cannot* be narrow
(an all-morning booking the meeting sits inside) up to a cap, past which short blocks lose their
titles again. Nothing is ever **clipped**, only unlabelled, and every block keeps its spoken label.
The numbers are *layout* and a platform may hold its own; the two formulas are the rule, and all
three clients unit-test them **composed** (a band this span can produce, at the height that band
gets, must still title a one-hour block) rather than pinning a constant.

### …and it draws the meeting itself, even where no calendar holds it

**The preview draws the meeting the card is about, as a hold, whenever the account's calendar does
not already hold it.** Same dotted treatment as any unanswered hold, at the times the card states
two rows above it, and **only** in the preview: nothing is written to the calendar, and the meeting
does not appear on the real grid until the user answers it (which files it, §"Who delivers the
answer").

Without it, the same invitation drew a different picture depending on the server behind it. Where
something files an invitation into the calendar, the tentative copy was in the diary read and the
preview showed it. Where nothing does (a bare mailbox, or an IMAP+CalDAV account with no bridge
from the mail store), the one block the card is about was the one block missing, so "Around this
meeting" was a drawn, entirely empty day and the reader had to place the meeting in it themselves.

**The gate is the calendar, not `calendar_scheduling`.** The capability answers whether the server
schedules what *we* write, and a server can advertise RFC 6638 and still never move an invitation
out of a mailbox (§"Advertising RFC 6638 is not a promise to file your mail"), so gating on it
would withhold the block on exactly the accounts that need it. The test is the one the answer path
already applies before writing: does the calendar hold this meeting?

Four rules keep the block from claiming more than it knows:

- **Never over an unread calendar.** It rides on `conflicts_known` like the rest of the preview; a
  block on a day nobody has read is indistinguishable from a day holding only that block.
- **No count moves.** `count_conflicts` skips the invitation's own `UID` whether the calendar holds
  the meeting or not, so the number stays "what else is in your calendar then" and the picture
  stays the meeting *among* it.
- **Nothing for a cancelled or superseded meeting.** A hold for a meeting that is off invents a
  commitment; a superseded one is superseded precisely because the calendar holds a newer revision,
  which is drawn instead.
- **Nothing for a declined one**, exactly as every other surface hides it.

It carries no event key and no calendar id, because there is no stored event to key and no calendar
to colour it by. That costs nothing: the preview carries no calendar list, so every block already
draws in the neutral swatch, and no client makes a preview block tappable.

## On the calendar: dotted for unanswered, gone for declined

The tentative event is **already there**. Exchange, Google, JMAP and CalDAV auto-schedule servers
create it server-side with `PARTSTAT=NEEDS-ACTION` the moment the invitation arrives, and every
engine provider normalises `PARTSTAT` on read. So rendering it needs **no calendar writes**: only
the missing presentation.

- **Unanswered** (`needs-action`): a client draws a **dashed border and a hatched leading gutter**.
  The visual is not sufficient on its own (a dashed border is invisible to a screen reader), so the
  accessibility label must **say** it ("Awaiting your response",
  `a11y_invitation_awaiting_response`). [`calendar.md`](calendar.md) §4, the spoken-grid rule.
- **The user's own appointment** (no attendees at all) and **a meeting we are not an attendee of**
  (a room booking, a colleague's event on a shared calendar) both read as commitments, never as
  unanswered holds. Getting this wrong would draw a user's own diary dotted and let a new
  invitation claim the slot was free.

### Declined events are hidden, and that has to be said out loud

Provider behaviour is **not** uniform, which is exactly why this belongs in the core:

| Provider | Native behaviour on decline |
|---|---|
| Graph / Exchange | removes the event from the calendar |
| Google | keeps it visible (a per-user Google setting governs hiding) |
| CalDAV / Sabre | keeps the resource with `PARTSTAT=DECLINED` |
| JMAP | keeps the object |

**Rule.** The core filters occurrences whose own participation is `declined` out of the grid, the
month and the agenda: one rule, identical on every platform, applied at the single join point where
occurrences meet their master events. That is both more predictable than inheriting four different
behaviours and what keeps the grid from clogging.

> **This hides data, so it says so.** [`calendar.md`](calendar.md) §4 forbids hiding anything
> silently. On three of the four transports the event still exists. We are hiding, not deleting: it
> stays on the server and in the store, and it reappears the moment the answer changes. **The
> invitation email is the way back**: its card shows the current answer. And the honest other half:
> **search is mail and contacts only** ([`search.md`](search.md)), so a hidden event is *not*
> findable through it, and the mail is the only route back until a calendar search exists. That is
> Outlook's own answer, and it is stated here rather than left implicit.
>
> **Graph is the fourth, and there the decline is not reversible.** Exchange removes the event from
> the invitee's calendar when they decline (the top row of the table above), so there is nothing
> left to un-decline: a later read of that event is `404 ErrorItemNotFound`, not a stale copy
> (observed live, engine `live_calendar_rsvp`). The invitation email survives and its card still
> renders, but changing the answer from it fails, because the event it would write to is gone. This
> is Outlook's own behaviour on Outlook's own accounts, which is why it is inherited rather than
> fought; what would be dishonest is the sentence above promising a way back that a Graph account
> does not have.

## Untrusted content

`SUMMARY`, `LOCATION`, `DESCRIPTION` and the organiser's display name are **attacker-controlled**.
The core emits them as **plain text**: control characters and the Unicode bidi overrides are
dropped, whitespace is collapsed, and the value is truncated on a *character* boundary (200
characters for a title or location, 500 for a description; Gmail writes a wall of `-::~:~::~`
filler that would otherwise push the message body off screen). A truncated description is flagged,
so a client says the text was cut rather than implying it ends there.

Markup is deliberately **not** escaped, because the contract is that these are text: escaping here
would show a literal `&amp;` on every client that renders them correctly. A client must therefore
render them as text: on GTK that means **`use_markup(false)`**, the libadwaita trap recorded in
[`../AGENTS.md`](../AGENTS.md). See [`rendering-security.md`](rendering-security.md).

The card is a new surface hosting untrusted sender content, so it is covered by the cross-platform
security-parity rule: a gate raised on one platform is raised on all of them.

## Answering is a verb of its own, and the client names the *message*

`Intent::RespondToInvitation { message, response, comment, notify_organizer, reply_subject }`. It
names the message the card came from (**never the event**) because the answer must go out as the address the
invitation matched, which on an aliased account is not the account's primary identity (§4). Only the
core knows the address set; a client that named the event would have to know the alias rule too, in
five places.

Underneath, the engine's neutral `rsvp_event` verb, with four adapter implementations:

| Provider | How it answers | Note to organiser | May stay quiet | Guard on the write |
|---|---|---|---|---|
| CalDAV | rewrite *my* `PARTSTAT` in the stored iCalendar, conditional `PUT`; an RFC 6638 server emits the `REPLY` | no | no | enforced (`If-Match`) |
| Graph | `POST /events/{id}/accept\|tentativelyAccept\|decline` | **yes** (`comment`) | **yes** (`sendResponse`) | **none**: the action endpoint takes no `If-Match` |
| Google | `events.patch` on the attendee's `responseStatus` | **yes** | **yes** (`sendUpdates`) | enforced (`If-Match`) |
| JMAP | `CalendarEvent/set` on `participants/<my id>/participationStatus` | no | no | none (no per-object revision) |

**A control a transport cannot honour is refused, never dropped.** The card carries `can_comment` and
`can_choose_notify` beside `can_respond`, and a client offers each only where it is `true`; the
adapter refuses the write otherwise. Dropping the note would be worse than refusing it: the user
would believe the organiser read their message.

**On the client-iMIP route both controls are ours, so both appear.** The note becomes a `COMMENT`
property in the `REPLY` this core writes, and "email the organiser" literally decides whether the
message is posted, so a plain CalDAV account, which could offer neither over the transport, gains
both. The same rule in the other direction: they are `true` because they are now honoured, not
because the card grew more generous.

`reply_subject` is the **localised** subject for that message, for example "Accepted: Sprint planning",
and it comes from the client because the core has no locale (`AGENTS.md` → "Localisation is client-side")
and this is copy a stranger reads in their inbox. Catalog keys `invitation_reply_subject_accepted` /
`_tentative` / `_declined`. `None` is safe and falls back to `Re:` plus the invitation's own subject:
deliberately **not** an English "Accepted: …", because announcing the answer in a language the user
does not speak is worse than quoting the organiser's own words back at them. Ignored entirely on the
server route, where no message of ours exists to put it on.

Answering is **not** applied optimistically. The write is awaited behind the existing
`CalendarWriteStatus` spinner and both surfaces are rebuilt from what the server holds; hiding a
declined meeting immediately would buy a few hundred milliseconds and cost a rollback path exercised
only when something has already gone wrong.

**The card reports the calendar's answer, not the email's.** `my_response` and the attendee tally both
read the stored event where there is one, falling back to the invitation only for a meeting the
calendar has not synced. The email is frozen at the moment it was sent: built from it, the card would
still say "you haven't answered" after you had, and would go on counting you among the people yet to
reply.

**The organiser is never counted among those yet to answer.** Whether they appear as an `ATTENDEE`
of their own meeting is a per-sender accident: a CalDAV auto-scheduler lists itself
`PARTSTAT=ACCEPTED`, Google emits only an `ORGANIZER` line, and iCalendar's default for a missing
`PARTSTAT` is `NEEDS-ACTION`. Read literally that reports the person who *called* the meeting as not
having replied to it, so a two-person Google invitation says "0 accepted · 2 awaiting". RFC 5546
§3.2.1 has the organiser attending by definition, so an organiser with no explicit answer counts as
one; an organiser who explicitly declined keeps their answer. The same meeting then tallies
identically whichever server sent it.

## The parser is tested against real invitations, not written ones

Eight captures live in [`../crates/mailcal-app/tests/fixtures/imip/`](../crates/mailcal-app/tests/fixtures/imip/)
(their provenance and scrubbing are recorded in the README beside them), driven by one table-driven
test. Seven are genuine: a `REQUEST` and `CANCEL` each from a CalDAV auto-scheduler, Google Calendar
and Exchange, plus a `REQUEST` sent and delivered **inside one M365 tenant**, and the eighth, a
published `.ics`, is authored because no server emits one on request.

This is not belt-and-braces over the unit tests: an invitation is a format whose failures are all in
the punctuation, and a hand-written fixture can only prove the parser handles what its author already
imagined. **Every** real `REQUEST` folds a `mailto:` URI **across a continuation line, mid-token**,
which a naive parser reads as a plausible `to:bob@…` that matches nobody: an invitation that
silently says "you are not invited to this". Three independent senders do it, so this is not one
server's quirk. A real auto-scheduler also dispositions a genuine invitation as
`attachment; filename="event.ics"`, which is why the gate reads the `METHOD` in the body and not the
disposition. The organiser-tally rule above was found this way: two senders, held side by side,
disagreed.

Exchange is the sender least safe to guess at, and each of its oddities is a way to be wrong that
no other capture exercises: a **Windows** `TZID` (`W. Europe Standard Time`) that is not an IANA
name, is unquoted, and contains spaces and a full stop, so the offset can only come from the
message's own `VTIMEZONE`, which starts in the year 1601; a `Subject` encoded in **Windows-1252**
while the calendar part beside it is UTF-8; an `X-MICROSOFT-LOCATIONS` value carrying escaped commas
and quotes across three folded lines; **no attachment at all**, the mirror of Gmail's duplicate; and
a delivery to the *optional* attendee's alias while `To:` names the *required* one, so matching on
`To:` answers as the wrong person.

And then the opposite hazard, from the same sender: an invitation that never leaves the tenant has
**no delivery header at all**. There was no MTA hop to write one, so `Delivered-To`, `X-Original-To`
and `Envelope-To` are all absent and `To:` is the only header naming the recipient: the bottom of
the fallback chain in §4 source 2, and the only capture that can fail if it breaks. This is the
*common* case, not an exotic one: colleagues in one M365 company inviting each other is most
corporate meeting mail, and a reader that stopped at the MTA headers would disable the buttons on
all of it. The capture also settles what Exchange does with a MAPI item it must render as MIME: the
iCalendar is the same shape as the internet-format one, down to the `X-MICROSOFT-*` set and the
mid-token fold, with plain `mailto:` addresses rather than the X.500 `/o=ExchangeLabs/…` form that
seemed likely before one was in hand. The envelope is the only thing that differs.

A Microsoft *reader* needs no fixture of its own beyond these. `provider-graph` fetches raw MIME
from `/messages/{id}/$value`, and for anything that arrived over the internet that returns the
**sender's original bytes**: verified by capturing a Google-sent invitation from a Microsoft
mailbox and finding Google's `Message-ID`, its intact DKIM signature and its boundary format, with
only Microsoft's `Received` hops prepended. So a Graph account sees the shapes above, and the one
message it sees that no other account type can is the tenant-internal one, which is now among them.

## Who delivers the answer

An RSVP that only writes a local `PARTSTAT` tells nobody: **a button that appears to work but
reaches no one is worse than no button**. So the RSVP affordance is gated on the account actually
being able to deliver a response, and it is **absent with an explanation**, never present and
disabled.

Two engine capabilities decide it, and they answer *different questions*:

- `Capabilities::calendar_scheduling`: **will anyone be told?** The server performs the scheduling a
  calendar write implies. Discovered on CalDAV at connect (`OPTIONS` on the calendar home, the
  `calendar-auto-schedule` token of RFC 6638 §2); constant `true` on Graph, Google and JMAP.
- `Capabilities::scheduling_submission`: **could we send it ourselves?** The mail transport can put
  the `method=` parameter on a `text/calendar` body part (RFC 6047 §2.4). True on SMTP, Graph and
  Google, which submit assembled RFC 5322 bytes; **false on JMAP**, which cannot express the
  parameter at all.

`calendar_rsvp` (can the transport *express* an answer) is the third, and it was the only one we
used to ask. That is the bug: on CalDAV the three come apart completely.

| the calendar can store it | the server schedules | we can send an iMIP message | route |
|---|---|---|---|
| yes | yes | — | **server**: store the answer and stop; ours would be a second reply |
| yes | no | yes | **client iMIP**: store it *and* post the `REPLY` ourselves |
| yes | no | no | **none**: the `PARTSTAT` would store perfectly and reach nobody |
| no | no | yes | **client iMIP**: a bare mailbox; nothing to contradict, and the organiser still learns |
| no | yes | — | **none**: it schedules on the write we cannot make |

Probe any server with
[`../scripts/dev/caldav-autoschedule-probe.sh`](../scripts/dev/caldav-autoschedule-probe.sh)
(read-only; sends no mail), or, better because it goes through the **real adapter** rather than
its own HTTP client, with the engine's `dav` tool, from an engine checkout:

```sh
cargo run -p dav-cli -- -p core-harness info          # what discovery concluded
cargo run -p dav-cli -- -p <profile> list             # events + the reply verdict each carries
cargo run -p dav-cli -- -p <profile> rsvp <uid> accept   # answer, and print the verdict
```

Servers are named once in `~/.config/allodia/servers/<name>.env` (mode 600, `URL`/`USER`/`PASS`,
plus `CALENDAR` for a real account; its collection is rarely called `default`). That directory
sits outside both checkouts, so **one profile serves both repos**; this repo's harness is
`core-harness` (port 28080), deliberately a different compose project from the engine's own
`stalwart` fixture (18080) so both can run at once. See `AGENTS.md` → "Debug a live CalDAV server
with the engine's `dav` tool".

### Advertising RFC 6638 is not a promise to file your mail

This cost a user-visible bug, and it is the most surprising thing on this page.
`caldav.soverin.net` (Sabre/DAV) advertises `calendar-auto-schedule` and exposes both
`schedule-inbox-URL` and `schedule-outbox-URL`, so `calendar_scheduling` reads `true` and the honest
route is **server**. And a Microsoft invitation delivered to that account still never reached the
calendar: it arrived as an email, in IMAP, and nothing on that deployment moves an iMIP message from
the mailbox into the calendar. **No RFC assigns that job to anyone**: RFC 6638 governs scheduling
the server performs on calendar operations, not a bridge from a mail store, so the token is not a
false claim. The two are simply different jobs, and only one of them has an owner.

The consequence is that "the server schedules" does **not** imply "the meeting is on the calendar".
It is also why the day preview gates its own hold on the calendar rather than on this capability
(§"…and it draws the meeting itself"). And the answer has to be written to *something*. So on **both** routes, if the calendar does not hold
the meeting, the core puts it there before answering: the invitation's own bytes with its `METHOD`
stripped (RFC 4791 §4.1 forbids `METHOD` on a stored resource), under a guarded create
(`If-None-Match: *`). That is exactly the attendee flow of RFC 6638 §3.2.2: the client stores the
scheduling object, the server turns the changed `PARTSTAT` into the `REPLY`, and it is what Apple
Calendar and Thunderbird do.

It has to be a whole-document write and not `create_event`: an `EventDraft` carries neither
`ORGANIZER` nor `ATTENDEE`, so a create through the neutral spine would store a plain appointment
with nothing to answer on. The guard matters because the concurrent writer is usually the *server*
(an auto-scheduling one deposits its own copy the moment the organiser writes), and a `412` is
therefore a **success with a different next step**: re-read and answer on the copy that is already
there, never overwrite it.

Verified live against the harness (Stalwart, which also advertises `calendar-auto-schedule`, with the
invitation `APPEND`ed to IMAP so nothing filed it, the Soverin shape exactly). Both halves:

| state | what happened |
|---|---|
| the meeting was on the calendar | create refused `412` → read back → `PARTSTAT` written → card reads "You accepted" |
| the meeting was **not** | create accepted `201` at `…/week-invited%40test.local.ics` → the stored resource carries the `ORGANIZER`, all three `ATTENDEE` lines and **no `METHOD`** → `PARTSTAT` written |

One more thing that only a live run found: the collection list is read from the **store**, which is
empty until a calendar sync has run, and the mail list is where a user lands. So an account whose
calendar had connected and discovered seconds earlier still failed with "this account has no
calendar". An empty list means *not looked yet*, never *none*; the answer path syncs and re-reads.

Credentials for that probe live in a mode-`600` file **outside the repo** and reach `curl` through a
`-K` config file, never `-u`: `-u` puts the password in the process list where any local process
can read it.

**On JMAP the same question has no probe, and at least one server answers "no".** JMAP Calendars
leaves scheduling to the implementation and advertises no capability for it, so unlike CalDAV's
`calendar-auto-schedule` there is nothing to ask. Tested against Stalwart (same two accounts, same
invitation, same neutral verb, minutes apart), the two transports diverge:

| answered over | the attendee's own copy | the **organiser's** copy |
|---|---|---|
| CalDAV (`PARTSTAT` in a `PUT`) | changes | the `REPLY` arrives, under a second |
| JMAP (`participationStatus` patch) | changes | **never changes** |

Checked in all three places a `REPLY` could land (the organiser's calendar copy, their scheduling
inbox, and their mailbox): none is generated. Nothing reports a failure at either end: the write
succeeds, the user's own calendar agrees, and the organiser is simply never told. That is the exact
shape this feature exists to prevent, arriving from the server instead of from us. The engine test
`jmap_rsvp_stores_the_answer_but_the_organizer_is_never_told` asserts the **absence**, so a server
that starts scheduling turns it red rather than passing unnoticed.

**What this means for the button.** It stays, and this is a judgement rather than an oversight: the
gap is one server's, not the protocol's, and it cannot be detected at runtime, so hiding the
affordance on every JMAP account would silence the servers that do schedule, to spare the ones that
do not. Both harness accounts sit on either side of this (`--account stalwart` is JMAP, so it has
the gap; `--account stalwart-imap` is CalDAV, so it does not), which is worth knowing when reading
a verification run. Listed under Known gaps below, and revisited if a second JMAP server is found
to behave the same way, at which point "unproven per server" stops being the honest description.

### A server can promise to send the reply, and then not send it

The **server** route is a promise: the client writes a `PARTSTAT`, the server mails the organiser,
and we deliberately send nothing ourselves because two replies are worse than one. When that promise
is broken, a conforming server says so: in the object it just stored, as the `SCHEDULE-STATUS`
parameter of RFC 6638 §3.2.9, carrying an RFC 5546 §3.6 status (`1.x`/`2.x` delivered, `3.x`/`5.x`
failed; there is no `4.x`). Nothing in this product used to read it.

It cannot be decided up front from capabilities the way the route itself is. `calendar-auto-schedule`
is advertised by servers that never deliver a single reply, and there is no token for *"…and it
works"*. The only honest source is what the server reports **after** the write. So: trust, verify,
then offer the fallback.

**Which property carries it decides what it is about.** `SCHEDULE-STATUS` on the `ORGANIZER` is the
verdict on the `REPLY` we just sent; on an `ATTENDEE` it is the verdict on a `REQUEST` we sent as
the organiser. Reading the wrong one reports on a message this user never sent.

**Silence carries no information, and that is the load-bearing fact.** Two servers disagree, and
both are conforming:

| server | delivers the reply | writes `SCHEDULE-STATUS` |
|---|---|---|
| Stalwart | yes, in under a second | **no** |
| Sabre/DAV (as deployed at one provider) | never | `5.2` |

So there are three states, not two, and collapsing them breaks in one direction or the other:

- reading silence as **failure** asks every Stalwart user to email an organiser who already has the
  reply, and answering "yes" sends a duplicate;
- reading a reported failure as **success** leaves "You accepted" as the only thing the user ever
  sees, while the organiser was never told.

An unrecognized status class is treated as *no report* rather than as a failure, for the same
reason: guessing "failed" would email an organiser who may already have the answer. The token is
logged at a level that is on by default, because it is exactly what someone debugging an unfamiliar
server needs.

**Where the reading happens.** In the engine, in `provider-caldav`, beside the `PUT` that produces
the status, not here. The verdict rides back on the write receipt as `ReplyDelivery`, and this
repo's job starts where the protocol's ends: what to do about it. (A parser was written here first
and thrown away; `AGENTS.md` → "Protocol knowledge belongs in the engine" is that episode.)

**The user is asked, not told.** Sending the reply ourselves means sending **mail as the user**, to
somebody they did not choose in this moment. That is not a repair an app gets to make silently, so
the default is to ask. Four rules bind every client:

1. **The RSVP worked, and the prompt says so first.** The answer is stored; what failed is the
   message to the organiser. A prompt that opened with "couldn't send" would invite the user to
   answer again, which writes the same `PARTSTAT` and fails the same way.
2. **The recipient is named.** `prompt.organizer` appears in the sentence, never the words "the
   organiser". Consent to send mail on someone's behalf is not informed without the address.
3. **The status code is not shown.** It rides the prompt for the diagnostics log; `5.2` explains
   nothing to the person reading a modal.
4. **The choice can be remembered, per account, and it applies in both directions.** A server that
   fails every reply must not ask at every meeting. One tick, wired to whichever button was pressed:
   beside "Don't send" it is a standing *no*, which is the half that is easy to drop and impossible
   to notice: the symptom of dropping it is being asked again, forever, on exactly the server the
   setting exists for. Stored as `reply_fallback` per account (`Ask` / `Always` / `Never`).

The question lives in the **core**, not in a view: it is raised on `Surface::InvitationReply`,
answered by `Intent::AnswerReplyPrompt`, and **taken** before any await, so two taps on a modal
that has not closed yet, or a client dispatching on both press and release, cannot email the
organiser twice. `None` is equally how the core says *close it*; a host never dismisses on its own,
and nothing may dismiss it without answering, or the core would go on holding a question the user
can no longer see. It carries no handle on the meeting, and everything is re-derived from the store
when the reply is finally sent, so a prompt cannot act on stale state.

Where there is no organiser to email, there is no question: an offer to email nobody is a control
that cannot work, so the answer is stored and nothing is asked.

**The chrome differs by platform; the decision set does not.** macOS and iOS present a sheet
(SwiftUI's alert takes buttons and nothing else, and the tick has to be a control whose state is
visible before committing); Android an `AlertDialog` with a checkbox; Windows an **InfoBar**,
because WinUI permits one `ContentDialog` at a time and silently drops a second: a question raised
while another dialog was open would have been thrown away while the core kept holding it.

**If we send it ourselves, it goes out the ordinary way.** `send_imip_reply` submits through the
outbox, so the send reports through `SendStatus` like any other message and inherits its durability.
The subject is the client's to compose (`invitation_reply_subject_*`), because the core carries no
locale and this is copy a stranger reads in their inbox.

### Exercising it: `MAILCAL_FAKE_REPLY_DELIVERY`

The state this whole section exists for is the one **no fixture can produce**. Every harness runs
Stalwart, which delivers replies and reports nothing; the server known to report `5.2` is somebody's
production account. So for a while the only way to see the prompt was to edit
`invitations_fallback.rs` by hand, which is how it was verified on macOS, iOS and Android in turn,
once each, and never again.

A **debug-only** environment variable substitutes the verdict instead:

```sh
MAILCAL_FAKE_REPLY_DELIVERY=failed:5.2       # also delivered:2.0 · unrecognized:9.9 · notreported
```

Two properties make it a testing aid rather than a mock. It replaces **only the server's verdict**:
everything downstream is the real thing, so a run still proves that the core raises the question,
signals `Surface::InvitationReply`, takes it before any await, remembers the choice and puts real
mail in the outbox; a hook that faked the *prompt* would keep passing after the wiring between core
and client was cut. And its value names a **variant**, never a status token to be classified: which
class a code belongs to is protocol knowledge and it lives in the engine, whose `ReplyDelivery` docs
say in as many words that no caller should branch on the text. A value it does not recognise is
declined with a warning and the server's real answer is used: it under-tests rather than inventing
a failure that never happened. It is `cfg(debug_assertions)`, like the harness CA trust beside it,
and it logs a warning on **every** RSVP while it is in force, so no log of a run can hide that it was
on.

The Windows UI suite ([`../clients/windows/uitests/InvitationReplyPrompt.Tests.ps1`](../clients/windows/uitests/InvitationReplyPrompt.Tests.ps1))
is built on it, and covers the half no other machine can see: `Mailcal.Tests` cannot link a WinUI
type and `cargo test` cannot see a XAML binding, so the InfoBar's `IsOpen`, the tick's initial state,
`IsClosable="False"`, and the code-behind line that clears the tick between meetings had nothing
watching them. It needs `MAILCAL_DEV_ACCOUNT=stalwart-imap` (`resolve_reply_delivery` runs on the
**server** delivery route, which is CalDAV) and it **declines** rather than accepts, because a
declined event is hidden from every calendar surface and so leaves the free-day fixture the
invitation-preview suite depends on exactly as it found it.

## The showcase dataset seeds one, and it is a check rather than a picture

The store screenshot set has an `invitation` screen (`scripts/dev/showcase.sh --screen invitation`,
`MAILCAL_SHOWCASE_SCREEN=invitation`): the seeded inbox carries a real iMIP `REQUEST`, and a client
opens it and stops. It exists because this is the one surface where mail and calendar are visibly
the same product: a message that offers Accept / Maybe / Decline over a drawing of the day it would
land on, and no other screenshot shows that.

Three things had to be arranged for it, and each of them is a rule from this document rather than a
prop:

- **The meeting is in the diary as well as in the mail**, under the same `UID` and at the same
  instant, the way an auto-scheduling server leaves it (§"On the calendar"). That is what makes the
  card read its answer off the calendar rather than off the frozen mail, and what draws the dashed
  hold.
- **The calendar is synced at boot**, before any message is opened. Mail otherwise syncs first, and
  the card would honestly report "we have not looked at your calendar", with its preview collapsed
  (§"The preview opens expanded"). A screenshot of that is a screenshot of the cold-start case.
- **The showcase calendar provider really answers an RSVP.** The buttons appear only where the
  account's provider advertises `calendar_rsvp`, and advertising a capability it then refused would
  be the "control that lies" this document forbids, aimed at whoever is taking the screenshots.

None of the three is visible in a PNG: a card missing its button row, or drawn over an unread
calendar, photographs exactly as well as a correct one. So they are pinned in
`mailcal-bindings::tests_showcase_invitation` (including the answer round-trip), and the capture
script asserts nothing about them beyond the frame not being blank.

## Logging and privacy

A meeting's title, its organiser, and its attendee addresses are message content.
[`logging.md`](logging.md) is absolute: the core logs counts, ids and durations only, never any of
this: the diagnostic log has to stay safe to attach to a support request.

**Privacy policy: no change needed.** An RSVP travels to the user's own provider over the existing
mail/calendar paths: no new network destination, no new stored data category, no new processor. The
card is assembled from a message the app already downloaded.

**Sovereignty:** no AI/model dispatch, so the `JurisdictionGate` is not involved.

## Per-platform matrix

Legend: ✅ shipped · 🚧 in progress · ⬜ not yet.

| Capability | Shared core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|---|---|---|---|---|---|
| Detect an iMIP invitation, all four sender shapes | ✅ | — | — | — | — | — |
| Two-condition RSVP gate (table-driven, tested) | ✅ | — | — | — | — | — |
| Superseded revision detected (`SEQUENCE`, RFC 5546 §2.1.5) | ✅ | — | — | — | — | — |
| Superseded card: details, no answer, says why | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Answering a superseded copy refused at the write | ✅ | — | — | — | — | — |
| Parser driven over **captured** invitations (CalDAV, Google, Exchange internet + tenant-internal) | ✅ | — | — | — | — | — |
| iMIP body part hidden from the attachment list | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Invitation card (organiser, when, where, attendee tally) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Card reachable from the message list through a screen-reader action | — | ✅ | ✅ | ✅ | ✅ | ✅ |
| Conflict count **stated in words** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| "We have not looked yet" distinguished from "nothing" | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Meeting-day preview grid | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| The meeting drawn on that preview as a hold where no calendar holds it | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Cancellation notice | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Untrusted fields as plain text | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Alias matching: delivery recipients (zero-config) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Alias matching: persisted per-account list | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Alias list editable in Settings | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| Unanswered holds drawn dotted on the grid | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Unanswered holds on the month grid and the agenda | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| "Awaiting your response" spoken label | ✅ (string) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Declined events hidden from grid / month / agenda | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Neutral RSVP verb + four provider adapters | ✅ | — | — | — | — | — |
| Delivery route decided from `calendar_scheduling` + `scheduling_submission` | ✅ | — | — | — | — | — |
| Invitation filed on the calendar when nothing else did (guarded create) | ✅ | — | — | — | — | — |
| iTIP `REPLY` written and posted as an iMIP message when no server will | ✅ | — | — | — | — | — |
| Localised reply subject supplied by the client | ✅ (keys) | ✅ | ✅ | ✅ | ✅ | ✅ |
| `SCHEDULE-STATUS` read after the write (RFC 6638 §3.2.9) | ✅ (engine) | — | — | — | — | — |
| Silence, success and a reported failure kept three states | ✅ | — | — | — | — | — |
| "The organiser wasn't told" prompt | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Recipient named in the prompt; status code withheld | ✅ (fields) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Remembered per-account choice, both directions | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Prompt cannot be dismissed without answering | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| RSVP write (Accept / Maybe / Decline) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Note to the organiser, where the transport carries one | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Answer without notifying, where the transport allows it | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Optional message to the organiser | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| Notify-organiser toggle | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| Remove a cancelled meeting from the calendar | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |

Two rows are ✅ everywhere because the core enforces them where occurrences and attachment lists are
assembled: **declined-hiding** and the **suppressed iMIP body part**. A client inherits both without
any client code and cannot opt out, so unlike every row above them, they were not "shipped per
platform" at all.

**macOS and iOS/iPadOS move together**, and always will: one Swift package (`MailcalKit`) draws the
reading pane and the calendar on both, so a card that renders on the Mac renders on the iPhone in the
same change. They are separate columns because a *layout* can still differ (a compact width wraps the
answer buttons where a Mac fits them on one line); the presence of a capability cannot. Note what may
**not** differ that way: whether the preview opens is a **rule**, not a layout, and it is the same on
every platform (above).

**Android renders the same card from the same numbers, and none of its own.** Its rules live in
`InvitationFormat.kt` (the deliberate twin of `InvitationFormat.swift`, function for function), so
the conflict wording, the attendee arithmetic and the preview's hour band are unit-tested on the JVM
rather than trusted to a screenshot. Its calendar surfaces are a **canvas**, not a view tree, so the
hold treatment is a draw call (`CalendarParticipation.kt`) resolved when the page is built and never
inside a frame; that is a performance rule of the Android grid, not a difference in the contract.

**Windows is the third twin, and its rules return a *choice* rather than a sentence.**
`Calendar/InvitationFormat.cs` is the same function set again, with one deliberate difference: where
Swift and Kotlin return localised strings, it returns `ConflictLine.Unknown`, `AttendeeLine.PendingOne`
and so on, and `Views/InvitationText.cs` maps each to its `L10n` call. That is not a stylistic
preference: `Mailcal.Tests` is a plain `net10.0` assembly and **cannot link `L10n.cs`**, which opens
with `using Microsoft.Windows.ApplicationModel.Resources`, so a rule phrased as a string would be a
rule no test could reach. Splitting at the choice keeps every one of them pinned, which is the same
seam `TimeZones.RelativePattern` and `CalendarEventSummary` already use. The card and its preview are
**composed** WinUI rather than painted on the calendar's Win2D canvas, for the reason `MonthGridView`
gives: §7's per-frame budget belongs to the time grid's pinch and fling, and a card that is laid out
once has neither.

**How a hold is drawn, per surface.** The grid and the all-day banner get the dashed border, the
hatched leading gutter and the faded fill; the month chip gets the hatch and the dashes (it is a few
points tall, so the hatch is what survives); the agenda is a list with no border to dash, so it prints
"Awaiting your response" on the row instead. Different pictures, one disclosure, and every one of
them also carries it in the spoken label, which is the part that is not optional.

**Linux is the fourth twin, and it returns sentences rather than choices.** `ui/invitation.rs` is
the same function set again (`conflicts`, `attendees`, `when`, `preview_span`, `preview_height`),
but it returns localised strings the way Swift and Kotlin do rather than the *choice* Windows
returns, because the reason for that split does not exist here: `l10n` is a plain generated Rust
module, so a rule phrased as a string is still a rule a unit test reaches. Its two calendar
renderers are a Cairo surface (the time grid, the all-day banner and the card's own preview) and
GTK CSS (the month chips), over one set of constants in `ui/calendar/paint.rs`, the twin of
`CalendarHold.cs` and `CalendarParticipation.kt`.

Two things this platform makes harder than the others, and neither is optional:

- **A string is not safe by default.** A libadwaita row parses its title *and* subtitle as Pango
  markup, so an unescaped ampersand renders the row **blank** and a markup-shaped `LOCATION` arrives
  styled. Every untrusted field on the card goes through a `GtkLabel` set with `set_text`, and the
  widget test asserts on the **rendered** label plus the absence of a `from markup` log record:
  the two halves catch different defects ([`rendering-security.md`](rendering-security.md), Gate 8).
- **An explicit accessible label loses to a relation.** A `GtkButton` with a label is `labelled-by`
  that label, so `Accept` keeps announcing "Accept" whatever label is set on it. The qualifier
  ("Accept this invitation") is a **`Description`** instead, which is announced after the name and
  has no competing relation, and the same applies to a month chip and an agenda row. GTK exposes no
  getter for either, so the only oracle is the AT-SPI run (`scripts/dev/test-linux-ui.sh`), which is
  where those assertions live.

**Windows draws it from one file, in two renderers.** `Calendar/CalendarHold.cs` is the twin of
Android's `CalendarParticipation.kt`, constant for constant, but the week grid and the all-day
banner are a **Win2D canvas** while the month chips and the card's preview are **composed WinUI**,
and a `CanvasControl` per month chip would cost far more than a hatch is worth. Android delegates
its composed path back to its canvas one; Windows cannot, so the hatch is written twice, immediately
below each other, over **one** set of constants (gutter, pitch, dash, `HoldFillAlpha`). Change the
pitch and you change both by construction. The two decisions that could differ between clients are
in the test-linked pure layer instead (`InvitationFormat.HoldAlpha`, `SpokenWithHold`), so the fade
arithmetic and the spoken suffix are pinned by `Mailcal.Tests` rather than by a screenshot. The
block already has a hairline, so a hold **restyles** that stroke rather than gaining a second edge;
a bar and a chip have none, so they gain the whole treatment in one call.

## Known gaps

- **Supersession is invisible until the meeting is on the calendar.** The rule compares the mail
  against the **stored event**, so a mailbox with no calendar behind it (a bare IMAP account, or
  an IMAP+CalDAV account whose server has no inbound iMIP bridge, see "Who delivers the answer")
  has nothing to compare against and every copy stays answerable. Answering now *files* the meeting,
  so the second invitation is correctly marked superseded once the first has been answered; before
  that, both look current.
  That is the conservative direction on purpose, but it means the users who most need this see it
  least: their calendar is exactly the one that never learns the meeting moved. Detecting it from
  the mailbox instead needs a `UID`-keyed index over mail, which the messages cannot supply by
  threading (Exchange sends no `References`/`In-Reply-To` on an invitation update), so it is engine
  work rather than a rule that could be added here.
- **The hold's faded fill drops the label's contrast below the 4.5:1 the core guarantees, on
  every client, not one.** [`calendar.md`](calendar.md) §1 says the core resolves each swatch so the
  label reads at ≥ 4.5:1 against its fill; the hold treatment then multiplies that fill's alpha by
  `HOLD_FILL_ALPHA` (0.4) and leaves the label at full strength, so a white title over a 40% fill
  composited on white lands nearer 2:1. Observed on Windows, but it is **not** a Windows bug: Android
  (`holdFill` + `swatch.text`) and Apple do exactly the same, which is why it is filed here rather
  than fixed on one client: a unilateral change would break the "one picture on every client" rule
  this section exists to state. The fix is one decision for all of them (darken the label on a hold,
  as the invitation card's own preview already does; or fade toward the surface and re-resolve
  contrast in the core), and it belongs in a change that touches every client at once.
- **The note to the organiser is not runtime-verified on Windows or Linux.** It is conditional on
  `can_comment`, and no harness account reports it: the CalDAV account takes the server route (both
  controls `false`), and the JMAP account reports `can_choose_notify` without it. So the **tick** is
  now runtime-verified on Linux (`scripts/dev/test-linux-ui.sh` photographs it on the harness's
  JMAP account) while the **field** is covered only by the compiler, by reading, and by the Linux
  widget test that feeds a card with `can_comment: true` and asserts the entry appears. Seeing it
  against a real transport needs a live Microsoft or Google account with an unanswered invitation on
  it, or a CalDAV server that does not schedule (the engine's SabreDAV fixture is the only one on
  hand). Every client is the direct mirror of `InvitationRespondView.swift`, driven off the same two
  FFI booleans.
- **The Linux reply-undelivered prompt was verified against a live server, not the harness.** It is
  raised on the **server** delivery route, which is CalDAV, so the shared acceptance run cannot
  reach it: that run pins the JMAP harness account, and the fixture the prompt needs is on the other
  one. What covered it instead was a real answer on a **Sabre/DAV** account (the deployment this
  document already records as reporting `5.2` and never delivering) with **no**
  `MAILCAL_FAKE_REPLY_DELIVERY` in force, so the whole chain ran on the server's own verdict:
  the answer stored, `5.2` read back, the question raised, the modal answered "send", and the iTIP
  `REPLY` submitted through the outbox. That is a stronger exercise than the substituted verdict the
  Windows suite is built on, and it is also the *only* one that has proved the wiring between core
  and client here: `Surface::InvitationReply` reaching the model and the generation opening the
  window exactly once.

  It is a **manual** verification, which is why this is still a gap: nothing re-runs it. The
  repeatable half is the GTK widget test (`ui/invitation/widget_tests.rs`), which renders a
  `ReplyPrompt` and asserts the organiser is named, the RFC 6638 status code is not, and a
  `close-request` is **refused** and leaves the window standing. To drive the live path again:
  launch with `MAILCAL_DEV_ACCOUNT=stalwart-imap` and `MAILCAL_FAKE_REPLY_DELIVERY=failed:5.2`, open
  the harness's `Quarterly planning`, and decline: declining leaves the free-day fixture the
  preview suite depends on exactly as it found it.
- **No capture of a tenant-internal `CANCEL`.** `exchange-internal-request` closed the gap that
  mattered (a `REQUEST` delivered inside one M365 tenant, with no delivery header), but its
  cancellation was not kept, because `outlook-cancel` already pins `METHOD:CANCEL` in Exchange's
  shape and the two would differ only in the envelope this fixture already documents. If a
  tenant-internal `CANCEL` ever turns out to differ, it is one capture away by the recipe in the
  fixtures README.
- **A JMAP account can no longer answer at all where its calendar does not schedule.** JMAP cannot
  put `method=` on a body part, so `scheduling_submission` is `false` and the client-iMIP route is
  closed to it; where the calendar server also does not schedule, the card correctly offers no
  buttons. That is honest rather than good: the same account over IMAP/SMTP could answer perfectly.
  Lifting it needs a JMAP server that stores a raw `Content-Type` faithfully (the engine's
  `live_imip.rs` pins the current behaviour and goes red if one appears).
- **The reply we post is not threaded into the organiser's scheduling inbox, only their mailbox.** It
  is an ordinary iMIP message: `In-Reply-To` the invitation, `From` the matched attendee, the iTIP
  object as an alternative body part. A server that reads its own scheduling inbox will not see it
  there. Every client that matters processes iMIP from the mailbox, which is the whole basis of
  cross-organisation scheduling, so this is a shortfall on paper rather than in practice.
- **A JMAP account may answer and tell nobody.** Stalwart stores the `participationStatus` and
  schedules no iTIP `REPLY`: the write succeeds, the user's own calendar agrees, the organiser is
  never told, and nothing on either side reports it. See "Who delivers the answer" above for
  the evidence and for why the button stays. Unlike CalDAV there is no capability to probe, so this
  cannot be detected per account; it is a property of the server the user happens to be on.
- **A Graph RSVP is unguarded**: observed, not assumed. Its action endpoint takes no working
  precondition: a *matching* `If-Match` is accepted and ignored (`202`), and a malformed one answers
  `500`, never a `412`. So answering "yes" to a meeting the organiser has since moved lands anyway
  and the user has agreed to a time they never saw. Reported honestly as `RsvpControls::guard =
  Absent` rather than hidden behind the adapter's enforced guard for edits; there is nothing a client
  can do about it but re-read afterwards, which the write already does.
- **A Google organiser answering their own invitation truncates the attendee list.** The RSVP sends a
  one-element `attendees` array. Google's leniency is keyed on the **caller's role**, not on the
  array: as an attendee (the only case that reaches this code today), the other guests are left
  alone, and as the organiser the same request replaces the array and drops them. Both halves are now
  live tests rather than the reasoning that used to stand here, so if Google changes either one the
  gap is revisited instead of quietly becoming wrong. Rebuilding the whole array from the engine's
  lossy projection would instead drop the per-attendee fields it does not model (`additionalGuests`)
  for *everybody* rather than nobody, so this is recorded rather than worked around.
- **No note on CalDAV or JMAP.** iCalendar has no per-attendee comment parameter, and while RFC 8984
  defines `participationComment`, no server we run has been seen to relay it, so it is advertised as
  absent rather than promised. Both are refusals, not silent drops.
- **"Propose new time" (iTIP `COUNTER`) is not in v1**, on any platform.
- **A `ClientImip` account cannot answer at all.** A bare IMAP mailbox with no auto-scheduling server
  never puts the meeting on a calendar, so there is no event to write to; the core refuses with a
  visible failure rather than a button that does nothing.
- **A cold-start answer fails, and says only "try again".** Mail syncs before calendars, so an
  invitation opened in the first seconds after launch has no calendar behind it (the same reason
  the card says "we haven't looked at your calendar yet") and the answer is refused. The failure is
  *visible* and the advice is *correct* (it works the moment the calendar arrives), which is why this
  is a gap and not a bug. What it does not do is say **why**: the core knows the reason and the write
  status has no room for it. Naming it would mean a richer per-write outcome than
  `CalendarWriteStatus`, which is more than one string. Seen on the iOS simulator, 0.13.0.
- **The mail list still shows a paperclip on an invitation.** The reading view correctly shows no
  attachment (the core suppresses the iMIP body part), but the row's clip comes from the engine's
  `Message.has_attachment`, which is protocol metadata (JMAP's server-computed `hasAttachment`, IMAP's
  `BODYSTRUCTURE`) set long before anything parses the part. So the list promises a file the reading
  view then does not offer. The fix belongs in the **engine**, where `has_attachment` should mean the
  same thing `attachment_meta` does ("there is a part the user can download") and is its own change;
  it is not something a client should paper over, or the four of them will paper over it differently.
- **The conflict count and the preview are single-account.** Both read only the calendar of the
  account the invitation arrived on, while every calendar surface the user knows is unified across
  accounts. A work invitation therefore cannot see a clash in the personal diary, and the card can
  state "Nothing else in your calendar then" over one: the same class of confident answer
  `conflicts_known` exists to prevent, from a different cause. The fix is to read every configured
  account over the same window and pool the occurrences; it is deliberately not in this release
  because the preview then also has to show whose calendar each block belongs to.
- **No "propose new time"** (iTIP `COUNTER`).
- **No local materialization for `ClientImip` accounts** (bare IMAP+SMTP with no auto-schedule
  server). The card renders, but nothing lands on the grid, and no iMIP `REPLY` can be sent: the
  engine's SMTP assembler is `text/plain`-only, so it cannot emit a `text/calendar` alternative part.
- **The alias list has no Settings surface.** It is persisted and honoured, but only editable by
  hand in `preferences.toml`. It belongs under the account category in
  [`settings.md`](settings.md).
- **No provider alias discovery** (the three sources above).
- **No DKIM, therefore no auto-apply.** The engine's `reconcile` wants an authenticated sender
  before applying an inbound message. Nothing here needs it: we *render* what the server already
  scheduled and would *write* only what the user explicitly clicks. Auto-apply stays out of scope.
## Enforcement

This contract is binding via [`../AGENTS.md`](../AGENTS.md). When you change how an invitation is
detected, shown, answered, or how participation renders:

1. Update this document (**the rule and the matrix**) in the same change.
2. Apply the change to **every** platform that has the surface, or record the shortfall under Known
   gaps, never silently.
3. Participation rendering and the declined-hiding rule also live in
   [`calendar.md`](calendar.md): keep both, and its matrix, in step.
4. A user-facing change also updates [`../README.md`](../README.md)'s capability matrix and adds
   a fragment under `docs/changelog/unreleased/` (every catalog locale, with its `Platforms:` and
   `Bump:`), see [`changelog.md`](changelog.md). It does **not** touch [`../VERSION`](../VERSION):
   only a release PR moves that.
5. A new gate on untrusted sender content updates
   [`rendering-security.md`](rendering-security.md) (rule **and** per-platform matrix) and applies
   on every platform.
