//! The meeting-invitation card shown above a message body.
//!
//! An invitation is a message that carries an iTIP scheduling object (iMIP, RFC 6047). This is
//! what a reading view draws for one: who called the meeting, when it is, how the other
//! attendees have answered, what it collides with in the user's own diary, and, when a reply
//! is actually owed: the Accept / Tentative / Decline choice.
//!
//! # What the core decides and what the client draws
//!
//! The core decides **whether there is a card at all**, which affordances it carries
//! ([`InvitationKind`]), and every number on it. The client decides how it looks and formats
//! the times: the core is tzdata-free for display purposes and emits **UTC instants**, exactly
//! as `docs/timestamps.md` requires of every other timestamp in the product.
//!
//! # Every text field here is attacker-controlled
//!
//! `SUMMARY`, `LOCATION`, `DESCRIPTION` and the organiser's display name come from whoever sent
//! the mail. They are **plain text, never markup**: the core strips control characters, collapses
//! whitespace and truncates, and a client must render them as text. On GTK that means
//! `use_markup(false)` on any row showing them: a localised ampersand or a subject containing
//! `<b>` is otherwise parsed as Pango markup (`AGENTS.md`, and `docs/rendering-security.md`).

use crate::calendar::grid::TimeGrid;

/// What a reading view may offer for an invitation: the outcome of the two-condition RSVP gate.
///
/// The gate (`docs/invitations.md`): a **scheduling `METHOD`** *and* an `ATTENDEE` matching one
/// of the account's own addresses. Both are required, because either alone is wrong; a
/// `PUBLISH` that happens to list you is an informational copy with no organiser awaiting a
/// reply, and a `REQUEST` addressed to somebody else is somebody else's meeting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum InvitationKind {
    /// A reply is owed and we are the one who owes it: show Accept / Tentative / Decline.
    #[default]
    Rsvp,
    /// The organiser cancelled this meeting. No reply is possible; offer to remove the
    /// (already tentative or accepted) event from the calendar so a dead hold cannot linger.
    Cancelled,
    /// Show the details, offer no response: a `REQUEST` where we are not an attendee (a
    /// forwarded invitation), or a `CANCEL` for a meeting that was never ours.
    Informational,
    /// The organiser has since sent a newer version of this invitation: the calendar holds a
    /// higher `SEQUENCE` for the same `UID` (RFC 5546 §2.1.5). Show the details, say the mail
    /// is out of date, and offer **no** response.
    ///
    /// Answering the stale copy is not merely useless, it is wrong twice over: the times on
    /// screen are not the meeting's any more, so the user would be agreeing to a slot they
    /// were never shown, and the reply would carry the old `SEQUENCE`, which an organiser's
    /// scheduler may discard as answering a revision it has already superseded.
    Superseded,
}

/// How this account has answered the invitation so far.
///
/// Mirrors the engine's `ParticipationStatus`, narrowed to the states a card renders.
/// `NeedsAction` is the unanswered case and the one the grid draws as a dotted hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ResponseStatus {
    /// Not answered yet.
    #[default]
    NeedsAction,
    /// Accepted.
    Accepted,
    /// Declined.
    Declined,
    /// Tentatively accepted.
    Tentative,
    /// Delegated to somebody else.
    Delegated,
}

/// How the invitation's attendees have answered, for the "3 accepted, 1 declined" line.
///
/// Counts, never a roster: a large meeting's attendee list is long, is attacker-controlled,
/// and is other people's addresses: so the card summarises it and the detail view is where a
/// full list would belong. [`Self::total`] counts every attendee including this account, so a
/// client can render "of N".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AttendeeTally {
    /// Every attendee on the invitation, including this account.
    pub total: u32,
    /// How many have accepted.
    pub accepted: u32,
    /// How many have declined.
    pub declined: u32,
    /// How many answered tentatively.
    pub tentative: u32,
    /// How many have not answered.
    pub needs_action: u32,
}

