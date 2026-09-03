//! The calendar FFI records.
//!
//! Split from `records.rs`, which is at the 500-line limit.
//!
//! The geometry here is **unit-free**: day indices, wall-clock minutes, and column
//! fractions; never pixels. A client multiplies by its own hour height and column width,
//! so the same page renders correctly on a phone, a tablet, and a desktop pane without the
//! core knowing anything about any of them.
//!
//! [`CalendarPage`] deliberately flattens the view-model's nested grid: a client walks
//! `timed` and `all_day` directly rather than reaching through a wrapper it has no other
//! use for.

/// An immutable calendar agenda snapshot for a host to render.
#[derive(uniffi::Record)]
pub struct CalendarSnapshot {
    /// The events, soonest first.
    pub events: Vec<EventRow>,
    /// The IANA id of the active display zone the rows are ordered in and that the
    /// host localises each `Z`-suffixed [`EventRow::start`] against.
    pub timezone: String,
}

/// One agenda row: an event's key, title, and formatted start.
#[derive(uniffi::Record)]
pub struct EventRow {
    /// The id of the account this event belongs to: the host passes it back in
    /// [`Intent::DeleteEvent`](crate::Intent::DeleteEvent) so the delete routes to the
    /// owning account (two accounts can mint the same event key).
    pub account: String,
    /// The event's provider key.
    pub key: String,
    /// The event's title (a placeholder if empty).
    pub title: String,
    /// The start as an RFC 3339-style string: resolved instants end in `Z`, floating wall clocks
    /// do not, and all-day/unknown values are empty. The host localises an instant against the
    /// enclosing [`CalendarSnapshot::timezone`].
    pub start: String,
    /// Whether this event's owning account supports calendar writes.
    pub can_write: bool,
    /// How this account answered; `NEEDS_ACTION` is an unanswered hold, drawn dotted, and its
    /// accessibility label must say so. `DECLINED` never appears (the core hides those).
    pub participation: crate::records_invitation::ResponseStatus,
}

/// Which day a calendar week begins on: a persisted app-level setting, not a locale default.
///
/// Defaults to **Monday** (ISO-8601, and the European convention). Get it wrong and every column
/// of the grid shifts, so the user reads Tuesday's meetings under Monday's heading.
#[derive(uniffi::Enum)]
pub enum WeekStart {
    /// The week begins on Monday: the default.
    Monday,
    /// The week begins on Sunday.
    Sunday,
}

/// Whether times render on a 24-hour or a 12-hour clock; in **mail and calendar alike**.
///
/// Defaults to **24-hour**. One app must not disagree with itself: a message list reading `14:05`
/// beside a calendar reading `2 PM` is exactly what this setting exists to prevent.
#[derive(uniffi::Enum)]
pub enum TimeFormat {
    /// `14:05`: the default.
    TwentyFourHour,
    /// `2:05 PM`.
    TwelveHour,
}

/// Whether the app paints itself light or dark: a persisted app-level setting.
///
/// Defaults to **system**: the host's own light/dark setting, followed while the app runs. A client
/// resolves this itself; the core computes nothing from it (every swatch already carries both a
/// light and a dark form). Read it before the app exists with `stored_appearance`, so the first
/// frame is already in the right scheme.
// `Copy` because the Linux client consumes these as real Rust types rather than through generated
// bindings, and hands one value to both the core setter and its own repaint.
#[derive(uniffi::Enum, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    /// Follow the host's light/dark setting: the default.
    System,
    /// Light, whatever the host is set to.
    Light,
    /// Dark, whatever the host is set to.
    Dark,
}

/// The shape the calendar opens in: the last one the user was looking at.
///
/// Persisted in the core rather than in each client: it is the user's choice, and it has to survive
/// the app closing. Defaults to the whole **week**, which is not an arbitrary preference: a page
/// IS a week, so any narrower shape leaves the rest of it hanging off the side of the screen as a
/// scrollable overhang, and on a phone that overhang takes the swipe before the pager does.
#[derive(uniffi::Enum)]
pub enum CalendarLayout {
    /// One day at a time.
    Day,
    /// Three days.
    ThreeDay,
    /// Monday to Friday.
    WorkWeek,
    /// The whole week: the default.
    Week,
    /// The month grid: a different layout, not a zoom level.
    Month,
    /// The agenda list: likewise.
    Agenda,
}

