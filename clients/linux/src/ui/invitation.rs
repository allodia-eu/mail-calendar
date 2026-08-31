//! The meeting-invitation card: its rules, its widget, its preview grid, and the question raised
//! when a calendar server could not pass the answer on.
//!
//! Everything the card states is the core's (`docs/invitations.md`): whether there is a card at
//! all, the organiser line, the attendee tally, the conflict count and the preview's geometry.
//! This module localises and arranges, so this client and the next cannot disagree about whether a
//! meeting clashes.
//!
//! This file is the rules: the Linux twin of `InvitationFormat.swift` / `.kt` / `.cs`, function
//! for function. It returns localised strings the way Swift and Kotlin do rather than the *choice*
//! Windows returns, because the reason for that split does not exist here: `crate::l10n` is a
//! plain generated Rust module, so a rule phrased as a string is still a rule a unit test reaches.
//!
//! **Security (Gate 8, `docs/rendering-security.md`).** The summary, location, description and
//! organiser name are attacker-controlled sender content that reaches the screen without passing
//! the HTML sanitiser or a web view. On GTK that makes `use_markup(false)` mandatory: a
//! libadwaita row parses its title *and* subtitle as Pango markup by default, so an unescaped
//! ampersand renders the row blank and a markup-shaped subject arrives styled.

pub(super) mod card;
mod preview;
mod prompt;

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) mod widget_tests;

pub(super) use card::{InvitationAnswer, InvitationCardView};
use mailcal_bindings::{
    AttendeeTally, CalendarWriteStatus, InvitationKind, InvitationResponse, ResponseStatus,
};
pub(super) use prompt::ReplyPromptDialog;

use super::calendar::date::{clock, instant_in, long_date};
use crate::l10n;

/// A span of wall-clock minutes from midnight; the unit the core's grid solver emits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MinuteSpan {
    pub(super) start: u32,
    pub(super) end: u32,
}

/// The band of whole hours the meeting-day preview draws: [`Self::first`] inclusive,
/// [`Self::last`] **exclusive**.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HourSpan {
    pub(super) first: u32,
    pub(super) last: u32,
}

impl HourSpan {
    /// How many hours the band covers; what the preview divides its height by.
    pub(super) const fn count(self) -> u32 {
        self.last.saturating_sub(self.first)
    }
}

/// The floor on the preview's hour band: see [`preview_span`].
const MINIMUM_PREVIEW_HOURS: u32 = 6;

/// Hours in a day, the ceiling every band is clamped to.
const HOURS_IN_DAY: u32 = 24;

/// Roughly the height one hour label needs before two of them collide.
const LABEL_HEIGHT: f64 = 18.0;

/// The height one hour wants: room for a 60-minute block's title plus its insets.
const IDEAL_PREVIEW_HOUR_HEIGHT: f64 = 20.0;

/// What the preview normally is; short enough that the message body is still on screen.
const ORDINARY_PREVIEW_HEIGHT: f64 = 132.0;

/// The ceiling, for a band a long booking forced wide. Taller than this is not a preview.
const MAXIMUM_PREVIEW_HEIGHT: f64 = 240.0;

/// Below this a preview block gets no title rather than one sliced through the middle; the same
/// rule the full grid applies at a low zoom.
pub(super) const MINIMUM_TITLED_HEIGHT: f64 = 12.0;

/// The card's heading: what this message is, before any detail.
pub(super) fn title(kind: InvitationKind) -> &'static str {
    match kind {
        InvitationKind::Cancelled => l10n::invitation_cancelled_title(),
        InvitationKind::Informational => l10n::invitation_informational_title(),
        InvitationKind::Superseded => l10n::invitation_superseded_title(),
        InvitationKind::Rsvp => l10n::invitation_title(),
    }
}

/// The sentence under the heading saying why no answer is offered, or `None` where none is owed.
///
/// A superseded card still looks answerable, so without this it reads as broken rather than out of
/// date.
pub(super) fn notice(kind: InvitationKind) -> Option<&'static str> {
    matches!(kind, InvitationKind::Superseded).then(l10n::invitation_superseded)
}

/// How this account has answered so far, in words.
pub(super) fn response(status: ResponseStatus) -> &'static str {
    match status {
        ResponseStatus::Accepted => l10n::invitation_response_accepted(),
        ResponseStatus::Declined => l10n::invitation_response_declined(),
        ResponseStatus::Tentative => l10n::invitation_response_tentative(),
        ResponseStatus::Delegated => l10n::invitation_response_delegated(),
        ResponseStatus::NeedsAction => l10n::invitation_response_needs_action(),
    }
}

