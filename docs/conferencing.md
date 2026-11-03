# Video meetings: cross-platform contract

**Scope.** The meetings an event can be joined at: what the event detail offers for each, and how
the grid, the month and the agenda mark an event that has one. Making a meeting, attaching one
and taking one off an event are not built, and this contract says nothing about them yet.

**Principle.** A meeting is something to join, not a link to read. The detail puts **Join** first
and the codes and numbers a person needs beside it, the way Outlook and Google Calendar do, so
nobody has to dig a link out of the notes.

## The rules

1. **The engine reads, the core decides, a client draws.** The engine reads every protocol's
   meeting fields, and the links an invitation leaves in a vendor property, a location or the
   description, into one conference per meeting (its `calendar-semantics.md`, "Conferences").
   The core flattens each into `EventDetail.conferences` and sets `has_conference` on every
   block, bar, chip and agenda row. Both answers come from one predicate,
   `mailcal_viewmodel::calendar::meeting::is_shown`, so the glyph and the detail cannot disagree.
   A client adds no meeting it found itself and drops none the core sent.
2. **What the detail shows for a meeting**, in this order: the service's name (`service`) with
   **Join** (`join_url`), then the meeting ID, the passcode, each dial-in number with its PIN,
   and the meeting's own `notes`. A dial-in `number` is the engine's reading of it, never the
   `tel:` URI's text. **Copy link** copies `join_url`. A row with no value is left out, never
   drawn empty. Several meetings are drawn one after the other, in the core's order.
3. **Join and dial-in open through the OS, and the core decides what they open.** Both links
   pass the link policy a clicked link in a message passes (`is_openable_link`) when the detail
   is built, so a client opens them as they are and checks nothing itself. `join_url` is set
   only for a ready meeting and only for an `https` link; the OS hands it to the service's app
   where one is installed, else to the browser. A dial-in `uri` is a `tel:` URI for the phone
   app, the number alone when the server's own carries a pause and a PIN, and `None` for a
   number the policy refuses (a service code), which is then shown as text. Nothing opens inside
   the app, and nothing is opened until the person taps: reading an event sends nothing anywhere.
4. **The service's name is the core's.** It is a brand name, the same in every language. When
   `service` is `None` (a link no rule recognises, such as a self-hosted server), the client
   shows its own catalog words for a video meeting, never the host name or a guess.
5. **A meeting the server is still making** (`state: Preparing`) shows "Preparing the meeting
   link" and no Join; one it could not make (`Failed`) says so. Neither offers anything to tap.
6. **The detail shows `display_notes`, the editor edits `notes`.** `display_notes` leaves out
   each block of text Teams and Google write into an invitation's description whose meeting the
   detail shows, since it repeats that meeting; the block of a meeting not shown stays, since
   it is then the only way in. The engine decides which meeting a block is of
   (`Event::notes`), given `is_shown`.
   `notes` is the description as the server holds it, block included, so a save from the editor
   does not take the link from every invitee. The same holds for the place: the detail shows
   `display_location`, which leaves out a location that is only a shown meeting's link, and the
   editor edits `location`, link included.
7. **The glyph.** A block, bar, chip or agenda row with `has_conference` carries a small video
   camera glyph after its title, from the platform's own set ([`icons.md`](icons.md)), and its
   accessibility label says the event has a video meeting. The glyph is never the only way the
   meeting is told: the detail always lists it.
8. **Everything a meeting carries came from whoever wrote the event**, an invitation's sender
   included. A client renders it as text, never markup, and never logs a link, a meeting ID or
   a passcode: a Zoom link carries its passcode in the URL ([`logging.md`](logging.md)).

## Per-platform

| | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|
| The detail's meetings, Join first (rules 2 to 5) | ✅ | ✅ | ✅ | ✅ | ✅ |
| The detail shows `display_notes` and `display_location` (rule 6) | ✅ | ✅ | ✅ | ✅ | ✅ |
| The camera glyph and its spoken label (rule 7) | ✅ | ✅ | ✅ | ✅ | ✅ |

On Apple the section is `EventConferenceView.swift`, and `EventConferenceTests` pins that Join and
a dial-in row open the core's links as sent. On Windows the section is `EventConferenceSection.cs`
over the WinUI-free `EventMeeting.cs`, which `EventMeetingTests` pins the same way; the grid draws
its camera in `CalendarTitleDraw.cs`. On Android the section is `EventConferenceSection.kt` over
`EventMeeting.kt`, and `EventMeetingTest` drives the section; the grid draws its camera in
`CalendarTitleDraw.kt`. On Linux the rows are `conference.rs` over `meeting.rs`, and the grid
paints its camera in `grid/camera.rs`.

## Enforcement

- [`event_conference_tests.rs`](../crates/mailcal-account/src/event_conference_tests.rs) pins
  rule 3's links: a service code is never offered to dial, and a meeting still being made offers
  no Join whatever link it carries.
- `is_shown` and `has_conference` in
  [`meeting.rs`](../crates/mailcal-viewmodel/src/calendar/meeting.rs) are the one rule; the
  detail's projection ([`event_conference.rs`](../crates/mailcal-account/src/event_conference.rs))
  and every row call it.
- [`tests_calendar_conference.rs`](../crates/mailcal-app/src/tests_calendar_conference.rs) reads
  a meeting through the app: the detail offers it and the grid marks its block, and the
  block's notes stay out of `display_notes`.
- A field added to `EventConference` is drawn on every client that ships rule 2, or it is not
  added.