/// The display settings, pulled after a `Surface::Settings` signal.
#[derive(uniffi::Record)]
pub struct DisplaySettings {
    /// Which day a calendar week begins on.
    pub week_start: WeekStart,
    /// The clock mail and calendar both render times on.
    pub time_format: TimeFormat,
    /// Whether the app paints itself light, dark, or however the host is set.
    pub appearance: Appearance,
    /// How many hours of the day the calendar grid shows at once: the horizon a pinch zooms in
    /// and out of. Already clamped by the core, so a client may divide by it without checking.
    pub visible_hours: u8,
    /// The shape the grid opens in, restored from the last session.
    pub layout: CalendarLayout,
}

/// Which edges of an event a drag moved: the shape of `Intent::MoveEvent`.
///
/// A drag is one of exactly three gestures, and naming them is what keeps the client's hit-test
/// (was the finger on the block, or on one of its edges?) from having to be re-derived as
/// arithmetic on the other side of the FFI.
#[derive(uniffi::Enum)]
pub enum EventEdge {
    /// The block itself was dragged: both edges move together, so the duration is preserved
    /// exactly.
    Whole,
    /// The top edge was dragged: the event starts earlier or later and ends where it did.
    /// Clamped by the core so a start can never pass its own end.
    Start,
    /// The bottom edge was dragged: the event ends earlier or later and starts where it did.
    /// Clamped the same way.
    End,
}

/// One day column.
#[derive(uniffi::Record)]
pub struct GridDay {
    /// The local calendar date this column shows, `YYYY-MM-DD`. The client formats the
    /// heading (weekday name, day number, week number): the core emits no localised text.
    pub date: String,
}

/// A block drawn inside the grid: one day's worth of one event.
///
/// An event crossing midnight arrives as several of these (one per day it touches) so a
/// client only ever draws a rectangle inside a single column.
#[derive(uniffi::Record)]
pub struct TimedSegment {
    /// The owning account, so an action on the block routes to it (two accounts can mint
    /// the same event key).
    pub account: String,
    /// The event's provider key.
    pub event: String,
    /// The calendar it belongs to; look its colour up in [`CalendarPage::calendars`].
    pub calendar: String,
    /// The event's title.
    pub title: String,
    /// Which day column: an index into [`CalendarPage::days`].
    pub day: u32,
    /// Wall-clock minutes from midnight where the block starts, `0..1440`.
    pub start_minutes: u32,
    /// Wall-clock minutes from midnight where it ends; always greater than
    /// [`Self::start_minutes`], so the block always has height.
    pub end_minutes: u32,
    /// Which lane of its collision cluster, `0..columns`.
    pub column: u32,
    /// How many lanes the cluster splits into: the divisor for the block's width.
    pub columns: u32,
    /// The event began before this column: draw the top edge open.
    pub continues_before: bool,
    /// The event runs past this column: draw the bottom edge open.
    pub continues_after: bool,
    /// Whether this event's owning account supports calendar writes.
    pub can_write: bool,
    /// Whether the user may **drag** this block to a new time: a writable calendar *and* an
    /// event that is theirs to reshape (their own appointment, or a meeting they organise).
    ///
    /// Strictly narrower than [`Self::can_write`]: a meeting somebody else called can sit on a
    /// writable calendar and still must not be silently re-timed. A block that reports `false`
    /// simply does not lift; there is no error to show, because the user was never offered
    /// the gesture (`docs/calendar.md` §13).
    pub can_move: bool,
    /// This occurrence's **original** start, as a wall clock in the event's own zone, or empty
    /// when the event does not recur.
    ///
    /// Opaque: pass it back verbatim as `Intent::MoveEvent`'s `occurrence` to move **this one**
    /// occurrence, or send `None` to move the whole series. Non-empty is also the signal that a
    /// drag must **ask** which the user meant, because the core will not guess; dragging one
    /// Tuesday standup is not the same as rewriting every Tuesday to eternity.
    pub occurrence_start: String,
    /// How **this account** answered, when the event is something it was invited to.
    ///
    /// `NEEDS_ACTION` is an unanswered hold: draw it with a dashed border and a hatched leading
    /// gutter. The visual is not sufficient on its own: a dashed border is invisible to a screen
    /// reader, so the accessibility label must **say** it
    /// (`a11y_invitation_awaiting_response`). `docs/calendar.md` §4, the spoken-grid rule.
    ///
    /// `DECLINED` never appears: the core hides declined events from every calendar surface. They
    /// remain in search, and the invitation email remains the way back.
    pub participation: crate::records_invitation::ResponseStatus,
}