/// The subject line for the reply the core emails to the organiser on an account whose calendar
/// server does no scheduling of its own.
///
/// Composed here rather than in the core because the core carries no locale (`AGENTS.md` →
/// "Localisation is client-side") and this is copy a stranger reads in their inbox.
pub(super) fn reply_subject(response: InvitationResponse, summary: &str) -> String {
    let summary = if summary.trim().is_empty() {
        l10n::invitation_no_title()
    } else {
        summary
    };
    match response {
        InvitationResponse::Accept => l10n::invitation_reply_subject_accepted(summary),
        InvitationResponse::Tentative => l10n::invitation_reply_subject_tentative(summary),
        InvitationResponse::Decline => l10n::invitation_reply_subject_declined(summary),
    }
}

/// What else is in the user's calendar over the meeting's window, **in words**.
///
/// `known == false` is **not** zero: the core could not read the calendar over this window at all,
/// and the count must not be printed (`docs/calendar.md` §4). On a cold start mail syncs before
/// calendars, so an invitation opened straight away lands exactly there.
pub(super) fn conflicts(count: u32, known: bool) -> String {
    if !known {
        return l10n::invitation_conflicts_unknown().to_owned();
    }
    match count {
        0 => l10n::invitation_conflicts_none().to_owned(),
        1 => l10n::invitation_conflicts_one().to_owned(),
        many => l10n::invitation_conflicts(i64::from(many)),
    }
}

/// The attendee tally as the single line the card shows, or empty when there is none.
///
/// Counts only, never a roster: the addresses belong to other people and are attacker-controlled.
/// Every non-zero bucket earns a phrase, because the four sum to the total and a line that leaves
/// one out reads as arithmetic that does not add up. Each bucket has a `_one` key because the
/// catalog has no plural machinery and Dutch needs a different verb at one; English reads fine
/// either way, which is why this was invisible until the card was read in Dutch.
pub(super) fn attendees(tally: &AttendeeTally) -> String {
    if tally.total == 0 {
        return String::new();
    }
    if tally.total == 1 {
        return l10n::invitation_attendees_one().to_owned();
    }
    let mut phrases = vec![l10n::invitation_attendees(
        &tally.accepted.to_string(),
        &tally.total.to_string(),
    )];
    if tally.tentative == 1 {
        phrases.push(l10n::invitation_attendees_tentative_one().to_owned());
    } else if tally.tentative > 1 {
        phrases.push(l10n::invitation_attendees_tentative(i64::from(
            tally.tentative,
        )));
    }
    if tally.declined == 1 {
        phrases.push(l10n::invitation_attendees_declined_one().to_owned());
    } else if tally.declined > 1 {
        phrases.push(l10n::invitation_attendees_declined(i64::from(
            tally.declined,
        )));
    }
    if tally.needs_action == 1 {
        phrases.push(l10n::invitation_attendees_pending_one().to_owned());
    } else if tally.needs_action > 1 {
        phrases.push(l10n::invitation_attendees_pending(i64::from(
            tally.needs_action,
        )));
    }
    phrases.join(" · ")
}

/// What the respond row says about the write currently settling, or `None` when there is nothing
/// to say.
///
/// `Saved` and `Idle` both say nothing on purpose: by then the card has been rebuilt from the
/// calendar and already shows the new answer, so a second "answer sent" is noise. `Failed` is the
/// one state that must never be silent; the card would otherwise sit showing the previous answer
/// while the organiser heard nothing, which is the failure this feature exists to prevent.
pub(super) fn write_line(status: CalendarWriteStatus) -> Option<&'static str> {
    match status {
        CalendarWriteStatus::Saving => Some(l10n::invitation_sending()),
        CalendarWriteStatus::Failed => Some(l10n::invitation_failed()),
        CalendarWriteStatus::Idle | CalendarWriteStatus::Saved => None,
    }
}

/// The meeting's "when", localised in `zone`.
///
/// All-day shows the inclusive day(s); the stored end is exclusive, so a one-day event whose end
/// is the next midnight must read as one date, not two. A timed meeting collapses the date when
/// start and end share one. The clock honours the app's 12/24-hour **setting** rather than the
/// locale's default, so mail and calendar cannot disagree with each other.
pub(super) fn when(
    starts_at: &str,
    ends_at: &str,
    all_day: bool,
    zone: &str,
    use_24_hour: bool,
) -> String {
    let Some((start_date, start_minutes)) = instant_in(starts_at, zone) else {
        return String::new();
    };
    let (end_date, end_minutes) = instant_in(ends_at, zone).unwrap_or((start_date, start_minutes));
    if all_day {
        // The stored end is EXCLUSIVE. Naming it would tell the user a one-day event lasts two.
        let last = if end_minutes == 0 {
            end_date.previous_day().unwrap_or(end_date)
        } else {
            end_date
        };
        if last <= start_date {
            return long_date(start_date);
        }
        return format!("{} – {}", long_date(start_date), long_date(last));
    }
    let from = clock(start_minutes, use_24_hour);
    let to = clock(end_minutes, use_24_hour);
    if start_date == end_date {
        format!("{}, {from} – {to}", long_date(start_date))
    } else {
        format!(
            "{} {from} – {} {to}",
            long_date(start_date),
            long_date(end_date)
        )
    }
}

