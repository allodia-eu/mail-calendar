# The calendar contract

The calendar is one product across every client. This file is the **contract** they all keep: what
the core decides, what a client decides, and the handful of rules that are not cosmetic: the ones
where getting it wrong produces a screen that looks perfectly plausible and is wrong.

Like [`docs/rendering-security.md`](rendering-security.md) and [`docs/logging.md`](logging.md), this
is a **cross-platform** document. Raise a rule on one platform and you raise it everywhere; any
shortfall goes under "Known gaps" rather than staying silent.

---

## 1. The core lays out. The client multiplies.

The core returns geometry that carries **no units at all**:

| The core says | The client turns it into |
|---|---|
| a day **index** | an x offset: `dayWidth × index` |
| a wall-clock **minute** | a y offset: `hourHeight × minutes / 60` |
| a **column fraction** (`column` of `columns`) | a block's width and lane within its day |
| a **lane** (all-day bars) | a row of the banner |

So the same page renders on a phone, a tablet and a desktop pane without the core knowing anything
about any of them, and no two clients can disagree about *where* an event is, only about how big an
hour looks.

**The corollary matters as much as the rule:** an hour has no height, and a day no width, until a
client multiplies. Zoom is therefore a *client* concern. The core never learns about pixels.

### What the core owns, and why

- **Overlap solving.** Clashing events are packed into lanes in Rust (`calendar/packing.rs`). A
  client that re-packed could column identical data differently from its siblings.
- **All-day lane stacking.** Same reason.
- **Colour.** Each calendar resolves to a light and a dark swatch (fill, label, border) and the
  **label is already guaranteed ≥ 4.5:1 (WCAG AA) against its fill.** No client computes contrast, so
  none of them can disagree about whether a chip is readable. How the *default* colour is chosen is a
  product decision: see "Colour defaults" below.
- **The day axis, including all-day semantics.** All-day events are **zoneless** (bare calendar
  dates, invariant under the display zone) and their end is **exclusive**. Localising them drags a
  UTC midnight to 02:00 in Amsterdam, the exclusive end stops looking like a midnight, and **every
  one-day event renders two days wide.** This took two bugs to get right; the month grid shares the
  time grid's `extent()` so the two layouts cannot disagree about which day an event lands on.
- **Which day a week starts on** (see §3).

### What a client owns

- **How tall an hour is, and how wide a day.** Pixels.
- **How many all-day lanes fit** before the banner starts hiding bars (§4).
- **How many chips fit in a month cell** before it starts hiding events (§4).
- **All localised copy.** The core emits ISO instants and structured fields and owns no locale
  facility at all. Weekday headings, hour rulers, period titles and spoken labels are assembled
  client-side.
- **`now`.** The red line is *not* in the snapshot. Baked in, it would go stale every 60 seconds and
  rebuild the page across the FFI every minute, forever, on battery. The client has a clock. Opening
  a time grid frames that line vertically, clamped at the day's edges; later renders preserve the
  scroll position the user chose.

### The scroll offsets are the client's, so keeping them valid is the client's job

A client that holds its own scroll offsets (every one of ours does, because a pinch has to *move*
them mid-gesture and a scroll view will not be told where to be) **must put them back inside the
content whenever the content changes size**, not only when a gesture moves them. The offsets are in
points; the content's size is the client's own multiplication of unit-free geometry; so anything that
changes a multiplier silently invalidates an offset that was correct a frame ago.

Three things do that, and **none of them is a gesture**: the **window** resizing (a shorter window
means a shorter hour), the **persisted zoom arriving after the first frame** (the horizon and the
column count are core settings, so a client can render, and frame itself on today, before they land),
and the **all-day banner** gaining a lane. Miss it and the grid is scrolled off its own content: the
gutter and the all-day band render, the hours and every event do not, and it reads as a calendar that
failed to load rather than as a scroll position. It is also self-healing on any scroll, which is what
makes it look intermittent: the user "clicks around a bit" and it comes back.

Android does this in `clampScroll(metrics)`, Windows by deferring its recentre until the viewport is
real, Apple in `CalendarScreenView.Grid`'s `onChange(of: maxHour)`. A new platform owes its own, and
owes the test, which needs no viewport: recentre against one geometry, change it, and assert the
offset is still inside the content.

**A client with a strip owes only the hour half of that**, because the day axis has no end to fall off
and its anchor re-derives itself from its position. What it owes instead is a **first framing that
waits**, and "wait" is a state rather than a moment: the grid keeps framing itself on today until a
hand moves it. Apple learned this the expensive way. SwiftUI lays the grid out several times before
the window settles, and the first pass measures 620x59, where an hour is five points tall; framing
once, on the first pass with a plausible-looking size, opened the calendar at 01:00. There is no test
for "is this viewport real yet" that is not a guess at a number only the window knows.

⚠️ **And measure the box, with the modifier built for it.** Apple's `onChange(of: geometry.size)`
inside the `GeometryReader` never fired for the resize that mattered: the body evaluated five times at
the new size while that comparison still held the old one, because a `GeometryProxy` read inside a body
is not a value SwiftUI tracks. `onGeometryChange` is. Attached *inside* the reader it then measured the
grid's own frame, which is far taller than the box it is laid out in (the hour ruler's natural height
is a whole day: 1778pt against 827pt), and framing against that put the grid at the bottom of the day.
Both failures render a perfectly plausible calendar, at the wrong time of day.

### Colour defaults

A calendar's colour is chosen in the **core** (so all three clients agree) in this order, and the
user can always override the result (the manager, §12):

1. **A user override**, if set.
2. **The server's colour**, snapped to the nearest palette hue. A user migrating from another client
   keeps their coding: their red calendar stays red-ish. (Only the ten-hue Allodia palette is ever
   rendered: Allodia Orange is reserved for actions, so a calendar can never borrow it.)
3. **A palette hue no other calendar is already using.** When the server sends no colour, the calendar
   is given a hue that is not already taken (by another calendar's override, its server colour, or
   another colourless calendar), so a freshly connected account comes up in *distinct* colours instead
   of a wall of blue, **across accounts too**. Assigned in the calendars' stable order, so adding a
   calendar takes a fresh hue rather than recolouring the ones already on screen; once all ten hues are
   in use it cycles, because a calendar must always have *a* colour.

This is a cross-platform product decision. It lives in the core (`calendar/color.rs` and the cache's
`resolved_calendars`), so a client never assigns a colour itself: it only draws the resolved swatch
and offers the override.

### The default calendar

**Which calendar a new event lands on is a setting, and the core resolves it.** `CalendarRow` carries
**`is_default`**, and exactly one row has it whenever any calendar can be written to. The order is:
the user's stored choice (Settings → Calendar) **while that calendar still exists and can still be
written to**, otherwise the first writable calendar.

Both halves of that condition are load-bearing. A choice outlives its calendar when an account is
removed or a calendar is deleted server-side, and it outlives its *writability* when a share is
downgraded to read-only, and a default that refuses the write fails at save time, with the event
already typed, which is worse than never having offered it.

Resolved once, in `calendar_colors::mark_default`, so a client reads `is_default` instead of keeping
a fallback rule of its own: four of them would drift, and the drift would show up as the calendar
Settings names not being the one the editor opens on. It is also **the colour a slot drawn on the
grid wears** (§10): a new event's block says where it is going before the editor opens.

### Device time-zone changes

The active display zone never changes merely because the device moved. A client reports a new IANA
device zone to the core; `TimeZoneSnapshot::pending_device` then raises one choice: **Update** the
display zone, or **Keep** the active one. Only Update persists the new zone and rebuilds the diary.
Closing the prompt means Keep. Linux checks the device zone once a minute while the app remains open
and drives both answers through the same core state machine as the manual Settings picker.

---

## 2. A page is a week. Views are zoom levels.

**Day, 3-day, work-week and week are not four views. They are four zoom levels of one grid.**

A grid page is always a **whole week**. That week is:

- the unit the **core is queried in**, and a page is pulled, painted and cached as;
- unchanged by any zoom. Zooming only decides how many of its seven columns are on screen.

**The days never move. Only their width changes.**

It used to be two further things, and on Android it still is: *the boundary a horizontal scroll could
not cross*, and *the thing a touch swipe lands on*. **Windows, macOS and iOS/iPadOS have taken both
down** (§6): their days are one continuous strip that a scroll runs straight through, and it comes to
rest on a **day** rather than a week. Windows had no choice, a trackpad hands it a pan it cannot see
the end of and a grid that must land on a week can only guess; Apple has the phases and took the strip
anyway, because it is the better model on any input and it is what Apple's own Calendar does on both
the Mac and the phone. What survives untouched is everything this section is actually about: the page
is still the week the core is queried in, and a zoom still moves no days. The week stopped being a cage
and stopped being a resting place; it did not stop being the unit.

### Why (this is the rule with a body count)

The first Android build snapped a pinch to a differently-anchored "week view". A Monday-aligned week
**cannot contain an arbitrary three-day window**. A user reading Sunday, Monday and Tuesday who
pinched outwards was shown the *previous* Monday-to-Sunday: two of the three days they were reading
vanished. It read as a glitch. It was the design, and no amount of patching inside it would have
helped.

The query is therefore `calendar_range(from, columns)`: **N consecutive days from an anchor, snapped
to nothing.** Widening three columns to seven keeps the same first day.

---

## 3. Week alignment is a deliberate act, not a side-effect

