# Video meetings: cross-platform contract

**Scope.** The meetings an event can be joined at: what the event detail offers for each, how
the grid, the month and the agenda mark an event that has one, and how the editor puts one on an
event or takes it off, at the calendar's own server, at the person's Jitsi server, or through an
account the person connected (Teams through a Microsoft work account, Talk through a Nextcloud
one).

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
   number the policy refuses (a service code), which is then shown as text. A PIN the server
   wrote only into the URI is the row's `pin`, since the tap no longer dials it. Nothing opens inside
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

9. **The core says which meetings a calendar can have, a client draws them.** Each
   `CalendarRow` carries `meeting_choices`, in order: the services the calendar's own server
   makes (Teams on a Microsoft calendar, Meet on a Google one), where its account's provider can
   write a meeting, then **Jitsi Meet** once a Jitsi server is set (rule 13). A calendar that
   cannot be written has none. The editor draws a **Video meeting** row only while the chosen
   calendar has a choice: **None** first, then each choice by the core's `name`. Changing the
   calendar redraws the row from the new calendar's choices, and a choice the new one lacks goes
   back to None.
10. **A new event starts with no meeting.** The person adds one by choosing it; nothing is added
    on their behalf.
11. **Editing an event shows the meeting it has.** The row shows the event's first shown meeting
    (`is_shown`) as its current value. Choosing None takes that meeting off, with the block of text
    and the location it left; choosing another service replaces it; leaving the row alone leaves
    the meeting alone. A meeting no choice makes (a Zoom link from an invitation) is shown by its
    name as the current value, and can be taken off but not chosen again. A meeting belongs to the
    series, so in an editor opened on one occurrence a changed meeting makes the save the
    series', as a changed repeat or roster does, rather than asking which occurrences were meant;
    the core refuses a meeting change sent with one occurrence.
12. **A meeting the calendar's server makes is asked of that server**; one made elsewhere is put
    on the event with a block of text in its notes, so invitees whose calendar reads no meeting
    field still have the link. The block's words are the client's, from the catalog, sent with
    the write, because the core has no locale. A Jitsi meeting sends nothing anywhere: its link is
    the server's address and a room name the core draws from the OS's secure random source, and
    the room exists only once someone opens it.
13. **The Jitsi server is the person's, never a default.** Settings → Videoconferencing holds one
    field, empty until the person fills it: an `https` address without a query or fragment, which
    the core checks before it stores it. While it is empty, no calendar offers Jitsi.

14. **An account used for video meetings makes them for an event on any calendar.** A Microsoft
    account an organisation administers makes Teams meetings, and an account a Nextcloud sign-in
    made makes Talk meetings on its server, each only while the account is used for `meetings`
    ([`accounts.md`](accounts.md) rule 19). Each such account adds a choice to every writable
    calendar's `meeting_choices`, after the calendar's own services and before Jitsi, carrying
    the account's address (`through`); a client draws it as the service's name with that address
    (`event_meeting_through`), so two work accounts are two choices a person can tell apart. A
    service the calendar's own server makes is offered as the server's only: a Microsoft calendar
    lists its own Teams, not Teams through its account as well. An event's Teams or Talk meeting
    that its server did not make opens the editor on the choice through an account when exactly
    one account offers that service, and is otherwise a meeting no choice makes (rule 11).
15. **A meeting made through an account is made before the event is written**, from the event's
    title and times and nothing else: no invitees, no notes, no location. The event is then
    written with it attached, as one made elsewhere (rule 12). If the meeting cannot be made,
    nothing is saved and the editor says so; if the event cannot be written, the meeting just
    made is deleted at its service, so no meeting is left that no event names. That is the only
    delete: taking a meeting off an event, replacing it, or deleting the event leaves the meeting
    at its service, because nothing in an event can say this app made it (on a Microsoft calendar
    such a meeting is only our block in the body), and deleting one the person pasted in would
    delete their meeting.
16. **Every meeting service is called through one place in the core** (`App::make_meeting` and
    `App::delete_meeting`), which is where the jurisdiction check will sit
    ([`provider-oauth.md`](provider-oauth.md) "Sovereignty scope"). A Jitsi meeting passes
    through it too, though making one sends nothing.

## Per-platform

| | macOS | iOS/iPadOS | Windows | Android | Linux |
|---|:---:|:---:|:---:|:---:|:---:|
| The detail's meetings, Join first (rules 2 to 5) | ✅ | ✅ | ✅ | ✅ | ✅ |
| The detail shows `display_notes` and `display_location` (rule 6) | ✅ | ✅ | ✅ | ✅ | ✅ |
| The camera glyph and its spoken label (rule 7) | ✅ | ✅ | ✅ | ✅ | ✅ |
| The editor's Video meeting row (rules 9 to 12) | ✅ | ✅ | ✅ | ✅ | ✅ |
| The Jitsi server in Settings (rule 13) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Teams and Talk through an account, in the editor (rules 14 and 15) | ✅ | ✅ | ✅ | ✅ | ✅ |

On Apple the section is `EventConferenceView.swift`, and `EventConferenceTests` pins that Join and
a dial-in row open the core's links as sent. On Windows the section is `EventConferenceSection.cs`
over the WinUI-free `EventMeeting.cs`, which `EventMeetingTests` pins the same way; the grid draws
its camera in `CalendarTitleDraw.cs`. The editor's Video meeting row is drawn by
`EventEditorDialog.cs` over the WinUI-free `EventEditorMeeting.cs`, which `EventEditorMeetingTests`
drives, and the Jitsi server is `SettingsDialog.Videoconferencing.cs`. On Android the section is `EventConferenceSection.kt` over
`EventMeeting.kt`, and `EventMeetingTest` drives the section; the grid draws its camera in
`CalendarTitleDraw.kt`. On Linux the rows are `conference.rs` over `meeting.rs`, and the grid
paints its camera in `grid/camera.rs`. The editor's Video meeting row is `meeting_row.rs` over the
GTK-free `editor_meeting.rs`, which its unit tests drive, and the Jitsi server is
`settings/videoconferencing.rs`.

## Enforcement

- [`event_conference_tests.rs`](../crates/mailcal-account/src/event_conference_tests.rs) pins
  rule 3's links: a service code is never offered to dial, a PIN only the URI carries is still
  shown, and a meeting still being made offers no Join whatever link it carries.
- `is_shown` and `has_conference` in
  [`meeting.rs`](../crates/mailcal-viewmodel/src/calendar/meeting.rs) are the one rule; the
  detail's projection ([`event_conference.rs`](../crates/mailcal-account/src/event_conference.rs))
  and every row call it.
- [`tests_calendar_conference.rs`](../crates/mailcal-app/src/tests_calendar_conference.rs) reads
  a meeting through the app: the detail offers it and the grid marks its block, and the
  block's notes stay out of `display_notes`.
- A field added to `EventConference` is drawn on every client that ships rule 2, or it is not
  added.