/// The invitation card for the open message, or `None` on a message that carries no iTIP
/// object (see [`crate::ReadingSnapshot::invitation`]).
// Four independent facts about a meeting; whether the notes were cut, whether it is all-day,
// whether it repeats, and whether this account can answer. None of them constrains another, and
// each crosses FFI as its own field for a client to branch on, so folding them into a state enum
// would obscure them rather than clarify anything.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InvitationCard {
    /// Which affordances the card offers.
    pub kind: InvitationKind,
    /// The organiser as `Name <email>`, or bare `email` when the invitation carried no name.
    /// **Attacker-controlled plain text.**
    pub organizer: String,
    /// The meeting's title. **Attacker-controlled plain text**; empty when the invitation had
    /// none, which a client renders as its own "(no title)" rather than inventing one here.
    pub summary: String,
    /// Where it is. **Attacker-controlled plain text**; empty when absent.
    pub location: String,
    /// The invitation's notes, truncated. **Attacker-controlled plain text**; empty when
    /// absent. Truncation is not cosmetic; Gmail's description is a wall of `-::~:~::~`
    /// filler that would otherwise push the whole message body off screen.
    pub description: String,
    /// Whether [`Self::description`] was cut short, so a client can say so rather than
    /// implying the text simply ends there.
    pub description_truncated: bool,
    /// The meeting's start, as a UTC RFC 3339 instant. The host localises it
    /// (`docs/timestamps.md`); the core ships no display tzdata.
    pub starts_at: String,
    /// The meeting's end, as a UTC RFC 3339 instant.
    pub ends_at: String,
    /// Whether this is an all-day (date-only) event, so a client shows a date rather than a
    /// time range.
    pub all_day: bool,
    /// Whether the meeting repeats, so a client can say "and the rest of the series". The
    /// card's times are the **first** occurrence's.
    pub recurring: bool,
    /// How this account has answered so far.
    pub my_response: ResponseStatus,
    /// How everyone has answered.
    pub attendees: AttendeeTally,
    /// How many *other* things the user already has in this meeting's window.
    ///
    /// Excludes the invitation's own event (it is usually already on the calendar as a
    /// tentative hold; counting it would report every invitation as clashing with itself) and
    /// excludes other unanswered holds, because two unanswered invitations are not yet a
    /// conflict: the sender of an Outlook invitation explicitly asks for
    /// `X-MICROSOFT-CDO-BUSYSTATUS:TENTATIVE` until answered.
    ///
    /// A client **must state this number in words** next to the preview; `docs/calendar.md`
    /// §4: nothing is hidden without saying so, and a grid the user has to read carefully is
    /// not a disclosure.
    ///
    /// **Meaningless unless [`Self::conflicts_known`] is `true`.**
    pub conflict_count: u32,
    /// Whether the calendar could actually be read over this meeting's window.
    ///
    /// `false` means **"we have not looked"**, never "nothing is there": the engine had not
    /// expanded the calendar over the meeting's day yet (a cold start syncs mail before
    /// calendars, so an invitation opened straight away hits this), or the diary read failed. A
    /// client must then say so and **must not** print [`Self::conflict_count`], which is zero
    /// only because nothing was counted.
    ///
    /// The same distinction the grid draws with `is_materialized`, from the same window, for the
    /// same reason: "Nothing else in your calendar then", stated over a calendar nobody has read,
    /// is a lie that looks exactly like a real answer.
    pub conflicts_known: bool,
    /// A one-day grid of the meeting's day, so the user can see the clash rather than being
    /// told about it. Built by the same `calendar::grid::build` every calendar surface uses,
    /// so it carries the same unit-free geometry and a client only multiplies
    /// (`docs/calendar.md` §1).
    pub preview: TimeGrid,
    /// Whether this account can actually deliver a response: the account's calendar provider
    /// supports RSVP writes.
    ///
    /// `false` means the buttons must be **absent with an explanation**, never present and
    /// disabled: on an account whose server does not reply for us, a button that appears to
    /// work but tells nobody is worse than no button at all.
    pub can_respond: bool,
    /// Whether a note to the organiser can actually be carried; Outlook's "optional message".
    ///
    /// `false` on CalDAV (iCalendar has no per-attendee comment) and on JMAP (the field exists
    /// in RFC 8984 but no server we run has been seen to relay it). A client offers the field
    /// only when this is `true`: a note that silently goes nowhere is worse than one never
    /// offered, and the core refuses such a write rather than dropping it.
    pub can_comment: bool,
    /// Whether the user may answer **without** telling the organiser; Outlook's "Email
    /// organiser" tick.
    ///
    /// `false` on every server-scheduled transport (CalDAV auto-schedule, JMAP), where the
    /// reply leaves the moment the participation status changes and no client can stop it. A
    /// tick that emails them anyway is worse than no tick.
    pub can_choose_notify: bool,
}