/// A bar above the grid: an all-day or multi-day event, spanning whole day columns.
#[derive(uniffi::Record)]
pub struct AllDayBand {
    /// The owning account.
    pub account: String,
    /// The event's provider key.
    pub event: String,
    /// The calendar it belongs to.
    pub calendar: String,
    /// The event's title.
    pub title: String,
    /// The first day column the bar covers.
    pub day: u32,
    /// How many columns it covers, at least 1.
    pub days: u32,
    /// Which stacked row of the banner it sits in, `0..all_day_lanes`.
    pub lane: u32,
    /// The event began before the first shown day: draw the left edge open.
    pub continues_before: bool,
    /// The event runs past the last shown day: draw the right edge open.
    pub continues_after: bool,
    /// Whether this event's owning account supports calendar writes.
    pub can_write: bool,
    /// This occurrence's **original** start, on the same terms as
    /// [`TimedSegment::occurrence_start`](crate::TimedSegment::occurrence_start); opaque, passed
    /// back verbatim, and empty when the event does not recur. A bar is one occurrence, so an
    /// edit or a delete reached from it puts the same question a block's does.
    pub occurrence_start: String,
    /// How this account answered; `NEEDS_ACTION` is an unanswered hold, drawn dotted, and its
    /// accessibility label must say so. `DECLINED` never appears (the core hides those).
    pub participation: crate::records_invitation::ResponseStatus,
}

/// One theme's rendering of a calendar colour.
#[derive(Clone, uniffi::Record)]
pub struct Swatch {
    /// The chip's fill, `#rrggbb`.
    pub background: String,
    /// The label colour on that fill; always at least 4.5:1 against it (WCAG AA), resolved
    /// in the core so three clients cannot disagree about whether a label is readable.
    pub text: String,
    /// The chip's edge, `#rrggbb`.
    pub border: String,
}

/// A calendar colour, resolved for both themes.
#[derive(uniffi::Record)]
pub struct CalendarColor {
    /// The palette colour it resolved to, `#rrggbb`; what a colour picker shows as selected.
    pub hex: String,
    /// How to draw it in a light theme.
    pub light: Swatch,
    /// How to draw it in a dark theme.
    pub dark: Swatch,
}

/// One calendar: the row a calendar manager lists, and the colour key the grid paints from.
#[derive(uniffi::Record)]
pub struct CalendarRow {
    /// The owning account's id.
    pub account: String,
    /// The calendar's provider key, unique within its account.
    pub id: String,
    /// The display name.
    pub name: String,
    /// The resolved colour, for both themes.
    pub color: CalendarColor,
    /// Whether its events are currently drawn.
    pub visible: bool,
    /// Whether this account's calendar provider supports writes.
    pub can_write: bool,
    /// Whether a new event lands here unless the user picks another calendar.
    ///
    /// Already resolved against what exists: the stored choice while it is present and writable,
    /// otherwise the first writable calendar. Exactly one row carries it whenever any calendar can
    /// be written to, so a client reads it instead of keeping a fallback rule of its own.
    pub is_default: bool,
}

/// One page of the calendar, laid out and ready to draw.
#[derive(uniffi::Record)]
pub struct CalendarPage {
    /// The day columns, left to right.
    pub days: Vec<GridDay>,
    /// The blocks inside the grid.
    pub timed: Vec<TimedSegment>,
    /// The bars above it.
    pub all_day: Vec<AllDayBand>,
    /// How many stacked rows the banner needs; size it from this and reserve nothing at
    /// all when it is zero.
    pub all_day_lanes: u32,
    /// The IANA display zone the layout was computed in.
    pub timezone: String,
    /// Every calendar across every account, for the manager and the colour lookup.
    pub calendars: Vec<CalendarRow>,
    /// Whether the engine has expanded far enough to answer for this page.
    ///
    /// **`false` does not mean "no events".** It means we have not looked yet. Show a
    /// loading state; rendering an empty week here is a confident lie that looks exactly
    /// like a real answer.
    pub is_materialized: bool,
}