The core owns **which day a week begins on**, a persisted setting (`WeekStart`), defaulting to
**Monday** (ISO-8601, and where this product's users are).

It is deliberately **not** derived from the device locale. A locale default is invisible and
unoverridable: a user on an `en-US` phone would silently get a Sunday-start week with no way to say
otherwise.

This is not cosmetic. **Get it wrong and every column of the grid shifts, so the user reads Tuesday's
meetings under Monday's heading.**

But alignment is applied only when a client **asks** (`week_start_date(date)`), and it asks on a
**seat**: the grid's first seat from the core, a shape picked from the menu, "back to today", a
week-start change. **Never on a zoom.** Aligning on every zoom is exactly the jump §2 exists to prevent,
and the difference between the two paths is a real one in the code (`SetMode` aligns, `SetZoom` does
not).

### How the aligned week is framed when it opens

Alignment fixes *which* days a page holds; framing is *which of them the client scrolls to* when the
grid opens or jumps home. That is a client concern (the core emits no pixels), but it is a **shared
product decision**, kept identical on Android and Windows and held down by one tested helper on each
(`framingColumn` / `FramingColumn` / `calendarFramingColumn`), never re-derived per view:

- **Work week** opens on the week's first day and shows five days (Monday to Friday) **whatever day
  it is**. "Work week" *means* Mon–Fri; framing it on today would open a Tuesday on Tue–Sat, putting
  Saturday in the view whose whole point is the working week. (The weekend is a scroll away.)
- The whole **week** opens on the week's first day too, **and the helper must say so, not leave it to
  a clamp.** For a long time it returned *today's* column here and the surface quietly clipped it to
  zero, because seven columns fill the viewport exactly and there is nowhere to scroll to. That is a
  rule holding only because of a bound somewhere else, and it collapsed the moment Windows took the
  bound away (§6): with a continuous strip, framing on today opens the grid on a week that **begins**
  on Tuesday: a Tuesday under the first heading, which is the exact misalignment this section exists
  to prevent. Both helpers now return `0` outright. On Android that changes nothing it renders; it
  removes a trap.
- **Day** opens on today. **3-day** opens on today plus the next two, today at the left edge.

So the two wide shapes frame on the week start; the two narrow ones frame on today.

> **A divergence to know about, and it is the strip's** (§6): on a **Sunday**, the 3-day zoom means
> Sunday–Tuesday, running across the week boundary. Windows, macOS and iOS/iPadOS now show exactly
> that. Android's days cannot leave their week, so its scroll clamps back to Friday–Sunday: today at
> the *right* edge, not the left. The shared helper agrees on all of them (column 6); it is the bound
> underneath that differs. The strip's answer is the one the rule always described.

### "Back to today" appears only when it would move you: *unless it lives in a cluster*

A cross-platform product decision (Apple and Android): the "back to today" affordance is shown **only
when today is not already in view**, so it never sits in the bar as dead chrome. "In view" is the
current *period*, not the sub-set of columns a narrow zoom shows:

- **Grid**: today is within the current **week page** (`today ∈ [anchor, anchor+7)`). So on today's
  week the button is hidden at every zoom, even if a narrow zoom has today's own column scrolled off
  the side, because that is the week the jump lands on.
- **Month**: the anchored month is today's month.
- **Agenda**: always hidden, the agenda always contains today, so the jump would be a no-op.

It reappears the instant a swipe takes today off screen, and disappears again on the way home.

**Windows shows it always, and that is not a lapse; it is the same principle one level up.** On a
desktop the button is not alone: it is the middle of a `< Today >` cluster, because a mouse with no
horizontal wheel (which is most of them) has no other way to reach next week. Hide the middle of three
buttons and the other two *move*: the chevrons slide 90px sideways, under the very cursor that was
aiming at them, every time the user scrolls past today's week. **A navigation control that moves while
you navigate is worse than one that is briefly redundant.** The rule is really "no dead chrome"; where
the button stands alone, hiding it serves that, and where it anchors a cluster, hiding it defeats it.

---

## 4. Nothing is hidden without saying so

Three places where a client must cap what it draws. In each, the cap is a client decision (it is a
question of how much room *this* screen has) but the **rule** is shared.

### The all-day banner

- Shows at most **3 lanes**. Past that, the last row becomes a per-day **"+N"** chip, and the banner
  is tappable to expand.
- Exactly 3 lanes fit with **no** overflow row: a "+1" that hides an event for no reason is worse
  than nothing.
- **The counts are per day column.** A hidden multi-day bar is hidden on *every* day it covers, so a
  three-day offsite pushed out of view adds one to three different columns. A single global "+N"
  would be wrong on every column but one, and **a "+1" that should say "+2" is a lie the user cannot
  see through**: they tap, find an event nobody told them about, and stop trusting the banner.

### The month cell

- Shows what fits, then **"+N more"**.
- **The overflow row only earns its place when it stands for more than it displaces.** With capacity
  4 and 5 events, drawing "+N more" *costs* a slot, so it draws 3 and says "+2". Drawing 4 and
  silently dropping one is not an option; saying "+1" would be a lie.

### An event block's label

- A block draws its title **only if it has room for one**, and that is a function of the **zoom**, not
  a constant. Zoomed out, a 15-minute event is a few pixels tall and *cannot* hold text.
- It stays a coloured block, **keeps its full spoken label** for a screen reader, and reveals its
  title when the user zooms in.
- Getting this wrong does not hide the title: it **cuts it through the middle**. (Android shipped
  that for one build. The grid was geometrically perfect and looked broken.)

### `is_materialized`

**`false` does not mean "no events".** It means the engine has not expanded that far yet. A client
**must** say so ("loading this period…") rather than render a confidently empty week, which is a
lie that looks exactly like a real answer.

The core owes the client the same honesty, and it costs one rule in the cache: **the materialized
window is claimed on evidence**: a calendar in the store, or an account set that is fully
connected and has no calendar in it. A rebuild runs whether or not the sync in front of it reached
anything, so without the rule a launch with no network over a never-synced store would claim a
window it filled from nothing, and draw that confidently empty week on every launch.

Both halves of that are load-bearing, and the second is why the rule is not simply "claim it if the
store has a calendar":

- An account that connected and **reported no calendar** will never produce one. Withholding the
  window there leaves "loading this period…" on screen for the life of the account, over a grid
  that is empty as a matter of fact.
- An account **nobody has dialed yet** looks identical by that test, and is the opposite case. What
  separates them is that a boot placeholder has no providers *of any kind*, so "has a mail provider
  but no calendar provider" is the shape that means *asked and answered*.

Both are decided in `rebuild_calendar_cache`, once, so no surface can hold a different opinion.

### Participation: an unanswered hold, and a declined meeting

Every occurrence carries `participation`: how **this account** answered, matched against its whole
address set (primary + aliases), not one identity. Two rules follow, and they are the two halves of
the same discipline:

- **`needs-action` is drawn dotted**: a dashed border and a hatched leading gutter, because the
  organiser is still waiting. A **dashed border is invisible to a screen reader**, so the
  accessibility label must *say* it ("Awaiting your response",
  `a11y_invitation_awaiting_response`). Visual and spoken, never one without the other.
- **`declined` never reaches a grid, month cell or agenda row.** The core filters it out at the one
  point where occurrences meet their master events, so every surface agrees and a client cannot opt
  out. This is a **core** rule because provider behaviour is not uniform (Exchange removes the
  event, Google keeps it, CalDAV and JMAP keep the object), and inheriting four behaviours would
  make the calendar unpredictable.

> **The declined rule hides data, so it says so.** The event still exists; we hide, we do not delete:
> it stays on the server and in the store, and it comes back the moment the answer changes. **The
> invitation email is the way back**: its card shows the current answer **and, since 0.13.0, changes
> it**: accepting a meeting you had declined puts it back on the grid. Note what is *not* a way back:
> **search is mail and contacts only**, so a hidden event is not findable through it
> ([`search.md`](search.md)). Full contract: [`invitations.md`](invitations.md).

An event with **no attendees** is the user's own appointment, and an event with attendees **none of
whom are us** (a room booking, a colleague's event on a shared calendar) is still ours to keep. Both
read as commitments, never as unanswered holds. Getting that wrong draws a user's own diary dotted
and lets a new invitation claim the slot was free.

**So is a meeting *we* called.** An `ORGANIZER` line carries no `PARTSTAT`: it is not an answer
slot, and RFC 5545 defaults a missing one to `NEEDS-ACTION`, so a server that decodes the organizer
into the participant list (SabreDAV/CalDAV routinely does) reports the person who *called* the
meeting as not having replied to it. Read literally, the user's own meeting is drawn dotted and the
conflict rule (which skips unanswered) then tells the next invitation the slot is free. RFC 5546
§3.2.1 has the organiser attending by definition, so the answer is not "we do not know", it is
"yes". Only the **absent** answer is inferred: an organiser who explicitly declined their own
meeting keeps that answer, and the declined rule above then hides it. This is the same rule the
invitation card's attendee tally already applies ([`invitations.md`](invitations.md)), and it must
stay the same rule: a meeting cannot be a commitment on the grid and an unanswered hold in the
tally.

> Two representations, one rule. JSCalendar merges an `ORGANIZER` and the matching `ATTENDEE` into
> **one** participant carrying both roles; a plain iCalendar server can leave them as two lines that
> decode to two participants. The test is therefore "is any **owner** at one of our addresses", not
> "does the attendee we matched own this": the second misses the split shape, and the address is
> the same in both.

### The attendee list (the detail view's roster)

The event detail carries **`attendees`**, one row per person: their name (empty if the event
carried none, so a client shows the address), their address, whether they called the meeting, and
how they answered. It is the roster the invitation card's tally is a *count* of, and the two must
never describe one meeting differently, so both read a participant's answer through the **same**
function (`mailcal_viewmodel::effective_response`), including the organiser rule above. A meeting
that tallies "2 accepted" over a roster showing one of the two as unanswered is one screen
contradicting another.

Three decisions the core makes once, so no client repeats them:

- **One row per address**, not per line. The split shape above would otherwise print the organizer
  twice, and only on the servers that split them. The merge keeps the **explicit** `PARTSTAT` over
  an absent one, and that test is made on the **raw** status rather than the mapped answer: the
  organiser inference turns an absent answer into `Accepted`, so a mapped test would let a bare
  `ORGANIZER` line silently re-accept a meeting its own `ATTENDEE` line declined.
- **A participant with no address is skipped**, exactly as the tally skips it: a roster longer than
  the "of N" beside it is a discrepancy with nothing on screen to explain it.
- **The organiser sorts first**; everyone else keeps the event's own order.

Every name and address is **attacker-controlled** (it came from whoever sent the invitation) and has
been through `plain_text`: control characters and bidi overrides out, whitespace collapsed, length
bounded. A client renders it as **text, never markup**: `use_markup(false)` on GTK.

**Attendees are read-only, on every surface.** Changing who is on a meeting means sending iTIP
updates to the people on it, which is its own feature. So the editor **shows** the list and states
that it cannot be changed there, rather than offering a control that would quietly drop the change,
and the detail view shows no add/remove affordance at all (an affordance that can
never fire is just a mystery).

**The picture is per surface; the disclosure is not.** A grid block and an all-day bar take the dashed
border, the hatched gutter and a faded fill. A **month chip** is a few points tall, so the hatch
carries it and the dashes are decoration. An **agenda row** is a list item with no border to dash at
all, so it prints "Awaiting your response" on the row, which is the same disclosure the dashes only
stand in for. What may **not** vary is the spoken label: every surface that can show a hold appends
it, on every platform.

> **Where the spoken half goes is per toolkit, and getting it wrong is silent.** On GTK an explicit
> accessible **label** loses to a `labelled-by` relation, which a libadwaita row and any labelled
> button both have, so the disclosure is a **`Description`** there (announced after the name, no
> competing relation), and only the block on a Cairo surface, which has no label of its own, takes
> it as a label. GTK exposes no getter for either, so a widget test cannot see any of this: the
> oracle is an AT-SPI run. `AGENTS.md` → the libadwaita row rules.

---

## 5. The grid is a pull, not a pushed snapshot

Every other surface follows the mail pattern: mutate → `observer.surface_changed(...)` → the host
pulls one immutable snapshot. **That breaks under a pager.**

- A grid renders **several weeks at once**: the one in view and its neighbours, so the next swipe is
  instant. **One snapshot slot cannot hold several.** (Android draws five, because its page offset may
  lag two whole pages behind the week it has already landed on; Windows draws at most two, because its
  strip re-anchors and cannot lag (§6). Either way, more than one.)
- `dispatch` is fire-and-forget on a multi-threaded runtime, so two quick swipes **race**: the grid
  can settle on *last* week after the user has already swiped to next.
- The observer is debounced at 250 ms.

So `calendar_range` / `month_page` are **direct, synchronous, argument-taking queries** over an
in-memory cache. They never touch the store or the network. **The client owns the anchor; the core
never learns where the user is.** A pull cannot arrive out of order.

`Surface::Calendar` survives, demoted to a **cache-invalidation signal**: "calendar data changed:
re-pull whatever you are showing."

**That signal is not free, and the core must not cry wolf with it.** It invalidates every page on
screen, and the host re-pulls all of them: *synchronously, on its UI thread*, which is the deal a pull
architecture makes. So:

- **A refresh that changed nothing signals nothing.** In the steady state a sync brings back no
  changes and rebuilds a byte-identical cache; signalling anyway buys a full re-layout for nothing.
  Land that while the user is mid-swipe and the fling stalls part-way through: indistinguishable
  from the page sticking between two weeks.
- **A page query allocates nothing it does not have to.** It runs once per week composed, several per
  swipe. Cloning the cache's ~1,100 occurrences (each several heap strings) on every pull put thousands
  of allocations inside the fling. It borrows.

### The grid paints from the store, and opening it costs no round-trip

The cache primes at launch from what the store already holds: no network, no expansion. Mail has
done this from the beginning; the calendar did not, and the omission was invisible in every test and
brutal in the hand: the cache was built by *nothing but the network refresh*, so opening the calendar
meant seconds of "loading this period…" over a store that had held the week all along.

Priming runs **off** the boot blocking path: it is a few hundred milliseconds of SQLite over a real
calendar, and the mail list must not wait for a surface the user has not opened. And it does nothing
at all on a store that has never synced: priming an empty store would set the window and flip
`is_materialized` to `true`, showing a first-run user a confidently empty week, which is precisely
the lie §4 forbids.

### The calendar syncs because the app has the account, not because the user opened it

Painting from the store is only worth anything if something puts a calendar in the store. Until
this rule existed, exactly one thing did: **the user opening the calendar tab.** A user who never
opened it had no calendar at all (not a stale one, none), so the grid stayed on "loading this
period…" for good, an invitation read on the way past could only answer "we have not looked" for
the rest of the session, and with no network there was nothing to fall back on.

So a launch fetches the calendar on its own, and the paint and the fetch sit in **different
places**, which is the part that has to be got right:

- **The paint** is spawned at boot, off the blocking path: the stored week goes on screen without
  the mail list waiting for it.
- **The fetch** waits until every account has actually connected (`reconnect_all`). An interactive
  boot returns **provider-less placeholders** so cached mail paints at once and dials afterwards, so
  a fetch at boot reaches nothing, files nothing, and looks exactly like a fetch that found an empty
  diary.

**Adding an account does the same thing**, in the first download the user is already waiting on:
a new account's store is empty by definition and the launch fetch has already been and gone, so
without it the first session is exactly the one with no diary in it.

Two properties this must keep:

- **It is not an `Intent`.** Dispatching `RefreshCalendar` would record the calendar as a feature
  the user used, on a launch where they may never open it: a launch is not a visit
  ([`analytics.md`](analytics.md)). Every one of these paths calls
  `refresh_calendar_in_background` directly.
- **A failed fetch changes nothing on screen.** The primed grid is already up; a sync that cannot
  reach the server neither blanks it nor claims a window (§4).

### A foreground diary keeps refreshing

A desktop process left open must not rely on another visit to the Calendar tab to become current.
Linux starts a five-minute timer once an account has connected and refreshes on that cadence for the
life of the process, whichever surface is visible. The refresh uses the same no-change suppression
above, so a quiet server causes no redraw. It bypasses the user-action intent, so it neither records
calendar adoption nor dismisses a failed-write warning; only the warning's explicit retry does that.
The other clients' standing cadence remains a known gap.

### The reads that build both surfaces are windowed, so a full diary stays cheap

Everything the calendar shows (the grid **and** the agenda) is built from the events live in the
materialized window, never the account's whole event history. This is invisible on a test calendar
and brutal on a real one: the store's `events()` read decodes *every* event in the account, and the
calendar used to run it **twice** on the boot/refresh path (once to join the window's occurrences to
their masters, once more to project the agenda) and then hand a row-per-event agenda across the FFI
for the host to reconcile on its UI thread every refresh. Measured on a ~9,800-event account, the
cache rebuild alone was **~5 s** and the agenda was **9,888 rows** reconciled in ~350 ms: a wait,
and a jank, for data the window does not even show.

So the join and the agenda both resolve **only the masters an in-window occurrence points at**
(`Engine::events_by_keys`), and the tap-to-open detail read resolves **the one key it was handed**
rather than scanning the account for it: that scan was the calendar tap that took *seconds* to open
on a large diary. The consequence for the contract is small but real: **the agenda is the same
windowed set the grid draws.** An event with no occurrence in the horizon is still stored and still
opens by key (its detail read is targeted, not windowed), but it is not in the agenda list: the
agenda is "what is on in the window we have looked at", not "every event ever synced". Widening the
window (the `is_materialized`/widen path §4 anticipates) widens the agenda with it.

And a refresh that changes nothing now **holds no write lease**. `refresh_calendar`'s re-expansion
(`Engine::expand_horizon`) used to re-derive *every* stored event over the whole horizon on every
call (thousands of events, under a scope lease held for seconds), so a tap that landed mid-refresh
blocked on that lease and took the very seconds the targeted read had just removed (a real 5 s stall,
observed on a full diary). The engine now skips a scope whose materialized window already matches the
one asked for, so a steady-state refresh (or the first one after a same-day relaunch) is a handful of
cheap reads, not a lease-bound re-expansion. A horizon advance or a display-zone change still
re-expands (the window carries both), so nothing a user pages to goes unmaterialized.

---

## 6. One gesture, one owner

**One handler reads the pointer stream, and it decides: pan, page, or zoom.** Not a pager plus a
vertical scroll plus a horizontal scroll plus a pinch detector, each reading the same finger and none
of them able to see the others. That arrangement is not tunable, and the two years of scar tissue
below are all the same wound.

**A consumed pointer event does not politely ask a scroller to stand aside: it CANCELS its drag. And
Compose's scrollable flings only when a drag *ends*, never when it is cancelled.** No fling, no
settle: the pager stopped dead wherever the finger was, and the grid sat between two weeks, forever.
The clue was in plain sight the whole time: it never once reproduced under synthetic single-pointer
swipes. Only a *second contact* triggered it, because only then did the pinch detector run, and only
then did anything consume.

So the pinch was made to consume nothing. That fixed the swipe and broke the zoom instead: the
scrollers underneath went on reading the same two fingers and panned the week around while it zoomed.
**Both failures were the same root (four handlers, one finger) and neither was fixable from inside
it.** With a single owner, consuming is free (there is nobody to consume *from*), and the pan is
simply not fed during a pinch.

- **The day strip takes a sideways drag first; the week gets the remainder.** *(Android. Windows has
  replaced this: see "The days are one strip" below.)* That is what a nested scroll gave for free, and
  writing it down is the price of taking the nesting away. A hard flick from mid-week carries through
  the end of the week and turns it, in one gesture: leftover fling velocity is handed to the page, or
  the grid grows a wall you can feel.
- **A turn opens the new week on its first day.** *(Android. On Windows this rule no longer exists: a
  strip has no turn to re-seat.)* The day scroll (`dayX`) is a position *within* a week, so committing a
  turn must re-seat it to the week start: **both directions**. Skip it and a sub-week zoom carries the
  old end-of-week offset across the boundary and lands mid-week: work-week scrolled to its end turned to
  the *next* week showing Wednesday instead of Monday. (It shipped that way on Windows for one build;
  Android had the identical omission, and the direction-dependent first attempt, "forward to the start,
  back to the end", got it wrong the other way, breaking a back-turn that had been landing on Monday.)
  The peeking neighbour is drawn framed at its own first day too, so the week sliding into view is
  already where it will land. In whole-week zoom there is no day-scroll (`maxDayX` is zero), so this is
  a no-op there.
- **A pinch is two contacts, and each is captured and released on its own: the single owner does
  not make the platform's capture bookkeeping single, too.** The owner tracks a *set* of pointers and
  finalises only when the last lifts; the platform shell that feeds it must mirror that exactly. The
  Windows port first gated the whole gesture on one "is a finger down" flag: the first finger's
  release flipped it, the second finger's release was skipped, and that contact's capture was
  stranded, so the OS thought touch was still owned by a lifted finger and routed every new touch to
  nowhere. **The touchscreen went dead while the touchpad (which sends the wheel, not a captured
  pointer) kept working, and the leaks piled up until the app fell over**: a full minute of use
  before it showed, because it took several pinches to strand enough captures. Capture per id, release
  per id, forward *every* release; and wrap the handlers so a fault ends the gesture cleanly rather
  than crashing or stranding a contact. Pinned by `CalendarMultiTouchTests` (both fingers up ⇒ the
  owner is empty-handed). **The half that is *not* pinned is the shell's own capture set** (the
  bookkeeping that actually leaked) because it needs a window; an injected-touch soak (`touch.ps1`:
  twenty pinches, then a swipe must still turn the week, with the handle count flat) would cover it and
  **has not been written**. Named in "Known gaps", not left as an implication. Apple and Linux inherit
  this the day they grow a second contact.
- **Between two weeks is never a resting place**: *where a week is a page.* Whatever ends a gesture
  (a lift, a cancel, a system dialog), the week lands. Run the settle in its **own** coroutine, and never
  re-enter the public entry point from inside one: an animation that cancels the job it is itself
  running in is a grid killing its own settle, which is exactly what "it sits there forever" turned out
  to be.
  **Windows has since dissolved the premise rather than the rule** (see "The days are one strip"): its
  weeks are laid end to end with the hour ruler pinned beside them, so a grid showing Wednesday to
  Tuesday is showing seven days, not half of each of two pages, and there is no longer any such place as
  "between two weeks". The rule stands wherever a page is still a page (which is Android, today) and
  the *reason* for it stands everywhere: **a grid must never come to rest in a frame it cannot explain.**
- **A turn is decided before it is drawn, and a decision a later event can undo is not a decision.**
  Commit the week the instant the flick is judged, and rebase the page offset by the width it moves:
  the frame is *identical*, and all that has changed is that the week is now banked. What follows is
  only the pixels catching up. *(On Windows there is no page offset to rebase: the decision is a
  remembered landing week that outlives the animation the next finger cancels. Same rule, and it had to
  be re-derived from scratch: see "The days are one strip", "What survived".)*
  This is the difference between "snappy" and "it ate my swipe". Commit at the *end* of the slide
  instead, and a second flick arriving mid-flight cancels the animation of the first and with it a
  week the user had already won: its progress surviving only as a partial offset, which is capped at
  one page. Two flicks then add up to one week. Measured on a real phone: **eight fast flicks turned
  three weeks.** After: eight turn eight.
  The price is a **lag** (the image trailing the week it has already landed on), and the grid must
  hold what it slides *through*, or draw a hole where a week should be. *(Android:)* at a lag of `f`
  pages it draws pages `(-1 - f) .. (1 - f)`, so the lag is capped at two and five live pages cover it.
  *(Windows:)* the strip re-anchors as soon as its left edge crosses a boundary, so the offset is always
  inside its own week and **at most two weeks can be on screen**: the lag cap and the five live pages
  are gone, and the hole they guarded against is unreachable by construction.
- **A gesture is judged on what its own finger did**, never on where the page is sitting. Once a turn
  is banked, the page carries that lag, and a lag looks exactly like a drag the other way. Judge the
  second of two fast flicks by the page's position and it reads as "he has changed his mind", and
  cancels itself. That is the swallowed swipe, hiding as arithmetic.
- **The settle must be quick.** Measured against Samsung on the same phone: their page turn takes
  0.02–0.15s, ours took 0.32–0.50s with Compose's default spring. That, and not the drag threshold,
  is what makes rapid swiping feel like fighting the app: a new flick lands on a grid still gliding
  from the last one. Critically damped and stiffer: no bounce, because a week that springs past its
  own column makes the day headings visibly overshoot the columns beneath them.

### The days are one strip, not a stack of pages *(Windows, macOS, iOS/iPadOS)*

**The bug.** Scrolling slowly on a trackpad, the calendar rubber-banded home six times in thirteen
seconds. Each snap was the grid doing exactly what it had been told: a wheel has no lift (below), so it
guessed the gesture had ended from a moment's silence, found a drag too short to commit, and sprang the
week back under the user's fingers. **A slow pan is mostly silence.** No threshold fixes that, because
the gesture that trips it is *a pan that has not finished yet*: tune the threshold down and an idle
wobble turns the week instead.

The snap was not the disease. The grid snapped because a week boundary was the only frame it could
*explain*: each page carried its own hour ruler and slid out with it, so coming to rest halfway left a
second column of `00:00–23:00` stranded down the middle of the screen and no ruler at all at the left
edge. Given that, snapping was the least-bad thing it could do.

**The fix is to make the resting frame explicable, and then stop resolving pans at all:**

- **The hour ruler is chrome, not content.** Drawn once, pinned to the left edge; the days scroll past
  it. The weeks are laid end to end with **no gutter between them**, so day *n* of week *k* is at
  `(k − anchor) · weekWidth + n · dayWidth − stripX` and nothing else. Verified by the renderer's own
  geometry, through the accessibility tree: the gap from `Sun 19` to `Mon 20` is the same as from `Mon`
  to `Tue`.
- **The horizontal axis is one continuous offset.** It was three coupled numbers (a week index, a page
  offset, and a day offset within the week) routed through a nested-scroll hand-off. All three moved
  the same days by the same pixels; the split was bookkeeping, and it was buying a wall at every week
  boundary. Now it is an anchor week plus `stripX ∈ [0, weekWidth)`, re-anchored when the left edge
  crosses a boundary, which moves the grid by *exactly nothing*, because the two terms cancel.
- **The banner's height is the largest of the weeks on screen**, not the current page's own. This is
  what a pinned ruler *costs*: the grid's `00:00` is where the ruler's `00:00` is, so a seam with a
  three-lane week on one side and an empty one on the other must still have one content top, or the
  hour lines would meet the ruler on one side and miss it on the other.
- **The grid comes to rest on a day. One rule, every zoom, every input.** Not a week, and not
  wherever the pixels happened to stop. A day is the smallest unit that puts a column edge against the
  grid's left edge, so it is the least the grid can move and still look deliberate, and a landing at
  most half a column away can only read as *settling*, where a week-sized one read as being overruled.
  The zoom stopped mattering to this: the user asked for narrower columns, not for a different idea of
  where the grid may stop.

  **Why not a week.** A week-sized landing drags the days sideways by up to three and a half of them
  that the user never asked to move, and it quietly undoes the framing they just chose. Worse, deciding
  a week needs a *threshold*: travel far enough and it commits, fall short and it springs home, and a
  threshold discards what the user did. It also needs to know that the gesture ended, which on a
  touchpad is unknowable: a lift, the OS's momentum and an active pan are the same burst of wheel
  messages. Guessing from silence is what rubber-banded the grid home six times in thirteen seconds. A
  day boundary needs no threshold and no judgement: it is just the nearest one.

**What this deleted.** The whole page-turn apparatus: the sixth-of-a-week threshold, the velocity
judgement, the change-of-mind rule, and the banked landing the driver carried across a cancelled
animation. That last one existed to guard a real bug: a flick banks its week before a pixel moves, so
a second flick landing mid-slide finds the strip a page behind the week already won, rounds to the
boundary *behind* it, and re-targets a week already taken (measured with the driver's own clock,
mid-rewrite: **eight flicks turned six weeks**).

**A flick now banks nothing.** It adds speed to a strip that coasts, and the landing is whichever day
the coast ends nearest. There is no decision for a later event to disagree with, so the class of bug is
gone by construction rather than guarded against, and a second flick mid-coast simply adds its speed,
which is what a hand expects. `CalendarFlickTests` pins that none is swallowed, and that a reversal
reverses immediately. What it deliberately no longer pins is that *n* flicks travel exactly *n* weeks:
a coast carries as far as its speed is worth, which is the point.

### On a desktop, the wheel is part of the pointer stream

A phone has one input. A desktop has three, and **all three belong to the same owner**: this is the
same rule, not a new one, but it is invisible until you meet it.

**A precision touchpad never gives an app its raw contacts.** Windows digests them and delivers a
two-finger pan as **wheel** messages, and a pinch as **Ctrl+wheel**. A mouse sends the same wheel. So
if the wheel goes to a `ScrollViewer` while touch goes to the grid's own handler, you have rebuilt
*exactly* the four-handlers-one-finger arrangement above, in a costume that does not look like it,
and it will fail the same way, because it is the same bug. **There is no scroller anywhere near the
grid, and there must never be one.**

Two consequences fall out, and both are stated rather than hidden:

- **A wheel has no "up", and no phase at all.** A touchpad's pan is a stream of notches that simply
  *stops*: Windows never tells an app when the gesture began, ended, or turned into inertia; a lift,
  the OS's momentum scroll, and an active pan are the *same* burst of wheel messages (browsers hook
  DirectManipulation precisely to recover this, which we do not). Silence is therefore the only
  end-of-gesture signal there is, and it is used for exactly two things: banking a Ctrl+wheel zoom's
  shape to the core once, and bringing a pan to rest on a day.

  **An idle window must be longer than the gap between two notches of the same gesture**, or it
  resolves a gesture that has not finished, which is the rubber-band, in miniature, forever. A mouse's
  measured gap is **~150 ms**. The zoom's window was 60 ms, so *every notch* was a finished gesture:
  against a real diary, seven settles in two seconds, each a core write plus four snapshot reloads of
  33–111 ms on the UI thread, mid-pinch. The windows are 250 ms for a pan and 350 ms for a zoom, and
  `CalendarWheelTests` pins both against that measured gap.

- **A notch asks for travel; it does not move the grid itself.** Applying each notch the instant it
  lands teleports the strip once per notch and leaves it perfectly still in between. Measured on a real
  mouse at ~150 ms: 24 notches over 3.8 s drew **16–31 fps**, at a *mean frame cost of 6.6 ms* against a
  16.7 ms budget. The frames were not slow: they were never asked for; a 6.5 Hz staircase, reported as
  "the scroll stops at random points". So a notch adds to a **target** and the strip eases toward it
  (exponentially: a fixed fraction of what is left, so it is quickest when furthest behind and cannot
  overshoot). The travel outlives the notch that asked for it, which is what makes a mouse's sparse
  notches and a trackpad's dense stream the same one continuous motion.

  Two shell-level corollaries, both of which cost a frame per notch on their own: the tick loop's clock
  must be seeded when the loop *starts* rather than on its first callback, and an input handler must
  mark the surface dirty rather than merely starting the loop.
- **A touchpad pinch is a scalar, so it can only zoom one axis.** `Ctrl`+wheel carries a single
  number; there is no second component to drive the day axis with. So on a touchpad the pinch zooms
  the **hours** and nothing else, and the diagonal pinch needs a touchscreen. This is the same
  shortfall macOS has, for the same reason: see "Known gaps".

---

## 7. Frame budget

The grid is judged against the platform's own calendar, and the bar is **not** the average frame:
both hit 120 Hz at rest. It is the frames *missed during motion*: measured against Samsung Calendar
on the same device, ours dropped **11.5%** of in-motion frames where theirs dropped 2.4%. A median
that looks identical hides that completely.

**Measured, on a real diary.** The composable grid against the canvas, same phone (S24 Ultra), the
same 1,100-occurrence personal calendar, the same scripted swipes, both **release** builds, twice each:

| in-motion frames | composable grid | canvas | + the week banked on decision |
|---|---|---|---|
| **dropped** (gap > 12.5 ms) | **20.7% / 19.5%** | 7.4% / 6.5% | **3.6%** |
| p90 gap | 18.3 / 18.7 ms | 10.1 / 10.0 ms | 9.2 ms |
| p99 gap | 32.4 / 29.0 ms | 25.4 / 25.6 ms | 16.6 ms |
| frames *delivered* during the motion | 341 / 360 | 447 / 460 | 433 |

Three times fewer dropped frames, and the canvas **delivers a third more frames**, which is the same
fact said twice: those are the frames the old grid was missing. Samsung's 2.4% is now within reach of
our 3.6%, from 20%.

The last column is not a rendering change at all. Committing the week the moment a flick is *judged*,
rather than when its slide *finishes* (§6), deleted a hitch at the **end** of every fling: the old
commit fired as the animation landed, taking a recomposition and a fresh page-paint with it, exactly
when the eye was watching the page arrive. Bank the week first and the slide runs clean to zero. A
correctness fix for the swallowed swipe, and it halved the dropped frames as a side-effect, which is
usually the sign the bug was never really about performance.

**Do not use `gfxinfo`'s "Janky frames %" for this.** It said the two were a wash (7.5% vs 6.2%), and
it was measuring nothing: the two grids do not render the same number of frames, because the canvas
settles and goes idle while the old grid keeps animating into the pause. A ratio over two different
denominators compares two different questions. The number that answers *what the eye sees* is the
**gap between one frame landing and the next, during the motion**, which is what the `mpdecimate`
recipe below was always reaching for, and what `dumpsys gfxinfo <pkg> framestats` gives you directly,
per frame, without a video.

Three rules keep it there, and all three were learned by breaking them:

- **The grid is drawn, not composed.** One canvas, not a `Box` per event. A composable per block means
  a layout pass per block on every frame of a pinch, and the grid's own draw is trivial by comparison:
  it is rectangles and short strings. What a frame is *allowed* to do is multiply the core's
  unit-free geometry by an hour height and a column width, and cull whatever falls outside the
  viewport. Everything else was decided before the frame began.
- **Compose the neighbouring pages, don't build them inside the fling.** A pager composes only the
  page in view unless told otherwise (`beyondViewportPageCount`). The next week (its page query
  *and* its sixty-odd event blocks) was therefore built at the moment the swipe began, on the UI
  thread. That is a hitch you feel as the page hesitating on its way across. The canvas keeps its
  neighbours in hand and holds them *across* a turn, so only the week that just came into reach is
  built: **five** live pages on Android (the offset may lag two whole pages behind the week it has
  landed on, so the grid slides *through* weeks it must be able to draw), and on Windows **two** drawn
  (the strip re-anchors and cannot lag) behind a **±4-week cache** painted on idle frames, which is the
  same idea moved off the fling entirely (§13).
- **Nothing that a zoom cannot change may be re-derived on a zoom.** A pinch changes the hour height
  every frame. Parsing hex colours, formatting a clock and building a localised accessibility string
  out of resources (per block, per frame) is far more expensive than the arithmetic they sat next
  to. Hold them, and make the type system hold them: the renderer is handed a page in which the
  colours are already `Color`s and the labels already carry their `TextStyle`, so there is nothing
  left to derive.
- **The one thing a zoom genuinely changes is the text layout: so stop it changing.** A column's width
  moves every frame of a pinch, the shaper's cache is keyed on that width, and so every visible label
  is re-shaped from scratch, sixty times a second, in the gesture the grid is judged on. Measured on a
  real diary: a pinch frame cost **3.4× a swipe frame** (1709µs against 496µs) while drawing *half* as
  many blocks: backwards, and the tell. Bucketing the width only delays the miss.
  **Freeze the width the text is shaped against for the length of the gesture.** The block's rectangle
  still tracks the fingers every frame, as it must; it is the layout *inside* it that is held, and it
  is clipped to the live rectangle anyway. The cost is that a title ellipsises against the width the
  block had when the pinch began: invisible, because nobody reads a label while it is moving. It
  re-shapes when the fingers lift. Per block, that is **3.2× cheaper** (129µs → 41µs), the worst frame
  falls from **5.9ms to ~2ms** of an 8.3ms budget, and the pinch delivers 90 frames a second where it
  delivered 68.

### A drawn grid has to be taught to speak

A canvas has no accessibility tree: that is the bill that comes with the frame budget, and §4's
"keeps its full spoken label" does not bend for it. So the grid materializes an invisible node per
event **when a screen reader is actually listening**, and not otherwise: the nodes cost layout, and a
pinch would pay for them sixty times a second for a service that is not running.

**The nodes must be placed by the renderer's own geometry, not by a second copy of it.** A screen
reader announcing an event somewhere other than where it is drawn is a bug that no sighted test, and
no sighted developer, will ever see.

Measure it, don't eyeball it. Either `mpdecimate` the recording to drop identical frames, or (better,
because it needs no video and reports the frames the *compositor* actually saw)
`dumpsys gfxinfo <pkg> framestats` and diff the completion timestamps. Then look at the gaps **under
60 ms** either way. A larger gap means the user paused; counting that as a dropped frame is how you
end up chasing idleness. (I did.)

