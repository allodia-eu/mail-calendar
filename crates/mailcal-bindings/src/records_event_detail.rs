//! The FFI mirror of a single event's detail: what a tap on an event opens, and what the
//! editor prefills from.
//!
//! Its own file because `records_calendar.rs`, which holds the grid's records, is at the
//! 500-line limit.

/// A single event's full detail: the detail view a tap opens, and what the editor prefills
/// from (see `MailcalApp::event_detail`).
///
/// Times are the event's **own wall clock**: a bare date `YYYY-MM-DD` when `all_day` (the end
/// exclusive: a one-day event on the 1st ends on the 2nd), else `YYYY-MM-DDTHH:MM:SS`. The
/// client resolves `calendar` to a name and colour from the page snapshot's `calendars`, and
/// localises `reminder_minutes` and the `recurrence` summary.
#[derive(uniffi::Record)]
pub struct EventDetail {
    /// The owning account's id.
    pub account: String,
    /// The event's provider key.
    pub key: String,
    /// The event's calendar key (`CalendarRow.id`).
    pub calendar: String,
    /// The title (may be empty: the client shows its own placeholder).
    pub title: String,
    /// Whether this is an all-day event.
    pub all_day: bool,
    /// The event's own IANA zone, or empty for a floating or all-day event.
    pub timezone: String,
    /// The start as the event's own wall clock (a bare date if all-day).
    pub start: String,
    /// The end, same terms as `start`; exclusive date if all-day.
    pub end: String,
    /// The location as the server holds it, if any: what the **editor** prefills from. It
    /// keeps a meeting's join link, so a save does not take the link from every invitee; show
    /// `display_location` instead.
    pub location: Option<String>,
    /// The description as the server holds it, if any: what the **editor** prefills from.
    /// It keeps the meeting block an invitation writes, so a save does not take the link from
    /// every invitee; show `display_notes` instead.
    pub notes: Option<String>,
    /// Minutes before the start of the first reminder, or `None`. Read-only: no client offers
    /// to change a reminder yet.
    pub reminder_minutes: Option<i32>,
    /// The repeat rule, or `None` if the event does not repeat. A client localises the
    /// summary itself; [`EventRecurrence::Complex`](crate::EventRecurrence) means it repeats
    /// on a rule the editor does not model; say so, and offer no edit.
    pub recurrence: Option<crate::EventRecurrence>,
    /// The rule as the parts a **sentence** needs (the rhythm and what ends it) or `None`
    /// when the event has no rule, or one too rich to state exactly (then say only that it
    /// repeats). See [`RepeatSummary`](crate::RepeatSummary).
    pub repeat_summary: Option<crate::RepeatSummary>,
    /// The rule as an editor's **controls** hold it, or `None` when the editor may not open
    /// it: no rule, one too rich to state, or one whose controls this app does not have.
    /// Then show the summary and offer no edit. See [`RepeatDraft`](crate::RepeatDraft).
    pub repeat_draft: Option<crate::RepeatDraft>,
    /// Whether the event recurs: so the editor can tell the user an edit hits the whole series.
    pub is_recurring: bool,
    /// Whether the calendar can be written; gates the edit and delete affordances.
    pub can_write: bool,
    /// Whether this account organises the meeting and may change its invitees.
    pub can_edit_invitees: bool,
    /// This account's own answer, when it may change it here (`EventIntent::Respond`): it is
    /// invited, does not organise the meeting, and has a way to tell the organiser. `None`
    /// offers no answer.
    pub my_answer: Option<crate::records_invitation::ResponseStatus>,
    /// The occurrence this detail describes, as the token that named it; empty when it
    /// describes the **series**, which is what an agenda row and a one-off event always do.
    ///
    /// **This, not the token the client sent, is what a scope question is asked from.** It
    /// comes back empty when the core could not resolve what was sent (the series changed
    /// underneath the view it was drawn in), and the times above are then the series': so a
    /// client that reads this can never offer *This event* against another occurrence's times.
    ///
    /// Hand it straight back as `EventIntent::Update`/`Delete`'s `occurrence`.
    pub occurrence_start: String,
    /// Everyone on the event, organiser first; empty for an appointment nobody was invited to.
    pub attendees: Vec<EventAttendee>,
    /// The notes the detail view shows: `notes` without each meeting block Teams and Google
    /// write whose meeting is in `conferences`; the block of a meeting not shown stays, as the
    /// one way into it. `None` when nothing else is left.
    #[uniffi(default = None)]
    pub display_notes: Option<String>,
    /// The location the detail view shows: the first that is not only a link of a meeting in
    /// `conferences`, which already offers it as Join.
    #[uniffi(default = None)]
    pub display_location: Option<String>,
    /// The meetings the event can be joined at, the calendar server's own first; empty for
    /// most events. The detail shows each with its Join button first (`docs/conferencing.md`).
    #[uniffi(default = [])]
    pub conferences: Vec<crate::EventConference>,
    /// The meeting the editor's Video meeting row opens showing, or `None` for an event with
    /// none (`docs/conferencing.md`, rule 11).
    #[uniffi(default = None)]
    pub editor_meeting: Option<crate::EditorMeeting>,
}

/// One person on an event, for the detail view's attendee list.
///
/// A roster, where `AttendeeTally` is a count: the tally answers "how is this meeting going"
/// above a message body, this answers "who is coming" on an event the user opened.
///
/// **Every string here is attacker-controlled plain text** (it came from whoever sent the
/// invitation) and has been stripped of control characters and bidi overrides, collapsed and
/// bounded. A client renders it as **text, never markup**; `use_markup(false)` on GTK.
///
/// Read-only unless [`EventDetail::can_edit_invitees`] holds; an organiser's edit to it travels as
/// the [`MeetingInviteePatch`](crate::MeetingInviteePatch)
/// [`meeting_invitee_patch`](crate::meeting_invitee_patch) derives from this list.
///
/// The value derives are for Linux, the one host written in Rust: it reads this struct as written
/// and holds the roster in a view-model relm4 clones and compares. Swift, Kotlin and C# generate
/// value types of their own.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct EventAttendee {
    /// The display name, or empty when the event carried none; show `email` instead rather than
    /// inventing one.
    pub name: String,
    /// The address, normalised (lowercased, `mailto:` stripped).
    pub email: String,
    /// Whether this participant called the meeting (the `ORGANIZER`), so a client can say so.
    pub is_organizer: bool,
    /// The invitee role an organiser may edit, or `None` for the organiser and unsupported roles.
    pub role: Option<crate::MeetingInviteeRole>,
    /// How they answered. An organiser who never answered reads as `Accepted`; RFC 5546 §3.2.1
    /// has them attending by definition, and the invitation tally counts them the same way.
    pub response: crate::records_invitation::ResponseStatus,
}
