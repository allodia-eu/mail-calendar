//! The FFI mirror of the meeting-invitation card, and its conversion from the view-model.
//!
//! Its own file because `records.rs` is at the 500-line limit, and because the card is one
//! coherent surface: the record, its two enums, the tally, the preview grid, and the `From`
//! impls that map them all belong together.
//!
//! **Every text field here is attacker-controlled.** A client renders them as **text**, never
//! markup; on GTK that means `use_markup(false)`, the trap `AGENTS.md` records. See
//! `docs/rendering-security.md`.

use mailcal_app::InvitationResponse as AppInvitationResponse;
use mailcal_viewmodel::{
    AttendeeTally as AppAttendeeTally, InvitationCard as AppInvitationCard,
    InvitationKind as AppInvitationKind, ResponseStatus as AppResponseStatus,
    calendar::grid::TimeGrid as AppTimeGrid,
};

use crate::{
    protocol::InvitationResponse,
    records_calendar::{AllDayBand, GridDay, TimedSegment},
};

/// What the invitation card offers: the outcome of the two-condition RSVP gate.
///
/// `Copy` and the value derives are for Linux, as on [`ResponseStatus`]: a fieldless answer is a
/// value, not something to borrow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum InvitationKind {
    /// A reply is owed and this account owes it: show Accept / Tentative / Decline.
    Rsvp,
    /// The organiser cancelled. No reply is possible; offer to clear the hold from the calendar.
    Cancelled,
    /// Show the details, offer no response: a forwarded invitation, or a cancellation for a
    /// meeting that was never ours.
    Informational,
    /// The organiser has since sent a newer version: the calendar holds a higher `SEQUENCE` for
    /// this `UID`. Show the details, **say the mail is out of date**, and offer no response;
    /// its times are no longer the meeting's.
    Superseded,
}

/// How this account has answered the invitation so far.
///
/// `Copy` and the value derives are for Linux, as on [`EventAttendee`](crate::EventAttendee): a
/// fieldless answer is a value, not something to borrow.
#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum ResponseStatus {
    /// Not answered yet: the state the calendar grid draws as a dotted hold.
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
/// Counts, never a roster: the addresses are other people's, and are attacker-controlled.
#[derive(uniffi::Record)]
pub struct AttendeeTally {
    /// Every attendee including this account; render as "of N".
    pub total: u32,
    /// How many accepted.
    pub accepted: u32,
    /// How many declined.
    pub declined: u32,
    /// How many answered tentatively.
    pub tentative: u32,
    /// How many have not answered (delegated attendees count here, so the four buckets sum to
    /// [`Self::total`]).
    pub needs_action: u32,
}

/// The meeting-day preview grid: the user's own day, so a clash can be *seen*.
///
/// The same geometry as a full calendar page (laid out by the same solver) so a client reuses
/// its grid renderer and only multiplies (`docs/calendar.md` §1). It carries no calendar list and
/// no materialization flag: it is one already-loaded day, not a page to page through.
#[derive(uniffi::Record)]
pub struct InvitationPreview {
    /// The single day column.
    pub days: Vec<GridDay>,
    /// The blocks inside the grid.
    pub timed: Vec<TimedSegment>,
    /// The bars above it.
    pub all_day: Vec<AllDayBand>,
    /// How many stacked rows the banner needs; reserve nothing when zero.
    pub all_day_lanes: u32,
    /// The IANA display zone the layout was computed in.
    pub timezone: String,
}

/// The meeting-invitation card for the open message.
///
/// A host draws it **above** the message body. Times are UTC instants for the host to localize
/// (`docs/timestamps.md`); the core ships no display tzdata.
#[derive(uniffi::Record)]
pub struct InvitationCard {
    /// Which affordances the card offers.
    pub kind: InvitationKind,
    /// The organiser as `Name <email>`, or bare `email`. **Attacker-controlled text.**
    pub organizer: String,
    /// The meeting's title. **Attacker-controlled text.** Empty when the invitation had none;
    /// the client supplies its own localised "(no title)" rather than the core inventing one.
    pub summary: String,
    /// Where it is. **Attacker-controlled text.** Empty when absent.
    pub location: String,
    /// The notes, truncated. **Attacker-controlled text.** Empty when absent.
    pub description: String,
    /// Whether [`Self::description`] was cut short; say so rather than implying it ends there.
    pub description_truncated: bool,
    /// The start, as a UTC RFC 3339 instant. Localise it on the host.
    pub starts_at: String,
    /// The end, as a UTC RFC 3339 instant.
    pub ends_at: String,
    /// Whether this is an all-day event; show a date, not a time range.
    pub all_day: bool,
    /// Whether the meeting repeats. The card's times are the **first** occurrence's.
    pub recurring: bool,
    /// How this account has answered so far.
    pub my_response: ResponseStatus,
    /// How everyone has answered.
    pub attendees: AttendeeTally,
    /// How many *other* commitments the user already has in this window.
    ///
    /// Excludes the invitation's own tentative hold and other unanswered holds. A client
    /// **must state this in words** beside the preview; `docs/calendar.md` §4: nothing is
    /// hidden without saying so, and a grid the user must read carefully is not a disclosure.
    ///
    /// **Meaningless unless [`Self::conflicts_known`].**
    pub conflict_count: u32,
    /// Whether the calendar could be read over this meeting's window at all.
    ///
    /// `false` means **"we have not looked"**: the engine had not expanded the calendar this far
    /// (mail syncs before calendars, so an invitation opened on launch hits it), or the read
    /// failed. Say so; do **not** print [`Self::conflict_count`], which is then zero only because
    /// nothing was counted. Printing "nothing else in your calendar then" over an unread calendar
    /// is the confident lie `docs/calendar.md` §4 forbids: the same rule as the grid's
    /// `is_materialized`.
    pub conflicts_known: bool,
    /// The meeting day, laid out.
    pub preview: InvitationPreview,
    /// Whether this account can actually deliver a response.
    ///
    /// `false` means the buttons are **absent with an explanation**, never present and
    /// disabled: a button that appears to work but tells nobody is worse than no button.
    pub can_respond: bool,
    /// Whether a note to the organiser has anywhere to go on this account's transport;
    /// Outlook's "optional message".
    ///
    /// `false` on CalDAV and JMAP. Offer the field only when this is `true`: the core
    /// **refuses** a note a transport cannot carry rather than dropping it, so sending one
    /// unasked fails the whole answer instead of quietly losing the text.
    pub can_comment: bool,
    /// Whether the user may answer **without** telling the organiser; Outlook's "Email
    /// organiser" tick.
    ///
    /// `false` on every server-scheduled transport, where the reply leaves the moment the
    /// status changes and no client can stop it. Offer the toggle only when this is `true`; a
    /// tick that emails them anyway is worse than no tick.
    pub can_choose_notify: bool,
}