---

## 8. Zoom

Both axes zoom, and both **anchor on the fingers**.

- After scaling by `f`, the scroll becomes **`(scroll + focus) × f − focus`**. Without it the offset
  stays fixed in *pixels* while the scale changes, so the same offset maps to a different time and
  **the grid slides out from under the user's hand**: appearing to zoom about the top of the day.
- Each axis is corrected by **the factor its zoom actually applied**, not the one it was asked for. At
  a clamp that is `1`. Correcting by the requested factor there drags the grid on every further frame
  of a pinch that has nowhere left to go, and lets an exhausted hour axis drag the day axis to a halt
  mid-diagonal.
- The axes are **independent but not exclusive**: an axis the fingers are not meaningfully spread
  along reports "no change" (a purely sideways pinch leaves a few noisy pixels of vertical spread, and
  dividing by that lurches the hours about). Spread them at an angle and **both** zoom, each by its own
  component. That is diagonal, and it is the behaviour worth having.
- The **horizon** (visible hours) and the **shape** (`CalendarLayout`) are persisted core settings,
  clamped by the core: a pinch runs off the end of its own gesture constantly, and a client that sent
  the raw value would leave one platform showing a 1-hour day. Persisting them in the core, not the
  client, is also what stops the phone and the desktop opening on different calendars.