/// The meeting's UTC instants as wall-clock minutes from midnight in `zone`.
///
/// Falls back to a one-hour span at midnight for an instant that will not parse: the preview then
/// draws the day it was given rather than nothing at all.
pub(super) fn meeting_minute_span(starts_at: &str, ends_at: &str, zone: &str) -> MinuteSpan {
    let Some((start_date, start)) = instant_in(starts_at, zone) else {
        return MinuteSpan { start: 0, end: 60 };
    };
    let (end_date, end) = instant_in(ends_at, zone).unwrap_or((start_date, start));
    // An end past midnight, or on a later day, belongs to the end of this day's grid.
    let end = if end_date == start_date {
        end
    } else {
        HOURS_IN_DAY * 60
    };
    MinuteSpan {
        start,
        end: end.max(start + 1),
    }
}

/// The hour band the meeting-day preview draws: **the meeting, everything that overlaps it, and an
/// hour of air**.
///
/// Never narrower than [`MINIMUM_PREVIEW_HOURS`], so a 30-minute meeting on an empty afternoon
/// still has context around it.
///
/// **Nothing the card counts can fall outside this.** A conflict is by definition an event
/// overlapping the meeting's own window, so every one of them widens the band and its *whole*
/// extent is inside it; a long booking that starts hours earlier drags the band back with it
/// rather than being cut off at the top edge with its title off-screen. What is left out is the
/// rest of the day, which the card states in words above the grid and which the disclosure label
/// names ("Around this meeting", never "your calendar that day").
pub(super) fn preview_span(meeting: MinuteSpan, others: &[MinuteSpan]) -> HourSpan {
    let mut earliest = meeting.start;
    let mut latest = meeting.end;
    for other in others {
        // Half-open on both sides, exactly as `count_conflicts` overlaps in the core: back-to-back
        // is not a clash, so an event ending as the meeting starts does not widen the band.
        if other.start >= meeting.end || meeting.start >= other.end {
            continue;
        }
        earliest = earliest.min(other.start);
        latest = latest.max(other.end);
    }
    let mut first = (earliest / 60).saturating_sub(1);
    // Ceil, so a block ending at 09:15 keeps the whole 09:00 hour, then pad.
    let mut last = (latest.div_ceil(60) + 1).min(HOURS_IN_DAY);
    // Alternating, later hour first, so the meeting sits near the middle of the band rather than
    // pinned to its top; the hours after a meeting are the more interesting of the two.
    let mut grow_after = true;
    while last - first < MINIMUM_PREVIEW_HOURS && (first > 0 || last < HOURS_IN_DAY) {
        if grow_after && last < HOURS_IN_DAY {
            last += 1;
        } else if first > 0 {
            first -= 1;
        } else {
            last += 1;
        }
        grow_after = !grow_after;
    }
    HourSpan { first, last }
}

/// How many hours apart the preview's labelled gridlines sit, given the height one hour gets.
///
/// A squeezed span leaves no room to label every hour; two labels overlapping is worse than
/// three-hourly ones; so the stride is derived from the height rather than fixed. Never zero: a
/// zero stride is a division by zero in the ruler.
pub(super) fn preview_stride(hour_height: f64) -> u32 {
    if hour_height <= 0.0 {
        return 1;
    }
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a positive ceiling of two small positive constants cannot leave u32"
    )]
    let stride = (LABEL_HEIGHT / hour_height).ceil() as u32;
    stride.max(1)
}

/// How tall the meeting-day preview draws, for a band of `hours`.
///
/// **Normally just [`ORDINARY_PREVIEW_HEIGHT`]**: the band is narrow ([`preview_span`] shows the
/// meeting and its clashes, not the whole day), so at six hours an hour already gets 22 and there
/// is nothing to fix. This exists for the case the band *cannot* be narrow: an all-morning booking
/// the meeting sits inside drags it out to ten or twelve hours, and at a fixed height the blocks
/// around it would go back to being untitled rectangles. So an hour is allowed
/// [`IDEAL_PREVIEW_HOUR_HEIGHT`] and the box grows: up to [`MAXIMUM_PREVIEW_HEIGHT`], past which
/// this stops being a preview above a message and starts pushing the message off the screen.
///
/// Beyond that cap short blocks quietly lose their titles. Nothing is ever *clipped*, only
/// unlabelled, and every block keeps its spoken label (`docs/calendar.md` §4).
pub(super) fn preview_height(hours: u32) -> f64 {
    (f64::from(hours.max(1)) * IDEAL_PREVIEW_HOUR_HEIGHT)
        .clamp(ORDINARY_PREVIEW_HEIGHT, MAXIMUM_PREVIEW_HEIGHT)
}