impl From<AppInvitationKind> for InvitationKind {
    fn from(kind: AppInvitationKind) -> Self {
        match kind {
            AppInvitationKind::Rsvp => Self::Rsvp,
            AppInvitationKind::Cancelled => Self::Cancelled,
            AppInvitationKind::Informational => Self::Informational,
            AppInvitationKind::Superseded => Self::Superseded,
        }
    }
}

impl From<AppResponseStatus> for ResponseStatus {
    fn from(status: AppResponseStatus) -> Self {
        match status {
            AppResponseStatus::NeedsAction => Self::NeedsAction,
            AppResponseStatus::Accepted => Self::Accepted,
            AppResponseStatus::Declined => Self::Declined,
            AppResponseStatus::Tentative => Self::Tentative,
            AppResponseStatus::Delegated => Self::Delegated,
        }
    }
}

impl From<AppAttendeeTally> for AttendeeTally {
    fn from(tally: AppAttendeeTally) -> Self {
        Self {
            total: tally.total,
            accepted: tally.accepted,
            declined: tally.declined,
            tentative: tally.tentative,
            needs_action: tally.needs_action,
        }
    }
}

impl From<AppTimeGrid> for InvitationPreview {
    fn from(grid: AppTimeGrid) -> Self {
        Self {
            days: grid.days.into_iter().map(Into::into).collect(),
            timed: grid.timed.into_iter().map(Into::into).collect(),
            all_day: grid.all_day.into_iter().map(Into::into).collect(),
            all_day_lanes: grid.all_day_lanes,
            timezone: grid.timezone,
        }
    }
}

impl From<AppInvitationCard> for InvitationCard {
    fn from(card: AppInvitationCard) -> Self {
        Self {
            kind: card.kind.into(),
            organizer: card.organizer,
            summary: card.summary,
            location: card.location,
            description: card.description,
            description_truncated: card.description_truncated,
            starts_at: card.starts_at,
            ends_at: card.ends_at,
            all_day: card.all_day,
            recurring: card.recurring,
            my_response: card.my_response.into(),
            attendees: card.attendees.into(),
            conflict_count: card.conflict_count,
            conflicts_known: card.conflicts_known,
            preview: card.preview.into(),
            can_respond: card.can_respond,
            can_comment: card.can_comment,
            can_choose_notify: card.can_choose_notify,
        }
    }
}

/// The question a host asks when a calendar server that promised to tell the organizer
/// reported that it could not (RFC 6638 §3.2.9); offering to email them instead.
///
/// Raised **after** the answer is stored, because that is when the server says so. So the
/// modal's first job is to be clear that the RSVP itself is fine: what failed is the message
/// to the organiser, and the user is being asked whether we may send it as an ordinary email
/// from their account.
///
/// Answered with `Intent::AnswerReplyPrompt`. There is no id to pass back: the core holds the
/// question and clears it the moment it is answered, so a double-tap cannot send two replies.
#[derive(uniffi::Record)]
pub struct ReplyPrompt {
    /// The account whose calendar server failed: the one a remembered choice applies to.
    pub account: String,
    /// The meeting's title, so the sentence names what is being confirmed.
    pub summary: String,
    /// The address the email would go to. A user authorising mail sent on their behalf is
    /// entitled to see the recipient, so a host **shows this** rather than saying "the
    /// organiser".
    pub organizer: String,
    /// The answer that was given. Carried because the reply's **subject** is the client's to
    /// compose (`invitation_reply_subject_*`), and by the time this question is asked the
    /// buttons that were pressed are long gone from the client's own state: a host that had
    /// to remember which one would be a second source of truth for a fact the core already
    /// holds.
    pub response: InvitationResponse,
    /// The RFC 6638 status the server reported (`5.2`, `3.7`, …). **Not for the prompt**; the
    /// copy a user reads is plain language. Carried for the diagnostics screen and support.
    pub status_code: String,
}

impl From<mailcal_app::ReplyPrompt> for ReplyPrompt {
    fn from(prompt: mailcal_app::ReplyPrompt) -> Self {
        Self {
            account: prompt.account.to_string(),
            summary: prompt.summary,
            organizer: prompt.organizer,
            response: match prompt.response {
                AppInvitationResponse::Accept => InvitationResponse::Accept,
                AppInvitationResponse::Tentative => InvitationResponse::Tentative,
                AppInvitationResponse::Decline => InvitationResponse::Decline,
            },
            status_code: prompt.status_code,
        }
    }
}