- **A settled pinch snaps to a rung (1, 3, 5 or 7 columns) and the grid opens on the whole week.**
  The shape is a persisted core setting with exactly four values, so a zoom that settles between rungs
  has nothing to save. Snap to the settled **level's** columns, not to the rounded count: a pinch
  outwards from the week lands on ~6.4, which rounds to **6** while the level it maps to is the whole
  week, of **7**, and six columns of a seven-day page is a week with a day hanging off the side.
  *(Android:)* that overhang is not merely untidy: it is a horizontal scroll *nested inside the
  pager*, and a nested scroll takes the drag **first**, so a swipe meant to turn the week is spent
  sliding along the one you are already on and comes to rest in the middle. *(Windows:)* there is no
  nesting left to trip over: the strip is one axis, and a week is a distance along it, so the rung is
  kept for the persisted shape and for the tidy columns, not to protect the swipe.

---

## 9. How this is kept honest

Every rule above was learned by shipping its violation, and most of them **look fine right up until a
hand moves fast.** So they are held down by three things, and the order matters: a rule nobody can
check is a rule that comes back.

### The tests that gate every PR

`./gradlew :app:test`: JVM, no emulator, and it runs in CI on every change.

- **`CalendarSurfaceStateTest`**: the state machine as arithmetic. The day strip takes a drag before
  the week does; a settled pinch lands on a rung; the scroll is clamped when the *menu* moves a bound
  that no finger touched; flicks accumulate; the lag is capped at what the live pages can draw.
- **`CalendarFlickTest`**: **the one that matters, and the one that did not exist.** The swallowed
  swipe needed a gesture to arrive *while the previous gesture's animation was still running*, and
  nothing we had (no unit test, and no synthetic swipe a script can inject over adb) ever did that:
  they all politely waited for the grid to settle. It reproduced only in a hand, moving fast.
  So the test takes the clock away from Compose (`mainClock.autoAdvance = false`) and delivers the
  next flick **one frame** after the last, with the turn still mid-slide. That makes a
  gesture-versus-animation race a deterministic, millisecond-exact JVM test. Restore the old
  commit-at-the-end ordering and it fails exactly as the phone did: twenty flicks, one week.
- **`CalendarGridTest`**: the drawn grid, read the way TalkBack reads it (§7): a canvas has no text
  nodes, so the semantics overlay is the only thing a test *or* a screen reader can see.
- **`CalendarDragTest`**: the drag (§10), in two halves. The pure half is arithmetic: what a press
  meant, where a drop would leave the block, what crosses the FFI. The half worth having is the
  **race**, because a long press is a *timeout* sitting in the same pointer stream as the swipe, the
  pan and the pinch, the four-handlers-one-finger arrangement of §6, wearing new clothes. Two cases
  a hand finds and a script does not: a swipe that must not become a drag, and **two fingers resting
  before a slow pinch**, which is perfectly still and therefore indistinguishable from a hold.
  Both were confirmed to bite by mutation: break the movement rule and the pan test fails; break the
  second-contact rule and the pinch test fails. A third mutation failed *nothing*, which is how a
  redundant line in the gesture owner was found and deleted.

**If you are tempted to test a gesture by waiting for it to settle first, you are testing the case
that already worked.**

### The same rules, on Apple

`swift test` over `MailcalKit`: no simulator, no device. Apple's gesture is SwiftUI's (§13), so the
gesture itself is not testable here; what is, and what carries every decision, is the pure model:
**`CalendarDragTests`** is deliberately case-for-case and name-for-name Android's `CalendarDragTest`,
because drag is a *cross-platform* contract and two clients that agree in prose but not in arithmetic
have not kept it. `EventEditorStateTests` covers the other half of a create: that a slot drawn by
hand opens the editor on the time drawn, while the "New event" button still rounds to the next hour.

### The same gates, on Windows

`dotnet test clients/windows/Mailcal.Tests`: a plain `net10.0` assembly. **No WinUI, no Windows
TFM, no test host, no emulator**, and it runs in ~100 ms as part of `build-and-run.ps1`, so it gates
every PR through the existing `windows` CI job.

That is possible because the grid's whole navigation model (the state machine, the zoom, the
paging, the all-day caps, **the gesture owner and the animation driver**) is free of every WinUI and
Win2D type, and the test project links those very source files: ten of them, ~2,150 lines, every one of
which runs headless.

What is left on the framework's side is a **translation, not a model**: pointer events in
(`CalendarSurface.Input.cs`), a render tick forward, a canvas out. It is not tiny (the shell and the
renderer are ~2,300 lines between them) but it holds **no decisions**. Nothing in it knows what a week
is. *If a change ever drags a `Windows.Foundation.Rect` or a `CanvasControl` across that line, the test
project stops compiling, which is the alarm working.*

- **`CalendarSurfaceStateTests`**: a port of Android's, with the same fixture and the same numbers
  wherever the two clients still answer the same question. Deliberately: this is a *cross-platform*
  contract, and two clients that agree in prose but not in arithmetic have not kept it. Where the strip
  has no Android twin: a drag running off the end of a week into the next, a re-anchor that moves the
  grid by exactly nothing, a pinch reaching back past the week's first day; the test **says so in a
  comment**, so a reader can tell a deliberate divergence from a drift.
- **`CalendarWheelTests`**: the trackpad, and the rubber-band. A slow pan is delivered as a handful of
  small notches and then the wheel simply *stops*, which is all a touchpad ever does; the assertion is
  that the strip is **exactly where it was left**, through the silence that used to be read as a lift.
  It also pins the `WM_MOUSEHWHEEL` sign inversion (a horizontal notch moves the days *with* the hand,
  not with the sign) and the 60 ms idle window that the Ctrl+wheel zoom still needs.
- **`CalendarFlickTests`**: the race, and Windows gets it *more* cheaply than Android did. Compose
  needed Robolectric and a test clock because its gesture owner and animations are welded to the
  framework; here the driver is advanced by an injected `Tick(dt)`, so delivering a flick **one frame
  into the previous turn's slide** is a plain unit test.
  It has now caught the swallowed swipe **twice**, in two different disguises. Restore the old
  commit-at-the-end ordering and it fails as the phone did: eight flicks turn **one** week. And during
  the strip rewrite it caught the same bug wearing new clothes, with the decided week sitting a page
  ahead of the drawn position, judging the next flick against the *nearest* boundary rounds to the one
  **behind** the decision and re-targets a week already taken: **eight flicks turned six**. A landing
  week is added to the week last *decided*, never to the week currently nearest.

**Synthetic touch is real on Windows, and it is still not this test.**
[`clients/windows/touch.ps1`](../clients/windows/touch.ps1) injects genuine multi-touch (see
[`docs/debugging.md`](debugging.md): the old claim that WinUI gestures "cannot be synthesized" was
wrong). It drives real swipes and a real **diagonal** pinch, which is the only way to exercise the
two-axis zoom at all. But it cannot land a gesture one frame into an animation, so it proves the
wiring, not the race. Both, or neither is enough.

### The measurements, on a real device

Neither of the numbers in §6 or §7 can be had from CI, an emulator, or a synthetic swipe, so
[`scripts/dev/calendar-perf.sh`](../scripts/dev/calendar-perf.sh) exists so they are reproducible
rather than folklore, on a **release** build (a debug Compose build is several times slower):

    scripts/dev/calendar-perf.sh frames    # gaps between frames, during motion
    scripts/dev/calendar-perf.sh flicks    # weeks turned per flick thrown, must be 1:1

It refuses to run unless the calendar is actually on screen. That is not paranoia: a horizontal swipe
on the *mail list* is a swipe action, and a stray one archives real mail.

**Two obvious instruments are worse than useless here, and both of them lied to me:**

- **`gfxinfo`'s "Janky frames %"** rated the composable grid and the canvas within one point of each
  other, while one was dropping three times as many frames as the other. A ratio needs a denominator,
  and the two grids do not render the same number of frames: a good one settles and goes idle, a bad
  one keeps animating into the pause.
- **`mpdecimate` over a screen recording** scored the *fixed* build worse than the broken one, on a
  recording a hand could feel was three times smoother. `screenrecord` caps at ~60fps and its encoder
  perturbs the app it is measuring. **Record video to see behaviour; measure timing with framestats.**

### The same measurement, on Windows, and the trap that has to be designed around

[`clients/windows/calendar-perf.ps1`](../clients/windows/calendar-perf.ps1) is the twin: it drives
the grid with real injected touch (`touch.ps1`) and reads **PresentMon**'s per-present CSV, then
reports the gaps between presents *during motion*, the same number `framestats` gives, and for the
same reason it is the only honest one. Same refusal to run unless the grid is on screen, same
never-log-content discipline.

But Windows forces a **design** decision the measurement depends on, and it is worth stating loudly
because it is invisible until you hit it:

- **A WinUI XAML surface (a Win2D `CanvasControl` included) owns no swapchain.** It renders into a
  DirectComposition surface, DWM composes it, and *the app presents nothing of its own*. Measured
  elevated on an ARM64 Surface: PresentMon captured 451 presents across `dwm.exe` and
  `WindowsTerminal.exe` (which *does* own a swapchain) and **exactly zero** for ours, through
  sustained motion. **A grid whose frames cannot be counted cannot be held to a budget**, so the
  grid hosts a `CanvasSwapChainPanel` and presents into a swapchain of its own. With that, it shows
  up as `Composed: Flip` and its present timestamps are real (0 → 315 presents on the same test). The
  draw code did not change; only who owns the buffer did.
- **`CompositionTarget.Rendering` is the Windows `gfxinfo`.** Timing it from inside the app is the
  tempting shortcut and it is a lie: it fires when the **UI thread** ticked, not when a frame was
  **presented**, and the compositor animates smoothly straight through a UI-thread stall. Do not use
  it as evidence of what the eye saw.
- **PresentMon needs elevation** (an ETW session), so the perf script raises a UAC prompt, or you
  join `Performance Log Users` once. This is Windows's version of the tax Android pays with a
  physical device: neither number can be had from CI.
- **PresentMon is a required tool for this measurement, not an optional extra**: it is the only
  instrument that reports true present timing (the rest lie, above). Install the Intel PresentMon
  system package; its CLI lands at
  `…\Intel\PresentMon\PresentMonApplication\PresentMon.exe`, which `calendar-perf.ps1`
  **auto-discovers**, so a bare `./calendar-perf.ps1` just works, and a missing install fails fast
  with the install hint rather than deep in the run. `-PresentMon <path>` overrides the discovered
  location (e.g. a standalone build); `PresentMonUI.exe` is the GUI: the script needs the CLI.

**First measured baseline** (release build, an ARM64 Surface's **60 Hz** panel, driven through
page-turns, hour-scrolls and a diagonal pinch), on two datasets:

| in-motion frames | showcase (14 events) | a real diary (~9,800 events, ~1,125 in window) |
|---|---|---|
| **dropped** (gap > 25 ms) | 1.2% | **2.3%** |
| median gap | 16.6 ms (vsync) | **16.6 ms (vsync)** |
| p90 gap | 18 ms | 18.7 ms |
| p99 gap | 26.1 ms | **51.2 ms** |