/// One event on one day of the month grid.
#[derive(uniffi::Record)]
pub struct MonthChip {
    /// The owning account.
    pub account: String,
    /// The event's provider key.
    pub event: String,
    /// The calendar it belongs to; look its colour up in [`MonthPage::calendars`].
    pub calendar: String,
    /// The event's title.
    pub title: String,
    /// Whether it covers the whole day (a filled bar, rather than a dot and a time).
    pub all_day: bool,
    /// Wall-clock minutes from midnight it starts at. `0` for an all-day event, and `0` on any day
    /// a multi-day event merely runs *through*; it did not start again that morning.
    pub start_minutes: u32,
    /// Whether this event's owning account supports calendar writes.
    pub can_write: bool,
    /// This occurrence's **original** start, on the same terms as
    /// [`TimedSegment::occurrence_start`](crate::TimedSegment::occurrence_start); opaque, passed
    /// back verbatim, and empty when the event does not recur. A chip is one occurrence, so an
    /// edit or a delete reached from the month grid puts the same question a block's does.
    pub occurrence_start: String,
    /// How this account answered; `NEEDS_ACTION` is an unanswered hold, drawn dotted, and its
    /// accessibility label must say so. `DECLINED` never appears (the core hides those).
    pub participation: crate::records_invitation::ResponseStatus,
}

/// One day cell of the month grid.
#[derive(uniffi::Record)]
pub struct MonthCell {
    /// The date this cell shows, `YYYY-MM-DD`.
    pub date: String,
    /// Whether this date is in the **anchored month**, rather than the leading/trailing days of
    /// its neighbours. Dim the others; otherwise the 1st of next month looks like part of
    /// this one and the user taps into the wrong month without noticing.
    pub in_month: bool,
    /// Everything on this day: all-day events first, then timed ones by start.
    ///
    /// **Not truncated.** How many chips fit is a question of how tall a cell is on *this* screen,
    /// so the client shows what it can and counts the rest: the core does not guess at a phone's
    /// row height.
    pub chips: Vec<MonthChip>,
}

/// A month, laid out and ready to draw.
#[derive(uniffi::Record)]
pub struct MonthPage {
    /// Exactly 42 cells, row-major: six weeks of seven days.
    ///
    /// Always six weeks, even when five would do: a grid that changes height as you page makes
    /// the whole screen jump.
    pub cells: Vec<MonthCell>,
    /// The IANA display zone the layout was computed in.
    pub timezone: String,
    /// Every calendar across every account, for the manager and the colour lookup.
    pub calendars: Vec<CalendarRow>,
    /// Whether the engine has expanded far enough to answer for this month.
    ///
    /// **`false` does not mean "no events"**; see [`CalendarPage::is_materialized`].
    pub is_materialized: bool,
}

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
    /// The location, if any.
    pub location: Option<String>,
    /// The notes/description, if any.
    pub notes: Option<String>,
    /// Minutes before the start of the first reminder, or `None`. Display-only in v1.
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
    /// The occurrence this detail describes, as the token that named it; empty when it
    /// describes the **series**, which is what an agenda row and a one-off event always do.
    ///
    /// **This, not the token the client sent, is what a scope question is asked from.** It
    /// comes back empty when the core could not resolve what was sent (the series changed
    /// underneath the view it was drawn in), and the times above are then the series': so a
    /// client that reads this can never offer *This event* against another occurrence's times.
    ///
    /// Hand it straight back as `Intent::UpdateEvent`/`DeleteEvent`'s `occurrence`.
    pub occurrence_start: String,
    /// Everyone on the event, organiser first; empty for an appointment nobody was invited to.
    pub attendees: Vec<EventAttendee>,
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
/// Attendees are **read-only** across the product: changing them means sending iTIP updates, a
/// separate feature. An editor shows this list; it does not offer to edit it.
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
    /// How they answered. An organiser who never answered reads as `Accepted`; RFC 5546 §3.2.1
    /// has them attending by definition, and the invitation tally counts them the same way.
    pub response: crate::records_invitation::ResponseStatus,
}