The steady state is the same on both: median at vsync, p90 ~19 ms, 98% of frames on budget. **The
whole cost of a full diary lands in the p99, and it is one specific, understood thing.** The nine
dropped frames over sixteen seconds are *isolated* ~50 ms spikes, each sitting between normal 15–19 ms
frames, not a run of jank. They are **page builds inside the swipe**: reaching a not-yet-seen week
builds its paint (every event's colour parsed, clock formatted, spoken label assembled) on the UI
thread, and on a busy week that is ~50 ms = three frames. That is exactly the §7 rule "compose the
neighbouring pages, don't build them inside the fling", not yet honoured on Windows: the renderer
itself is clean (the steady draw doesn't move between 14 events and 1,125), so this one hitch is the
only thing left to chase. It is the same p99 tail Android documents. See "Known gaps".

**Read that against the right bar, not the flattering one.** Android's §7 numbers (Samsung's 2.4%
included) were taken on a **120 Hz** phone, where the budget is half (8.33 ms) and the same
smoothness is twice as hard to hold. **1.2% at 60 Hz is not a like-for-like win over 2.4% at 120 Hz.**
What it says honestly: on this hardware the grid is not dropping frames a hand would feel. What it
does *not* yet say: how it holds up on a 120 Hz Windows display, or after the fling/settle constants
(ported from Android, not re-measured here) are tuned to this GPU. Those are open, and named in
"Known gaps".

### The trace

`adb shell setprop log.tag.MailcalCal DEBUG`, then relaunch. Off in every build unless asked for
(a log tag, not a debug flag, so it works in the only build worth measuring), and it costs one cached
boolean. It reports counts and durations, and **never** a title, a time or an attendee: this runs
against real diaries, and `docs/logging.md`'s never-log-content rule does not bend for convenience.

It logs what the single owner *decided each finger was*: `pan_x` / `pan_y` / `zoom` / `drag` /
`tap`, which is how you catch a pinch being misread as a pan by a real hand, and it is what proved
the zoom no longer pans the grid. It found the shaper bug too: a pinch frame costing 3.4× a swipe
frame while drawing *half* the blocks is not a performance problem, it is a clue.

---

## 10. Dragging: a delta, not a destination

Drawing a slot out on empty grid, and moving or resizing an event by dragging it, are one feature
with one rule underneath them. **What crosses the FFI is how far the hand moved**: a signed count of
whole days and minutes, and the core applies it to the event's **own** wall clock
(`Intent::MoveEvent` → `mailcal_account::apply_event_drag`).

The obvious design is the other one: send where it was dropped. It is wrong three times over, and
each way is invisible until it bites.

1. **The client draws in the display zone; the event lives in its own.** A meeting in
   `Europe/Amsterdam` read on a device set to `America/New_York` is drawn six hours earlier, so the
   clock it was dropped under is *not* the clock it must be written with. A client that sent the drop
   position would move a colleague's meeting to the wrong hour for everyone else: the exact failure
   `EventEdit`'s wall-clock rule exists to prevent, arrived at from a new direction.
2. **A dragged block is not always the whole event.** The grid splits an event crossing midnight into
   one segment per day and clips each to its column (§1), so a segment's `start_minutes` is `0` on
   every day but the first. There is no absolute start on screen to send.
3. **A destination cannot preserve a duration.** Rounding a drop to the grid and re-deriving the end
   silently re-times the event; a delta moves both edges by the same number and the duration comes
   out bit-identical.

A delta has none of those problems, because it is the same number in either zone and on any segment.
It is also **wall-clock** arithmetic, not elapsed time: an event dragged across a spring-forward
boundary stays at 10:00 rather than landing at 11:00, which is what the grid showed and what the user
meant.

### Only your own events lift

`TimedSegment` carries **`can_move`**, and it is strictly narrower than `can_write`: a writable
calendar *and* an event that is the user's own: an appointment nobody was invited to, or a meeting
this account **organises** (`mailcal_app::invitations::owns_or_organizes`).

Everything else is somebody else's. A meeting we were invited to, a room booking, a colleague's event
on a shared calendar: we may well have write access, and re-timing it behind the organiser's back is
still not a move. The right affordance there is **propose a new time** (iTIP `COUNTER`, a feature of
its own) so until that exists the block simply does not lift, and a press on one draws out a *new*
slot exactly as a press on bare grid does. Doing nothing at all reads as a missed gesture.

**The core checks this again on the write.** A client gating the gesture is the right thing for the
user and is not the check: the intent crosses an FFI, and a write that trusts its caller is not a
check at all.

Note what `can_move` is **not**: it is not `participation`'s question. That one collapses "nobody was
invited" and "invited, but not us" into the same `Accepted`, because for *drawing* they are the same
thing: both are commitments, neither is an unanswered hold (§4). For *writing* they could not be
more different, and one function must not answer both.

### A repeating event is asked about, never guessed

**Every surface that draws one occurrence names it.** `TimedSegment`, `AllDayBand` and `MonthChip`
each carry **`occurrence_start`**: that occurrence's own start, as a wall clock in the event's own
zone, and **empty when the event does not recur**. It is opaque: a client hands it back verbatim as
`Intent::MoveEvent`'s `occurrence`, and non-empty is also the signal that a write must **ask**
first.

`EventRow` is the exception, and deliberately: the agenda holds one row per *event*, not per
occurrence, so a row has no single day to name and a write from it is a series write. A client says
so rather than asking a question it could not honour.

Dragging one Tuesday standup is not the same as rewriting every Tuesday to eternity, and only the
user knows which they meant, so every client shows *This event · All events*, and cancelling writes
nothing. `EventEdit::occurrence` has no default for the same reason; the engine splits a
`RECURRENCE-ID` override out of the series when one is named, and patches the series when none is.

The question is put to the user on **every** write that can be either: a drag, an edit and a delete
all carry an `occurrence`, all three read it off the surface the user opened, and all three mean the
same two things by it.

**It is the only question that write asks.** Where a client already confirms a destructive act, the
scope question **replaces** that confirmation rather than following it: *This event · All events*
carries its own way out, and a delete that raises two dialogs teaches the user to dismiss both. So a
client's generic "Delete this event?" is for the writes that name no occurrence: a one-off event,
and the agenda row that stands for the whole series.

**And nothing on the way to it answers it.** An editor that will ask at Save may not also carry
"Changes apply to the whole series.": that sentence states one of the two answers, so on an
editor opened on one occurrence it tells the user something the next dialog contradicts. It is
shown exactly where it is true and nothing will be asked: an editor opened on the **series**.
Same fact on both sides, so they can only disagree if one of them forgets to read it.

**The token travels with the reference the user clicked, and into the read.**
`MailcalApp::event_detail` takes it, and what comes back describes **that occurrence**: its own
start and end, as the expander produced them. Without it the detail is the series', whose start is
its **first** occurrence's: a September standup opened on any later day reads as August's, and an
editor prefilled from it writes that date back.

**A client asks its scope question from `EventDetail::occurrence_start`, never from the token it
sent.** The two differ exactly when the core could not resolve what was sent: a token goes stale
when the series changes underneath the view it was drawn in, and then the field comes back empty
and the times above it are the series' again. Reading the echo is what makes it impossible to offer
*This event* against another occurrence's times.

**And the answer is checked before anything is sent.** A token names an occurrence only if the core
would mint that token for a block it drew: asked against the store the grid was drawn from, and
answered by re-minting rather than by parsing, so the check cannot drift from the emitter. A token
that names nothing is a **refusal**, never a widening: a delete that could not find the occurrence
and removed the series instead is the one outcome nobody can undo. A one-off event is named by no
token at all, so any token on one is refused too: the write that would otherwise put a
`RECURRENCE-ID` into a document that has no rule.

What this catches is a wrong token rather than a hostile client: one the core minted from the wrong
field (it did, once, the block's start instead of its recurrence id), one a client built itself
instead of handing ours back, or one that was valid until the series changed underneath it.

### A repeat rule is structure, and what we cannot model is read-only

`EventDetail::recurrence` carries the rule **structurally**: frequency, interval, the weekdays,
month days and months it names, and how it ends, not a frequency word. A word cannot tell "every
week" from "every second week", so a summary built from one called both *Weekly*. The sentence
itself stays client-side, like every other piece of localised text.

Two values, and the second is the one with teeth:

- **`Simple`**: a rule the core can state in full. A client may seed a repeat editor from it.
- **`Complex`**: it repeats on a rule richer than that shape holds: several rules, an exclusion
  rule, `bySetPosition`, ISO week numbers, a non-Gregorian `rscale`, or a repeat measured in hours
  or finer. **Say that it repeats; offer no edit.**

Which one an event gets is not decided by a list of the parts we understand. The core builds the
projection, **rebuilds the engine rule from it, and compares**: anything that does not come back
identical is `Complex`. A hand-maintained list would be wrong the day the engine gains a field: a
rule carrying it would look simple, an editor would seed from a projection missing it, and the save
would write the rule back without it, silently, on a real series. The round trip degrades such a
rule to read-only instead.

An override is **not** part of the rule. A series stays editable after somebody moves or cancels a
single occurrence of it.

A write carries the same shape back. `CreateEvent` takes the rule; an edit takes one of three
answers: say nothing and the series is untouched, `Set` replaces the rule, `Clear` makes the event
a single one. `DeleteEvent` takes an `occurrence`, exactly as an edit and a drag do.

Three writes the core refuses, whatever the client sends:

- **A rule over a `Complex` one.** The client only ever saw "it repeats", so whatever its editor
  holds is missing the parts that made the stored rule complex, and the save would drop them. A
  client is gated on the same answer; the core checks it again, because a write must not trust its
  caller. **Clearing** is allowed: stopping a repeat needs no knowledge of the rule it stops.
- **A rule paired with an `occurrence`.** A rule belongs to the series; one occurrence is an
  instance *of* a rule, not a holder of one.
- **A rule that describes no series**: an interval of zero, a count of zero, an end date that is
  not a wall clock.

A rule ending on a date ends at a **wall clock in the event's own zone**, and iCalendar requires
that bound in UTC once the event is zoned (RFC 5545 §3.3.10). The core resolves it, because it is
the only layer holding both the rule and the tzdata, the same reason it resolves the instant that
names an occurrence.

**And a rule that could not be drawn is refused too.** A rule the expander will not take
materializes zero occurrences, so the event is stored, is invisible to every range read, and the
grid draws it nowhere: saved, and gone. A rule that expands but matches nothing after the event's
own start is the same failure with one block on screen: an event that says it repeats and never
does. Both are worse than a write that fails, so both fail instead. What is refused:

| Refused | Because |
|---|---|
| a weekday's position on a daily or weekly rule | a week holds one Monday, so "the fourth" names nothing |
| a weekday's position on a yearly rule that names no month | positions are counted per month, so the rule has to say which |
| a position past the fifth | no month holds a sixth Monday |
| a month outside 1–12, a day outside ±1–31 | not a month, not a day |
| an interval past what a calendar can draw | it repeats once, and nothing that repeats once is a repeat |

The list is **measured against the engine that ships**, not copied from it, and a test re-measures
every entry: if the engine grows to cover one, that test goes red and this table loses a row.

### A rule is editable exactly when it can be stated, and four controls is not a rule

An editor puts four controls on screen: how often, how many periods to skip, which weekdays, and
what ends it. `SimpleRecurrence` says more than that: a monthly series pinned to the month's
**last day**, or to a weekday's **position** in it, is a rule no control there offers.

So `EventDetail::repeat_draft` carries what the controls hold, and two rules decide the rest:

- **A rule the core could not state is never offered for editing.** The draft is absent for
  exactly the rules `repeat_summary` is absent for: a client that cannot say what a rule is must
  not offer to change it, which is the judgement `Simple` against `Complex` makes one layer up,
  made again about the form beside the sentence.
- **What the controls do not model, they keep.** The draft carries the rule it was read from, and
  the parts no control here holds are put back on save, for exactly as long as the **frequency is
  still the one they were read under**. Change monthly to weekly and they go, which is right: a day
  of the month means nothing in a week.

Rebuilding from the four controls alone would drop that part and write a different series: "the
last day of the month" quietly becoming "the 31st", which skips every short month. So a client does
not rebuild the rule at all: it hands the draft back and the core answers with one of
[the three](#a-repeat-rule-is-structure-and-what-we-cannot-model-is-read-only), or with nothing,
which is the fourth answer and the one a save that never touched the repeat gives.

**A changed repeat settles which occurrences a save meant.** A rule belongs to the series, so an
editor opened on one occurrence does not ask *This event · All events* once the rule has moved;
it says so under the controls instead, before the user touches them. The core refuses a rule paired
with an `occurrence` in any case; no client builds that payload, and the refusal is the second
place it cannot happen.

### Which sentence a repeat rule gets is decided once; the words are each client's

A frequency word cannot tell "every week" from "every second week", so a summary needs the rule's
parts. But **deciding which sentence a rule gets is not localisation, and it does not belong in a
client.** `EventDetail::repeat_summary` carries the decision already made:

- an empty `days` / `month_days` / `months` list is **not a gap to fill in**: it means the rule
  takes that part from the event's own start, and the core has already read it there, so a weekly
  rule that names no weekday still arrives naming one;
- the weekdays arrive **in week order**, not in the order the server sent them;
- a bound arrives as a plain date, or the summary does not arrive at all;
- **a rule that cannot be stated exactly is `None`**, and a client says only that the event
  repeats. Two day-of-month values, a weekday position past the fifth, an unreadable end date:
  describing any of them approximately states a series the user does not have, and nothing on
  screen would tell them apart. It is the judgement `Simple` against `Complex` makes, one layer up.

Four clients each deriving that from this document is four sets of disagreements, and only the one
a reader happens to be looking at is visible. What stays client-side is the **wording**: the frames
come from each platform's catalog, and the weekday and month names from its own locale data, which
is the one part of a localised string nobody has to translate.

**"The fourth Monday" carries its own article.** Italian inflects the ordinal for *domenica* and
Portuguese for *segunda* through *sexta*, so the frame is "Monthly on {position}" rather than
"Monthly on the {position}", each position has two wordings, and the set of weekdays taking the
second one is stated **per language, in the catalog** (ISO weekday numbers, empty where the
question does not arise). The alternative is a table of genders in client code: a language's
grammar in a place no translator can reach, that the next language needs an engineer to extend.

### Editing a series can throw away what the user did to one occurrence

Every transport folds a per-occurrence change into the same overrides map, so the user sees one
idea: *this Tuesday is different*. What a later **series** edit does to that difference is four
different server policies, measured:

| A series edit… | CalDAV | JMAP | Graph | Google |
|---|---|---|---|---|
| keeps occurrences the user **moved**, when the series' time moves | ✅ | ✅ | ❌ | ❌ |
| keeps them when the **repeat rule** changes | ✅ | ✅ | ❌ | ✅ |
| leaves a **name** the user gave one occurrence alone | ✅ | ✅ | ✅ | ❌ |

Two of the four discard the user's work with nothing on screen to say so, and the only moment
anything can be done about it is the edit itself. So a client asks
`MailcalApp::series_edit_warning` with the payload it is about to dispatch, and shows what comes
back between Save and the write, **and nothing to say is the common case**, because the core ANDs
three facts:

- what the **account's server** does (`Capabilities::override_survival`),
- whether **this series** actually holds a per-occurrence change, and
- whether **this edit** does the thing that would lose it.

A clean series is never warned about. That is not a nicety: a dialog that appears on every
repeating event is what teaches people to click past the one that mattered. Cancelling a single
occurrence counts as the user's work like any other change to it.

**The third fact is what keeps the sentence true rather than merely rare.** Each
`OverrideSurvival` flag describes the consequence of a *particular* kind of edit, so a flag is owed
only by an edit that does that thing: on a server that folds a moved occurrence back when the
series moves and leaves an override's own fields alone, a retitle costs nothing, and saying
otherwise spends the user's attention on a loss that will not happen. It is asked in the core, not
in each client, for the reason the sentence's *choice* is: four clients deriving three booleans
from a form is four chances to get one wrong, on a dialog no harness can raise. The core compares
the payload against what is **stored**, so a field the user typed and typed back is not a change.

**A save scoped to one occurrence is never warned about, and must not ask.** It writes an override
of its own and costs no other occurrence anything, so the scope question comes first, and only its
*All events* answer can raise a warning.

It is **one closed enum, not the three facts**, for the reason the repeat summary is not a
frequency word: a client turning booleans into a sentence is four clients disagreeing. The core
decides which warning applies; each client writes the sentence from its own catalog. **No client
learns a provider's name**: "Outlook does this" is not a thing to tell somebody about their own
calendar, and it stops being true the moment a fifth transport arrives.

### What a client decides, and what it may not

| The client decides | Because |
|---|---|
| Which gesture begins a drag | It is the platform's input, not the calendar's: see below |
| Where the resize grab zones are, and when they apply at all | Points, and a function of the zoom |
| That the delta snaps to **15 minutes** | Shared by rule, held in each client (`DRAG_SNAP_MINUTES` / `dragSnapMinutes`) |
| That a **create** is the union of an hour band and the finger | Shared by rule, same place: see below |
| Whether the preview stays inside its day | A picture, and it must not lie about the write |
| That the block may be drawn **between** snap steps, while the readout is not | Shared by rule, same place: see below |

**The gesture is genuinely per-platform, and the difference is the input rather than a preference.**
On a **desktop** a click-drag creates and moves, and there is no drag-to-pan, because a desktop
*scrolls* a calendar: the wheel and the trackpad already move both axes (§12 note ⁷). On a **phone or
tablet** a plain drag is the only way to pan, so it stays a pan and a **long press** is what takes
hold of an event.

Three rules bind whatever the gesture is:

- **What you see is what you get.** The preview is clamped to the day it is drawn in, so an event
  dragged to the top of the screen stops at 00:00 rather than silently landing on the previous day.
  To change the day you drag *sideways*: the one thing the preview can actually show.
- **A press that went nowhere writes nothing.** A zero-delta patch spends a network round-trip and a
  revision to change nothing.
- **A resize clamps rather than refuses.** Dragging an edge past its opposite stops at a quarter of an
  hour; a block that will not shrink, with nothing on screen to say why, is worse than one that stops.

### A create is a union, so it cannot jump

**The slot a create draws is the union of the hour the press landed in and where the finger is now.**
Stay inside that band and it is the band: "an event here", a clean hour on the hour. Drag below it
and the top stays on that hour while the bottom follows; drag above it and the bottom stays on the
following hour while the top follows.

Two properties fall out, and both are the point:

- **The band contains the touch.** It is the hour the finger is *inside*, floored, measured from the
  **unrounded** minute. Rounding to the nearest boundary instead sends a press at 17:50 to
  18:00–19:00, drawing the whole block below the finger that asked for it; and pinning from the
  *snapped* minute lands 16:53 in the band below the one under the finger, because 16:53 snaps
  forward to 17:00.
- **A union is continuous**, so there is no threshold at which the slot changes shape and therefore
  nothing that can jump at one. An anchored span has to choose an anchor, and choosing the touch
  point makes the block leap off the hour it was showing the moment the finger moves. Judging
  "press or drawn length?" per frame is the same defect from the other side: dragging back across the
  anchor re-enters the press threshold, and the slot teleports to a pinned hour for a single frame,
  once per crossing.

The cost, stated rather than discovered: **a drawn slot is never shorter than an hour.** Shorter is
the editor's job, and the drag already opens it prefilled.

### The block may glide; the readout may not

The state carries the finger's minute twice: **snapped**, which is what `Intent::MoveEvent` sends
and what the readout says, and **raw**, which is what the block is drawn from. They differ only
mid-gesture, never by a whole snap step, and only on the edge actually in the hand: the anchored edge
is drawn exactly where the write will put it.

This is the one place the picture is allowed to differ from the write, and it is why the live readout
is not optional. The pill quotes the **snapped** time, so nothing on screen claims a minute the drop
will not honour.

### A drag is a picture, so it says the time out loud

A quarter-hour snap is *invisible* on a zoomed-out grid: the block moves three pixels and the user has
no way to know whether that was 15 minutes or 30. So the drag carries the time it would land on, live,
for as long as the gesture runs. This is the one string a frame is allowed to format (§7 forbids
per-*block* derivation, not one label).

It rides in a **floating pill beside the block**, not written inside it. Inside, it was dropped
exactly when it was needed most: a fifteen-minute slot at a zoomed-out horizon is a few pixels tall,
and the label that tells a 15-minute snap from a 30-minute one did not fit in it. The pill is clamped
to the visible grid, so a slot drawn against an edge cannot push its own readout off screen.

**A new event opens with the caret in its title.** The same rule the composer's To field follows
([`contacts.md`](contacts.md) §4), and for the same reason: the caret goes where the work starts, and
the title is the one field a new event cannot be saved without. **Editing** an existing one does not
take it: the event already has a title, and raising a keyboard over the form hides the dates, which
are usually what the user opened it to change.

Three traps, all silent. Two are shared with the composer: a toolkit drops a focus request made
before the sheet has finished presenting or the widget has been mapped, leaving a field that only
*looks* ready; and nothing a screenshot can see is different either way, so the assertion has to be
on focus itself (`EventEditorFocusTest` on Android, `EventEditorFocus.Tests.ps1` on Windows; an
AT-SPI run on Linux).

The third belongs to the **edit** half, and it inverts the first: where a dialog places the caret
itself, withholding the request is not enough. A WinUI `ContentDialog` focuses the **first**
focusable control in its content, here the title, and it does not do so only once: it returns the
caret there at the next thing that touches the form, so focusing something else as the dialog opens
is undone a moment later, whatever the target and whenever the call is made. What works is to *be*
the control it chooses: the form's own scroller, made a tab stop while editing, sits ahead of the
title, so the dialog's choice is a container rather than a text field. Arrow keys then scroll the
form and the title is one Tab away.

The refusals matter as much as the fix, because each looks correct: focusing the Save button (or
the start date, or anything else) as the dialog opens leaves the caret right for a moment and wrong
thereafter, and it takes the **scroll** with it: the returning focus change cancels a programmatic
scroll outright, which puts the attendee roster below the fold out of reach and reads as an
unrelated bug in the editor's scrolling.

**Non-visual parity is the editor, and that is deliberate.** A drag cannot be performed by a screen
reader, and no amount of labelling changes that, so the capability it offers must exist somewhere
else, and it does: the same time is editable in the event editor, on every platform, reachable by tap
(§12). A drag is a shortcut, never the only door.

---

## 11. The bar a new platform has to clear

Apple's grid predates everything §6, §7 and §9 now say, and its strip (footnote ⁴) is the first piece
of it retro-fitted: `CalendarStrip` is a plain Swift value type with no SwiftUI in it, so where the
grid rests, which weeks it is showing and what a pan does to it are all pinned by `CalendarStripTests`
without a viewport. That is the shape the rest of it owes. Windows was built *against* these rules from
the start, and is the proof they port: **the entire navigation model** (state machine, zoom, paging, gesture owner,
animation driver) **is framework-free C# with an injected clock**, and every rule above is a headless
test. What is left on WinUI's side is a translation that holds no decisions (§9). That is not a Windows
trick. It is what these rules look like when a platform adopts them deliberately instead of discovering
them, and it is why the strip rewrite (§6) could change the whole horizontal axis and still be *proved*
before the app was ever launched.

**The rules in this file are the contract, not Android trivia**: a client may not ship the grid until
it meets them, and "it feels alright" is not evidence, because every bug in this document felt alright.

Concretely, a platform building the grid owes:

- **One owner for the pointer stream** (§6). Not a pager plus a scroller plus a pinch recogniser, each
  reading the same finger and none of them able to see the others. That arrangement is not tunable:
  it produced both the swipe that stuck between two weeks *and* the zoom that panned the grid, and
  neither was fixable from inside it.
- **A turn decided before it is drawn** (§6), or fast flicks eat each other. On a real hand, eight
  flicks turned three weeks and every unit test was green.
- **A gesture judged on its own finger**, never on where the page is sitting (§6).
- **The frame budget measured, on a real device, on a release build** (§7): with the gaps between
  frames *during motion*, not a jank ratio and not a video. Both of those instruments returned
  confident, wrong answers here.
- **A test that delivers a gesture while the previous animation is still running** (§9). Every test
  that waits for the grid to settle first is testing the case that already worked.
- **The spoken grid** (§4, §7). A drawn grid has no accessibility tree; that is a bill, not an excuse.

The Linux client implements the grid. Its GTK
surface paints the day/3-day/work-week/week grid with Cairo from one immutable scene:
the visible rectangles and the semantic AT-SPI event buttons are both placed from that scene's exact
core-provided day/minute/column geometry. It also has header-driven `< Today >` navigation, the
agenda and month surfaces, the all-day and month overflow rules, explicit loading for every
`is_materialized: false` page, and detail/create/edit/delete dialogs. The deterministic Stalwart
acceptance path drives create → detail → edit → delete through AT-SPI without coordinates.

The first Flatpak release scopes
navigation to the mode menu and `< Today >` header cluster, vertical scrolling to GTK's native
scroll view, and slot creation to a primary-button drag on empty space. Horizontal swipe/trackpad
navigation, pinch, rapid flicks and the continuous strip remain explicit follow-ups; their matrix
cells stay ⬜ rather than being implied by the grid. The sighted path materializes no semantic event
buttons, while enabling toolkit accessibility rebuilds the exact-geometry AT-SPI overlay at runtime.
The scoped release path is frame-qualified on the packaged runtime (note ¹²).

---

## 12. Per-platform matrix

| | Shared core | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|:---:|
| Time grid: day / 3-day / work-week / week | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Now line, initial vertical framing, week numbers, jump-to-today | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| All-day banner + per-day "+N" + expand | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Month grid (6×7, "+N more") | ✅ | ✅ | ✅ | ✅ ³ | ✅ | ✅ |
| Agenda list | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Unanswered holds drawn as holds: grid, all-day banner, month, agenda ⁸ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| "Awaiting your response" in the spoken label ⁸ | ✅ (string) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Declined events hidden from grid / month / agenda ⁸ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Calendar manager: visibility + colour override (persisted) | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Display settings: week start, 12/24h, horizon | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Pinch-to-zoom: hours | — | ✅ | ✅ | ✅ | ✅ | ⬜ |
| Pinch-to-zoom: days, and **diagonal** | — | ✅ | ✅ | ✅ ¹ | ✅ | ⬜ |
| Shape + horizon restored on launch | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Stored diary on screen at launch, and filled without being opened | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Week paging: by swipe | — | ⬜ ⁴ | ⬜ ⁴ | ⬜ ⁴ | ✅ | ⬜ |
| Continuous day strip + **pinned** hour ruler (rests between weeks) | — | ✅ ⁴ | ✅ ⁴ | ✅ ⁴ | ⬜ | ⬜ |
| Comes to rest on a **day**: every zoom, every input | — | ✅ ⁴ | ✅ ⁴ | ✅ ⁴ | ⬜ ⁴ | ⬜ ⁴ |
| Free horizontal scroll **across** weeks: trackpad / mouse wheel | — | ✅ ⁴ | ✅ ¹⁷ | ✅ ⁴ | — | ⬜ |
| A wheel notch asks for **travel**, not a jump (eased, any cadence) | — | ✅ ⁴ | — | ✅ ⁴ | — | ⬜ |
| Free vertical scroll: hours, by wheel, trackpad or finger ⁷ | — | ✅ | ✅ | ✅ | ✅ | ✅ |
| Horizontal **touch** pan across weeks, landing on a day | — | — | ✅ ⁴ | ✅ | ⬜ | ⬜ |
| `< Today >` header navigation (steps by the visible span ⁵) | — | ⬜ | ⬜ | ✅ | ⬜ | ✅ |
| Drawn (canvas) grid, not composed | — | ✅ | ✅ | ✅ | ✅ | ✅ |
| One owner for the pointer stream | — | ⬜ | ⬜ | ✅ | ✅ | ⬜ |
| The spoken **time** grid (a11y over a canvas): the month surface is its own question, see "Known gaps" | — | ✅ | ✅ | ✅ | ✅ | ✅ |
| Frame budget measured on a release build | — | ⬜ | ⬜ | ✅ ² | ✅ | ✅ ¹² |
| Create / delete event | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Create with a **calendar picker** (grouped by account) · all-day · notes · location ⁶ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **View** an event (tap → detail) and **edit** it: title, time, notes, location ⁶ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| An event's **attendee list**: organiser first, each with their answer ⁹ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| The same list, **read-only, in the editor** ⁹ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| A **new** event opens with the caret in its title ¹¹ | — | ✅ | ✅ | ✅ | ✅ | ✅ |
| Write affordances gated on `can_write`: a read-only row offers no delete; "New event" disabled without a writable calendar | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Drag-to-create**: draw a slot out on empty grid, opening the editor prefilled ¹⁰ | ✅ | ✅ | ✅ | ⬜ | ✅ | ✅ |
| **Drag to move · resize** an event, gated on `can_move` (your own appointments and the meetings you organise) ¹⁰ | ✅ | ✅ | ✅ | ⬜ | ✅ | ⬜ |
| A drag on a **repeating** event asks *This event · All events* ¹⁰ | ✅ | ✅ | ✅ | ⬜ | ✅ | ⬜ |
| A repeat rule **summarised in full**: every second Tuesday, until 3 June, rather than one word ¹³ | ✅ (structure) | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Writing** a repeat rule: set one, change one, stop the repeat ¹³ ¹⁶ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| A **delete** on a repeating event asks *This event · All events* ¹³ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| An **edit** on a repeating event asks *This event · All events* ¹³ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| A detail opened on one occurrence reports **that occurrence's** times, not the series' ¹⁴ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| A **warning** before a series edit throws away what the user did to one occurrence ¹³ ¹⁵ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| Drag an **all-day bar** in the banner | — | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |
| **Propose a new time** for somebody else's meeting (iTIP `COUNTER`) | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ | ⬜ |

¹ **Touchscreen only.** A precision touchpad's pinch reaches the app as a scalar `Ctrl`+wheel, so it
zooms the hours and nothing else: see §6 and "Known gaps".
² Measured: 1.2% of in-motion frames dropped, on a 60 Hz panel (§7). The *instrument* is settled
(the grid owns a swapchain, so PresentMon sees it); the *tuning* against 120 Hz and this GPU is not:
see "Known gaps".
³ **Composed (XAML), not drawn**: like Android's month grid, and deliberately so: the "drawn, not
composed" rule (§7) is a *time-grid* concern, a property of the pinch and the fling, which the month
has neither of. It is paged by header chevrons rather than a swipe (a month is not a week-stride),
and a tapped day drops into the day zoom.
⁴ **The strip, and where it rests.** See §6, "The days are one strip". Windows, macOS and
iOS/iPadOS all draw one continuous strip that comes to rest on a **day**, at every zoom and for every
input: wheel, trackpad and finger alike. There is no page turn: a flick coasts and lands on the day it
stops nearest.

**Android deliberately keeps week paging**, and is not wrong to: nothing on a touchscreen forces the
question, and paging matches what Samsung Calendar does on the same hardware. **Linux is pending**,
and is further back than the others were, having no horizontal gesture surface at all: see "Known
gaps". A platform adopting the strip takes the whole rule, not half of it.

**What Apple has that Windows does not is phase.** Windows hands a precision touchpad's pan to the
app as wheel messages with no begin, end or inertia flag, so it infers the end from silence and pays
a quarter-second for the guess. `NSEvent` says when a gesture began, when the fingers lifted and when
its momentum ran out, and `UIPanGestureRecognizer` says the same, so the Apple strip lands because the
user let go. Only a legacy **mouse wheel** reports no phase there either, and only that path keeps an
idle window (`CalendarScrollGesture.swift`).

**Apple's landing is one animation, not a driver.** `CalendarGridView` is `Animatable` on the strip
position, so SwiftUI re-evaluates its body per frame with the interpolated value and the weeks the
grid *pulls and draws* are the ones it is sliding through. Interpolating only the offsets would draw a
hole where the week arriving from the right should be, which is the same failure Windows' lag cap and
five live pages were guarding against, reached from the other end.
⁵ **Except the work week, which steps a whole week rather than its five columns**: a five-day step
would land the next click on Saturday-to-Wednesday, and "work week" means Monday to Friday or it means
nothing. The day zoom steps a day, the 3-day steps three, the week a week, the month a month.
⁶ **Shipped on core + macOS + iOS/iPadOS + Windows + Android + Linux.**
The FFI carries the whole
editor/detail surface: create with a target `calendar` + `all_day` + `notes` + `location`, an
`event_detail` read that projects a stored event (its own wall clock, calendar, location, notes,
reminder/recurrence summaries), and `Intent::UpdateEvent` (title/time/notes/location, in the event's
own wall clock).
Every shipped client has the detail sheet, the shared create/edit editor, and the per-account
calendar picker, matching Samsung Calendar's flow; Linux implements the same provider-neutral
intents behind its 🚧 surface, and its picker is a **flat** `gtk::DropDown` whose every entry names
its account (`Account · Calendar`) rather than a list in per-account sections.
**Windows is built and UIA-verified on a Windows host**:
`EventEditorDialog`, `EventDetailDialog`, the grid/agenda/month tap routing, plus the pure
`EventEditorState` + `CalendarHitTest` (unit-tested, `dotnet test` green); the
create-with-picker → detail → edit → delete flow was driven end-to-end against the Stalwart harness
(real CalDAV, each write reconciled), including a real touch tap on the drawn grid. Two engine-bounded
v1 limits, on every platform: **all-day and the calendar are set at create and are display-only on
edit** (the patcher refuses a form or calendar change), and **reminders are display-only**
(read-only: see "Known gaps"). The repeat rule is editable; what its controls do not reach is
under "Known gaps" too.

⁷ **A desktop scrolls a calendar; it does not drag it.** The grid holds its own scroll offsets
rather than living in a scroll view (§7: a pinch has to *move* them mid-gesture, which no scroll view
allows), and the cost of that is the scrolling a scroll view would have given for free. Every client
puts it back by hand. On macOS a two-finger trackpad scroll and a mouse wheel both move the hours, and
`Shift`+wheel reaches the day axis for a plain mouse with no horizontal axis to report; on iOS/iPadOS
a finger does it. The day axis is no longer this row's business on macOS, iOS or Windows: it belongs
to the strip (note ⁴), which has no end to clamp against.

**Linux's ✅ is its scroll view's.** The surface's root is a `gtk::ScrolledWindow`, so a wheel or
two-finger scroll moves the hours with no code of ours, and `set_hscrollbar_policy(Never)` is what
pins the days.

⁸ **The three participation rows, and why only one of them is ✅ everywhere.** §4 owns the semantics;
the full contract is [`invitations.md`](invitations.md). *Declined-hiding* is applied in the core's
occurrence cache, at the single point where occurrences meet their masters, so a client inherits it
with no client code and cannot opt out; it was never shipped per platform. The other two are
**drawing**, and drawing is the client's half of §1: the core stamps `participation` on every
occurrence, segment, band, month chip and agenda row, and each client decides what a hold looks like.
**Linux** draws all four from one set of constants in `calendar/paint.rs`, across two renderers:
Cairo for the grid block, the all-day band and the invitation card's own preview; GTK CSS for the
month chips, because a `DrawingArea` per chip would cost more than a hatch is worth. The agenda row
has no border to dash, so it prints the hold in its subtitle instead. Since one Swift package draws
both Apple platforms, macOS and iOS/iPadOS necessarily moved together.

Windows was the same gap until `CalendarHold.cs`, and how it closed is the part worth keeping: its
**invitation card's meeting-day preview** already drew a hold dashed, because that surface arrived
with the card, but the preview is a *composed* one-day picture while the grid and the banner are
the **Win2D canvas** whose paint is built per page. Two renderers, so the hatch is written twice
over one set of constants rather than delegated the way Android delegates its composed month chip
back to its canvas (a `CanvasControl` per chip would cost more than a hatch is worth). The fade and
the spoken suffix are in the WinUI-free `InvitationFormat`, so both are unit-tested rather than
trusted to a screenshot. Details in [`invitations.md`](invitations.md).

Android draws the same three pictures by a different mechanism, and the difference is worth knowing
before changing either: its grid is a **canvas** (§7, nothing may be derived inside a frame), so
whether a record is a hold is resolved once when the page is built and travels as a boolean on
`BlockPaint`/`BandPaint`, while the Apple client asks the record on each redraw. Both read the same
`participation`; only the moment differs.

⁹ **The attendee list.** The projection is entirely in the core (§4, "The attendee list") and crosses
the FFI as one repeated field, so a client only draws rows, which is why all four shipped platforms
moved together, and why Linux inherits it behind its 🚧 surface. What is *not* in the core is what
the two lines under a name say; that rule (an unnamed attendee is shown by address, so their address
is not repeated beneath it) is small, entirely about what a user reads, and therefore unit-tested on
each client: `EventAttendeesTests` (Apple), `EventAttendeesTest` (Android), `AttendeeSummaryTests`
(Windows), `attendees_tests.rs` (Linux, which also holds that neither line is parsed as Pango
markup). **Read-only everywhere**, so the editor's copy carries the sentence saying so.

Not shown, deliberately: **required vs optional** (`ROLE=OPT-PARTICIPANT`), and **which row is
you**: see "Known gaps".

¹⁰ **Dragging.** §10 owns the semantics. The **core** half is shared and inherited with no client
code: `can_move` and `occurrence_start` are stamped onto every `TimedSegment`, and `Intent::MoveEvent`
turns a signed day/minute offset into a patch of the event's own wall clock, so no client converts a
zone, and none can move a meeting somebody else called (the core refuses it a second time).

What each client owns is the **gesture**, and it differs by input rather than by taste: **macOS** and
**Linux** use a click-drag, because the wheel and trackpad already scroll the grid (note ⁷);
**iOS/iPadOS** and **Android** use a long press, because a plain drag is their only pan. Linux applies
that gesture to empty grid space only; event blocks keep opening their detail view until move and
resize are implemented.

**macOS and Android were both driven end-to-end against the Stalwart harness**: create, move and
resize, each write reconciled against CalDAV, and on Android the `can_move` gate confirmed on screen:
long-pressing a meeting somebody else organises leaves it exactly where it is and draws a new slot
instead. Android additionally carries the gesture *races* as JVM tests (§9), which is the half no
hand can reach reliably.

One Swift package draws both Apple platforms, so iOS/iPadOS moves with macOS. **iOS was also driven
by hand on the simulator**: a long-press drag on empty grid drew 04:00–06:30 from a press at 04:50,
the hour band pinned, the block in the calendar's own colour, the floating readout tracking the
snapped time. iPadOS still rides on the shared code and is compile-verified only.

⚠️ **A perfectly still synthetic press creates nothing on iOS**, and that is the instrument rather
than the product: the gesture is a long press *sequenced before* a drag, so with no movement at all
the drag never delivers a value and no slot begins. A real finger always jitters. Inject it with
`idb ui swipe --delta 1 --duration <long>`: small enough steps that the first stay inside the long
press's 12pt tolerance for its 0.4s, then keep going, because `idb ui tap --duration` does not.

**Windows ships nothing.** Its precision-touchpad pointer stack is the one the drag was not written
against: a pan arrives as phase-less wheel messages (§6), with no begin or end to own. Linux draws and
settles a create drag; move and resize remain absent: see "Known gaps".

¹¹ **The opening caret.** §11 owns the rule. Held by `EventEditorFocusTest` on Android, both halves,
and it fails when the request is dropped, and driven against the harness on 2026-08-23 on the
**iPhone** simulator and an **Android emulator** (title focused, keyboard up, typing lands in it).
macOS draws the editor from the *same* SwiftUI view as iOS, but was not driven: this build's sidebar
offered no Calendar entry to reach it by.

**Windows** is held by `uitests/EventEditorFocus.Tests.ps1` (both halves, each watched failing
against the rule broken) and was driven against the harness on 2026-08-24. That run is what found
the third trap above: the edit half was *wrong* on Windows and looked right in the source, because
the dialog was placing the caret in the title by itself. Two owed: a pass on macOS, an AT-SPI run
on Linux.

¹² **Linux's first Flatpak release path is measured on the pinned GNOME runtime.**
`scripts/dev/test-linux-calendar-perf.sh` builds optimised with the GNOME 50 SDK, keeps 139 events
in the visible week, scrolls the vertical grid for 600 frames, and reads GDK's completed
presentation timestamps. On GTK 4.22.4 at 3840×2160 and 60 Hz, 611 presentations had a 16.668 ms
median, 16.672 ms p90, 16.675 ms p99, and **zero** gaps over 1.5 refresh intervals. Every positive
presentation gap is counted, including stalls of 60 ms or longer. The test asserts that semantic
nodes are absent on the sighted path; the AT-SPI suite separately runs with them on.

¹³ **The rule crosses the FFI structurally, in both directions.** An edit and a delete carry an
`occurrence` exactly as a drag does, and `MailcalApp::series_edit_warning` says, for the edit in
hand, when a series edit is about to discard a per-occurrence change, a **rule change** among
them, which is the edit two of the four providers answer by discarding every override.

¹⁴ **This is what the edit question was waiting on, and it was a wrong date on screen in its own
right.** A detail projected without the occurrence carries the *series'* own start and end, which
are its **first** occurrence's, so every recurring event opened from the grid on any later day
read as its first, with no editing involved; an editor seeded from it would then have written that
date back, and answering *This event* against it would have moved that occurrence onto the series'
date. The times now come from the row the expander produced for that instant, which is the engine's
answer rather than a second one computed here.

**What still comes from the series is an occurrence's own *fields***: a title, location or notes
the user gave one occurrence show the series'. Reading them means reading a JSCalendar patch, and a
parser for one belongs in the engine (`AGENTS.md` → "Protocol knowledge belongs in the engine"); the
engine builds overrides today and does not read them back. The grid has always drawn the series'
title on every block, so the detail agrees with the surface it was opened from.

¹⁵ **No harness can raise this one, on any platform.** The warning is the AND of what the account's
server does to overrides, whether this series holds any, and what the edit changes, and the two
transports the local
Stalwart harness speaks (CalDAV and JMAP) are exactly the two that **keep** a user's
per-occurrence work, so the core correctly answers "nothing to say" against it. Seeing the dialog
needs a Microsoft or Google account with a series one of whose occurrences has been moved or
renamed. What is machine-checked instead is the pair either side of it: the core's decision
(`series_warning_tests.rs` and `tests_calendar_series_warning.rs`, every combination including
the narrowing) and each client's mapping from verdict to sentence.

¹⁶ **Four controls, and one decision none of the five clients makes.** The editor offers a
frequency, an interval, a weekday row and an end condition. Which rules it may open, what a save
should send, and which parts of a rule survive an edit that never touched them are all
`EventDetail::repeat_draft` plus `repeat_change_of`: one answer, so five clients cannot disagree
about it, the same argument `repeat_summary` settles for the sentence. What stays each client's is
the **wording**, the platform's own weekday names, and the order its locale starts a week in.

The interval control never repeats the frequency word the picker above it already shows: a stepper
reading "Monthly" under a picker reading "Monthly" states nothing, so it reads "Every month" and
"Every 2 months" instead.

¹⁷ **The iPad's trackpad.** An indirect pointer's scroll never reaches a SwiftUI `DragGesture`, so
`CalendarScrollGesture` attaches a
`UIPanGestureRecognizer` with `allowedScrollTypesMask = .all` and `allowedTouchTypes = []` to the
window and filters by location: on the window, because a SwiftUI overlay is a **sibling** of the
content rather than its ancestor, so a recognizer attached there would only ever fire if the overlay
became the hit target, which would cost the grid every tap it has.

**Hand-verified on 2026-09-03**, on a booted iPad simulator, by a real two-finger trackpad gesture
over its window. ⚠️ It cannot be verified any other way: the Simulator does not forward a
*synthetic* host scroll into the guest at all, as a real `UIScrollView` on the same screen proves by
not moving either. A test that injects one passes while measuring nothing.

---

## 13. Known gaps

Stated, not buried.

- **The Windows month surface is invisible to assistive technology.** `MonthGridView` overrides no
  `OnCreateAutomationPeer`, so it is absent from the automation tree even while it is the view on
  screen: a screen reader finds nothing to read, and no test can address it. §7's "a drawn grid has
  to be taught to speak" is what closing it looks like: the time grid on the same client already
  does, which is why the two behave differently. Found on Windows; the other clients' month
  surfaces have not been checked against this.
- **Linux has no horizontal calendar gesture.** Its first Flatpak release deliberately uses header
  navigation and native vertical scrolling, with click-drag creation on empty grid (§12). It still
  has no swipe, horizontal trackpad strip, pinch, rapid-flick state machine, move or resize. Those
  rows remain ⬜; adding any of them inherits the whole pointer-owner and performance contract.
- **Apple, Windows and Android have no standing foreground calendar cadence.** Their launch fetch
  and tab visit keep the offline floor, but a client left open on Calendar can still remain stale.
  Linux now refreshes every five minutes after connection; the other clients need an equivalent
  host timer or a shared runtime watch.
- **The attendee list says who and how, not what kind.** A participant's `ROLE` is read only for
  `owner` (the organiser), so a **required** and an **optional** attendee are drawn identically:
  Outlook and Apple Calendar distinguish them. Nor is **your own row marked**; you find yourself by
  address. Neither is hidden information (both are in the list either way), and adding either means
  a field, a catalog key and four clients, so both wait for a reason. **No attendee can be added,
  removed or answered *for* from the calendar**: that is iTIP, and the invitation card is where an
  answer is given today ([`invitations.md`](invitations.md)).
- **Samsung is still ahead, though not by much.** Their 2.4% against our **3.6%**: the shipped number,
  after the week was banked on decision (§7's last column). This entry used to quote 6.5–7.4%, the
  canvas *before* that fix, and so overstated the remaining distance by about double; the p99 gap came
  down with it, from ~25 ms to 16.6 ms. What is left has not been chased down.
- **Only Android and Windows have the single gesture owner.** Apple has taken half of it: one owner
  for the *position*, `CalendarScreenView`'s strip, which every input reports a delta to, so a pan, a
  flick, a wheel notch, a pinch and a jump home cannot disagree about where the grid is. What it has
  not taken is one owner for the *pointer stream*: SwiftUI's own gesture system still arbitrates
  between the composed grid gesture, the pinch catcher and the scroll catcher. That arrangement is
  what §6 warns about, and the reason it has not bitten here is that the three read different inputs
  (a finger, two fingers, an indirect pointer) rather than the same one.
- **Windows: a hard multi-flick past the prebuilt halo can still build a week inside the swipe.**
  The p99 hitch measured earlier (a not-yet-seen dense week's paint: every event's colour parsed,
  clock formatted, spoken label assembled, built on the UI thread mid-fling, ~50 ms) is now taken
  off the swipe by §7's own rule: the moment the grid settles, the live-range neighbours out to a
  halo (±4 weeks) are painted on idle frames, presenting nothing, so a page turn lands on a week that
  is already drawable. This covers the common cadence (flick, settle, flick) and two quick flicks in
  a row completely. What it does **not** cover is a *sustained* multi-flick that outruns the halo into
  never-seen, event-heavy weeks: there the edge week is still built at the fling's leading edge, one
  hitch per new week beyond the halo. Widening the halo trades memory and idle work for it. **The
  before/after has not been re-measured on the real diary yet** (§7 forbids claiming a perf win
  without it); until then this entry states the change, not a result.
- **Windows: the frame budget is measurable, but the tuning loop hasn't run.** The instrument problem
  is *solved*: the grid presents into its own swapchain, so PresentMon sees it, and
  `clients/windows/calendar-perf.ps1` reports the in-motion gaps on a release build. What has not
  happened is the loop Android went through: this is a first pass on an ARM64 Surface (a modest GPU,
  a 60 Hz panel), and the fling decay and settle spring were ported from Android's numbers rather than
  re-measured against this hardware. Samsung's 2.4% (at 120 Hz) is the bar; a 60 Hz result cannot be
  compared to it directly, and where Windows sits on a 120 Hz display is an open number.
- **A Windows touchpad pan still has no phase, and what that costs is now a quarter-second.** Windows
  hands a precision touchpad's two-finger pan to the app as wheel messages with **no**
  begin/end/inertia flag, so a lift, the OS's momentum scroll and an active pan are indistinguishable
  bursts (§6). The grid infers the end from silence, and the window has to clear the gap between two
  notches of the same gesture (~150 ms on a mouse) or it resolves a gesture still in progress, so a
  pan lands on its day 250 ms after the fingers actually stop, and a zoom banks 350 ms after. A pinch
  that *pauses* mid-gesture for longer than that still banks early. The real fix is what Chromium and
  Firefox do: hook **DirectManipulation** (or read the raw HID contacts via `RegisterRawInputDevices`
  / the InteractionContext API), which would also buy true fling velocity and overscroll from the
  touchpad, and would let both windows go to zero.
- **The strip has not been brought to Linux.** Windows, macOS and iOS/iPadOS rest on a day, at every
  zoom and for every input; Android pages by the week *deliberately* (footnote ⁴) and is not part of
  this gap. Linux is further back than the others were: its grid is a `gtk::ScrolledWindow` with one
  capture-phase primary-button controller for creating events, but no owner for horizontal navigation
  or pinch, so it needs the single pointer owner before it can have the strip. **A platform takes the
  whole rule or none of it**: half of it is the rubber-band.
- **Apple's grid is not frame-budget measured.** §7's bar is the frames missed *during motion*, on a
  release build, and nobody has taken that number on a Mac or a phone. It is also the one client whose
  grid is **composed rather than drawn** at the block level, which §7 says costs a layout pass per
  block per frame, so the measurement is more likely to say something than on the clients that already
  pass. Until it is taken, no performance claim may be made for the Apple strip.
- **The Windows day landing has not been frame-measured on a 120 Hz panel.** The wheel path was
  measured on a 60 Hz ARM64 Surface, before and after, by injecting notches at a known cadence and
  reading the grid's own frame counters. The ease constant (a 60 ms time constant) and the two idle
  windows were chosen against *that* hardware and a mouse's ~150 ms notch gap; where they sit on a
  120 Hz display, or against a trackpad that streams an order of magnitude denser, is an open number.
  Not done.
- **Only Windows has the continuous strip; Android and Apple still page, with a sliding ruler.** The
  divergence is deliberate and is driven by an input the phones do not have (§6, note ⁴), but two
  clients now draw the same grid in two ways, and a user with both will feel it: on the phone the week
  is a wall, on the desktop it is not. The strip is the better model on any input (it is what macOS
  Calendar does), so the intended end state is one strip everywhere; Android's `dayX`/`pageOffset`
  hand-off and its per-page gutter are what stand in the way. **Not scheduled.** Until it is done, a
  reader of §6 must check the platform tag on the rule they are relying on.
- **The shell's capture bookkeeping is not pinned by a test: only the pure owner is.** The leak that
  killed the touchscreen (§6) was in the *shell's* set of held captures, and `CalendarMultiTouchTests`
  cannot reach it: it tests the framework-free owner, which was blameless. The test that would have
  caught the real bug needs a window and real contacts: `touch.ps1` can now inject them (twenty
  pinches, then a swipe must still turn the week, with the process's handle count flat), so this is a
  test that can be written and has not been. Until it is, a regression in the capture set is caught by
  a human noticing the touchscreen has gone dead, a minute later.
- **The all-day banner's height changes as a seam scrolls in.** With the ruler pinned, the content top
  is one height for the whole surface, and it is the *largest* of the weeks on screen, so scrolling a
  three-lane week into view beside an empty one steps the grid's top edge down as it appears. It is
  correct (the alternative is hour lines that miss the ruler on one side of the seam) and it is not
  animated. Nobody has complained yet; if it reads as a jump, the fix is to tween the content top rather
  than to give the weeks separate ones.
- **A Windows touchpad cannot pinch diagonally, and a mouse cannot pinch at all.** Windows never
  hands an app a precision touchpad's raw contacts: a pinch arrives as a scalar `Ctrl`+wheel, so the
  day axis has no second component to zoom by, and the touchpad zooms hours only. The diagonal pinch
  needs the touchscreen. This is the same shortfall macOS has (below), arrived at from the opposite
  direction: macOS *can* read the trackpad's touches and cannot read a mouse's; Windows can read a
  touchscreen's and cannot read a touchpad's.
- **A Mac's mouse wheel still guesses when its gesture ended.** A trackpad reports phase, so the strip
  lands because the fingers lifted; a wheel reports none at all, so that path keeps a 250 ms idle
  window and lands a quarter-second after the last notch, exactly as Windows does and for exactly the
  same reason. It is the one Apple input that cannot say when it has stopped.
- **A Mac with no trackpad has no pinch at all** (a mouse sends no magnify events): the shape is a
  menu choice there. SwiftUI's `MagnifyGesture` is a scalar and cannot do this; the diagonal pinch
  works by reading the `NSTouch` objects off the raw magnify event, which know where the fingers are
  on the trackpad. A device that magnifies without reporting two touches falls back to hours-only.
- ~~**Apple pages weeks by header chevrons, not by a swipe.**~~ Closed, and not by adding a pager: the
  hand-off at a week's edge that SwiftUI would not do for free stopped existing when the days became
  one strip (footnote ⁴). The chevrons remain, and step a **week** rather than the visible span, which
  is the `< Today >` row still reading ⬜.
- **The month grid repeats a multi-day event as a chip on each of its days** rather than drawing one
  continuous bar across the span. Google draws the bar.
- **Saturday-start weeks** (much of the Middle East) are a real convention `WeekStart` does not cover.
- ~~**Nothing warns before a series edit discards a user's per-occurrence work.**~~ Closed: every
  client reads `EventDetail::series_edit_warning` and puts it between Save and the write. The one
  thing to know about it is that **no local harness can raise it** (note ¹⁵): the transports that
  destroy overrides are the two no fixture speaks.
- ~~**No client writes a repeat rule yet.**~~ Closed: all five now offer the controls, seeded
  from `EventDetail::repeat_draft`.
- **The repeat controls cannot express a monthly or yearly rule's anchor.** "The last day of the
  month" and "the second Monday" are rules the editor **keeps** but does not offer: open such a
  series and the frequency, interval and end are editable while the anchor rides along untouched,
  and changing the frequency drops it. Nothing on screen names the anchor, so a user who wanted to
  move "the second Monday" to "the third" has no way to say so and no way to see why. Closing it
  is a control (a *day of the month* row for monthly, a *position* row for monthly and yearly),
  not a contract change: `SimpleRecurrence` already carries both, and the round trip that decides
  `Simple` against `Complex` already lets them through.
- **A rule too rich to state cannot be stopped either.** `EventRecurrence::Complex` gets the
  sentence and no controls, which is right for *changing* a rule the client only half sees. But
  the core allows a `Clear` over one, because stopping a repeat needs no knowledge of the rule it
  stops. No client offers that, so the only way out of a complex series is to delete it.
- **On Windows the scope question is a UI suite case, but its destructive answer is not.**
  `uitests/EventSeriesScope.Tests.ps1` drives the drawn grid the way the wheel suite does: the
  event peer publishes a live physical-pixel rect and the click is injected at the OS level, since
  the peers carry no Invoke pattern, and asserts that the question is put from the grid, not put
  from the agenda, and writes nothing when backed out of. What it does **not** assert is that
  answering *This event* removes the right one: the oracle for that is the `EXDATE` the server ends
  up holding, and the runner has no route to the harness's store. That answer is still a hand
  check.
- ~~**The warning is decided from the edit, but no client tells it what the edit is.**~~ Closed:
  `MailcalApp::series_edit_warning` takes the payload the save is about to dispatch, and all five
  clients ask it there. A retitle on Graph is no longer told it will move occurrences it does not
  touch.
- ~~**On Windows, cancelling the warning discards what the user typed.**~~ Closed: `ShowEditor`
  is a loop, so backing out of either question reopens the editor over the same
  `EventEditorState`, which the dialog reads and writes in place, so what was typed is still in
  it. WinUI still permits one `ContentDialog` at a time; reopening is how that constraint is paid
  for rather than passed to the user.
- ~~**No client asks on an *edit*.**~~ Closed, together with the wrong date that blocked it: a
  detail now reports the times of the occurrence it was opened on (note ¹⁴).
- **An occurrence's own title, location and notes are still the series'.** The times come from the
  expander, which is the engine's own answer; the *fields* live in a JSCalendar patch on the
  override, and reading one here would be a second implementation of a format the engine owns
  (`AGENTS.md`): it has an `OverrideBuilder` and no reader. So an occurrence the user renamed
  opens under the series' name, on every client and on every surface: the grid draws the series'
  title on every block too, so nothing on screen disagrees with anything else. Closing it is an
  engine change first.
- **Dragging is shipped on macOS, iOS/iPadOS, Android and Linux, which has create only.** §10 is
  the contract; what is *not* done:
  - **Windows has no drag; Linux has no move or resize.** Both are hard cases rather than leftovers.
    Windows hands a precision touchpad's pan to the app as **phase-less wheel messages** (§6), so
    "the pointer is holding an event" has no begin or end to hang a drag off: the touchscreen would
    work today and the touchpad would not, and shipping half of it is worse than shipping none.
    Linux's click-drag starts only on empty space; event blocks remain clicks until their move/resize
    ownership and repeating-event question are implemented.
  - **All-day bars cannot be dragged**, on any platform. The banner is a second geometry with its own
    rule (an **exclusive** end date, §1), and a bar that moves by whole days is a different gesture
    from a block that moves by minutes. Left out rather than approximated.
  - **A segment clipped by midnight cannot be dragged**, on any platform. Its visible rectangle is a
    *clip* of the event, not the event, so every gesture on it would mean something other than what it
    looks like: a resize would pull an edge the grid invented, and a move would preview a jump the
    drop does not make. The editor still moves it; the grid declines to.
  - **No edge auto-scroll.** Dragging to the top or bottom of the viewport does not scroll the grid,
    so a move is bounded by the hours on screen. Google and Samsung both scroll here. The fix is a
    frame loop *inside* the gesture, which is exactly the kind of thing §6 says to be careful with, so
    it is a deliberate follow-up rather than an oversight.
  - **No undo.** A drop writes immediately, and the way back is another drag (or the editor). The
    mail list has `SwipeUndoController`; the calendar has no equivalent, and a drag is the first
    calendar gesture that can lose data to a slip of the hand.
  - **iOS/iPadOS is compile-verified, not hand-verified**: one Swift package draws it and macOS, and
    only macOS was driven (note ¹⁰).
- **Somebody else's meeting cannot be dragged, and there is nothing to do instead yet.** `can_move` is
  `false` on a meeting we did not organise, so the block does not lift, correct, and incomplete: the
  answer a calendar should offer there is **propose a new time**, which is iTIP `COUNTER` and a
  feature of its own (a new engine verb, an outgoing scheduling message, and a card for the reply).
  Until it exists, moving somebody else's meeting means asking them.
- **Writes are not optimistic, and the engine's outbox is not an offline queue.** `create_calendar_event`
  and `patch_calendar_event` await their network round-trip inline; there is no background drainer.
  Create or edit an event on a train today and it silently never reaches the server: the write's
  `CalendarWriteStatus` settles `Failed`, which is honest, but the change is lost rather than queued.
  This applies equally to create, **edit**, and delete: the editor rides the *same* inline-await path
  they do. The earlier gate that withheld `Intent::UpdateEvent` from the FFI "until the write is
  durable" is **lifted**: an editor that surfaces its own failures is no worse than the create that
  already shipped that way, and blocking it while create/delete ship without the outbox was
  inconsistent. The durable outbox that fixes all three at once is the tracked follow-up.
- **Write capability is now surfaced per row, but not yet per-calendar, and only as a boolean.**
  `CalendarRow`, `TimedSegment`, `AllDayBand`, `MonthChip` and `EventRow` all carry `can_write`, set
  from the account's provider `Capabilities.calendar_write_guard()`. This lets a client hide edit
  affordances for read-only accounts. Two gaps stay open: per-calendar write rights
  (`Calendar.access.may_write`) are still not sourced from the provider, so every calendar in a
  writable account reports the same flag; and the boolean collapses the guard's two writable states.
  `WriteGuard::Enforced` (CalDAV, **Microsoft Graph, and Google Calendar**, a stale edit is refused
  with a `412`) and `WriteGuard::Absent` (JMAP, a stale edit **silently wins**, last-writer-wins)
  both read as `can_write: true`, so a
  client cannot yet tell the user that a JMAP save carries no conflict protection. Do not word a
  success toast in a way that implies one. The third gap is closed: **every client now gates its
  write affordances on the flag**, under one cross-client policy. A per-event **delete** affordance
  (a swipe action, an inline button, a flyout item) is **hidden** when its own row reports
  `can_write: false` (an affordance that can never fire is just a mystery) while the global
  **"New event"** button is **disabled**, never hidden, so the header keeps its shape; it enables
  only when some `CalendarRow` on the page reports `can_write: true`, and an empty calendar list
  (nothing synced yet) counts as no. The create routing keeps that promise: `calendar_account`
  targets the selected account only when it can write, else the first account that can, and a
  create with no writable account anywhere is the documented no-op, so an enabled button cannot
  route a write to an account the core knows will refuse it. Two consequences worth knowing. The
  **showcase** dataset's calendar provider advertises no write guard, so under showcase the
  calendar now renders read-only (New event disabled, no delete affordances), honest, since its
  writes always went into a void, but it changes what showcase screenshots show. The **Windows**
  wiring (three XAML binds over a `dotnet`-tested pure gate, `CalendarWriteGating`) has now been
  built and verified on a real Windows host: `New event` reads `IsEnabled=false` under the
  read-only showcase and `true` against the harness, and the agenda's per-row delete affordance is
  absent on every read-only row and present on every writable one, so the matrix marks it shipped.
- **JMAP calendar writes now work, and the harness exercises them.** The core's create/patch/delete
  ride the engine's provider-neutral write verbs, so the *same* host code writes to CalDAV and JMAP;
  a JMAP write used to fail silently with `InvalidState`. `live_jmap` proves a create → read-your-write
  → patch → delete round-trip against the Stalwart harness over JMAP. What the JMAP harness still
  cannot exercise is the **`Enforced` guard path** (the `412` on a stale edit): that is CalDAV-only,
  and a `stalwart-caldav` dev account is the fix for driving it.
- **Microsoft 365 (Graph) calendars now read/sync and write.** A Microsoft account binds a
  token-refreshing Graph calendar provider alongside its mail providers (the same shared token +
  concurrency gate), gated on the `Calendars.ReadWrite` OAuth scope, so it rides the *same*
  provider-neutral create/patch/delete verbs and per-row `can_write` gating as CalDAV/JMAP, and
  Graph advertises `WriteGuard::Enforced` (a stale `If-Match` ETag is a `412`). It is host-driven
  and needs no client change: a connected Microsoft account's agenda simply appears. Four limits
  ride in from the engine's `graph.md` and are **not** yet closed: only the account's **default
  calendar** is bound (the Graph parallel of CalDAV's primary-calendar bind); the fetch **window**
  is baked in at connect (a few months back, ~a year on) and re-centred only on reconnect; **one
  display zone per provider** (the device zone, sent as `Prefer: outlook.timezone` for
  DST-correct expansion; the user's *chosen* display zone still re-projects the agenda in the
  view-model). The CalDAV `412` harness gap above applies to Graph too: no dev account drives its
  `Enforced` path locally.

  A `patch` reaches **one occurrence** here as it does everywhere else. Graph and Google address
  an occurrence by an id they derive rather than by a `RECURRENCE-ID`, and Google derives it from
  the occurrence's original start **in UTC**, refusing a timed target that has not resolved one.
  So `build_event_patch` resolves that instant beside the wall clock; an all-day or floating
  occurrence resolves to none and needs none, because Google addresses that one by date.
- **Google Calendar reads/syncs and writes, natively, and needs no reconnect.** A Google account
  binds a token-refreshing Google Calendar provider alongside its Gmail provider (the same shared
  token source), riding the *same* provider-neutral create/patch/delete verbs and per-row
  `can_write` gating as CalDAV/Graph/JMAP; it advertises `WriteGuard::Enforced` (a stale edit is a
  `412`). Two differences from Graph. **It is IANA-native**: Google's API speaks real time-zone ids,
  so there is **no display-zone `Prefer` header** and none of Graph's one-zone-per-provider caveat:
  the view-model's chosen-zone re-projection is the only zone step. And **both scopes are granted at
  connect** (Gmail + Calendar in one consent), so there is **no scope-upgrade reconnect**: the
  "reconnect to enable calendar" banner below is Microsoft-only; a Google account's agenda simply
  appears. The same engine-side limits ride in as for Graph: only the account's **primary calendar**
  is bound, and the fetch **window** is baked in at connect (≈120 days back, ≈400 forward) and
  re-centred only on reconnect. The `412` harness gap applies here too: no dev account drives the
  `Enforced` path locally.
- **A scope-missing Microsoft account is told to reconnect, not left silently calendar-less.** Every
  Microsoft account connected **before** calendar support (or with revoked consent) lacks the
  `Calendars.ReadWrite` scope, so its calendar-list probe `403`s (`ErrorAccessDenied`). The core
  classifies that specific `403` as `AccountError::CalendarAccessDenied` (distinct from a transient
  failure), records the account in a per-account **`calendar_reauth_accounts`** set on
  `ConnectivitySnapshot` (signalled on `Surface::Connectivity`, **not** suppressed while offline;
  it is a standing permission gap, and mail is unaffected), and every client renders a **"reconnect
  to enable calendar"** banner on the calendar naming the account. Its action re-runs that account's
  Microsoft sign-in with the address as `login_hint` and the now-expanded scopes; because the
  re-auth resolves to the **same account id**, `complete_microsoft_login` upgrades the existing
  account's token **in place** (no duplicate), reconnects the calendar, and clears the flag (which
  clears the banner). This is a **cross-platform contract**: the signal is shared and all four
  clients (Android: device-verified; macOS + iOS/iPadOS: one SwiftUI banner, compile-verified;
  Windows: InfoBar, compile + UIA owed on a Windows host; Linux: GTK banner and loopback sign-in,
  runtime-verified) render it. The **whole loop** was verified live on a real Android device
  2026-07-18 and on Linux 2026-08-25: the `403` classified as `CalendarAccessDenied`, the banner
  named the affected account, **Reconnect** opened the Microsoft consent for
  `Calendars.ReadWrite` targeted at that account via `login_hint`, and on consent the token upgraded
  in place, the calendar connected, and the banner cleared. The consent **click** is the one step
  that needs a real Microsoft account and cannot be driven from the harness.
- **A calendar write's result is surfaced to the user, not just logged.** Every create/edit/delete
  drives `CalendarWriteStatus` (`Idle` → `Saving` → `Saved`/`Failed`), pulled after a
  `Surface::CalendarStatus` signal via `App::calendar_write_status`: a client shows a small spinner
  while it settles and a warning when it could not be confirmed. The load-bearing rule from
  read-your-writes is encoded here: a `Reconciled::Busy`/`Failed` write **landed on the server**, so
  the core recovers by *re-reading* (`reconcile_calendar_events`), **never** by re-issuing the write,
  and `Failed` means "could not confirm the local view," not "your change was rejected." An explicit
  `RefreshCalendar` clears the status (a full sync reconciles the whole scope); a genuine outage shows
  through `Surface::Connectivity`, not here. **Cross-platform status:** the surface is shared;
  **Android** and the **Apple** clients (macOS + iOS/iPadOS, one SwiftUI header) render it today
  (spinner/warning/retry). **Windows** has the header badge wired the same way (the pure
  `CalendarWriteIndicators.Of` mapping is unit-tested via `dotnet test`, and the model + XAML mirror
  the send-status hint), but it is **🚧 pending verification on a real Windows host**: WinUI XAML and
  the view-model cannot be built off-Windows, so its first green build is the Windows CI job, not a
  local run. Until then the matrix marks it in progress, not shipped.
