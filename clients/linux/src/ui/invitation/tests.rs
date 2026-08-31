//! The invitation card's rules, pinned. Every one of them is a sentence the user reads or a number
//! the preview divides by, and none of them needs a window.

use mailcal_bindings::{
    AttendeeTally, CalendarWriteStatus, InvitationKind, InvitationResponse, ResponseStatus,
};

use super::{
    HourSpan, MinuteSpan, attendees, conflicts, meeting_minute_span, notice, preview_height,
    preview_span, preview_stride, reply_subject, response, title, when, write_line,
};
use crate::l10n;

const ZONE: &str = "Europe/Amsterdam";

fn tally(
    total: u32,
    accepted: u32,
    declined: u32,
    tentative: u32,
    needs_action: u32,
) -> AttendeeTally {
    AttendeeTally {
        total,
        accepted,
        declined,
        tentative,
        needs_action,
    }
}

#[test]
fn an_unread_calendar_is_a_different_sentence_from_an_empty_one() {
    // `conflicts_known: false` is NOT zero: printing "nothing else in your calendar then" over a
    // calendar nobody read is the confident lie `docs/calendar.md` §4 forbids, and a cold start
    // lands there every time (mail syncs before calendars).
    assert_eq!(conflicts(0, false), l10n::invitation_conflicts_unknown());
    assert_eq!(conflicts(3, false), l10n::invitation_conflicts_unknown());
    assert_eq!(conflicts(0, true), l10n::invitation_conflicts_none());
    assert_ne!(conflicts(0, true), conflicts(0, false));
}

#[test]
fn one_conflict_gets_its_own_sentence_rather_than_a_count() {
    assert_eq!(conflicts(1, true), l10n::invitation_conflicts_one());
    assert_eq!(conflicts(2, true), l10n::invitation_conflicts(2));
}

#[test]
fn every_non_zero_bucket_earns_a_phrase_so_the_tally_adds_up() {
    let line = attendees(&tally(6, 2, 1, 1, 2));
    assert!(line.starts_with(&l10n::invitation_attendees("2", "6")));
    assert!(line.contains(l10n::invitation_attendees_tentative_one()));
    assert!(line.contains(l10n::invitation_attendees_declined_one()));
    assert!(line.contains(&l10n::invitation_attendees_pending(2)));
}

#[test]
fn a_bucket_of_one_uses_its_singular_key() {
    // The catalog has no plural machinery and Dutch needs a different verb at one; English reads
    // fine either way, which is why this was invisible until the card was read in Dutch.
    let one = attendees(&tally(3, 1, 0, 0, 1));
    assert!(one.contains(l10n::invitation_attendees_pending_one()));
    let many = attendees(&tally(4, 1, 0, 0, 2));
    assert!(many.contains(&l10n::invitation_attendees_pending(2)));
}

#[test]
fn a_meeting_of_one_is_a_sentence_rather_than_arithmetic() {
    assert_eq!(
        attendees(&tally(1, 1, 0, 0, 0)),
        l10n::invitation_attendees_one()
    );
    assert!(attendees(&tally(0, 0, 0, 0, 0)).is_empty());
}

#[test]
fn only_a_superseded_card_explains_why_it_offers_no_answer() {
    assert_eq!(
        notice(InvitationKind::Superseded),
        Some(l10n::invitation_superseded())
    );
    for silent in [
        InvitationKind::Rsvp,
        InvitationKind::Cancelled,
        InvitationKind::Informational,
    ] {
        assert_eq!(notice(silent), None);
    }
    assert_eq!(
        title(InvitationKind::Cancelled),
        l10n::invitation_cancelled_title()
    );
    assert_eq!(
        title(InvitationKind::Superseded),
        l10n::invitation_superseded_title()
    );
    assert_eq!(
        title(InvitationKind::Informational),
        l10n::invitation_informational_title()
    );
    assert_eq!(title(InvitationKind::Rsvp), l10n::invitation_title());
}

#[test]
fn the_card_reports_the_calendars_answer_in_words() {
    assert_eq!(
        response(ResponseStatus::Accepted),
        l10n::invitation_response_accepted()
    );
    assert_eq!(
        response(ResponseStatus::Declined),
        l10n::invitation_response_declined()
    );
    assert_eq!(
        response(ResponseStatus::NeedsAction),
        l10n::invitation_response_needs_action()
    );
    assert_eq!(
        response(ResponseStatus::Delegated),
        l10n::invitation_response_delegated()
    );
}

#[test]
fn a_settled_write_says_nothing_and_a_failed_one_always_does() {
    // `Saved` is silent on purpose: by then the card has been rebuilt from the calendar and shows
    // the new answer. `Failed` is the one state that must never be silent.
    assert_eq!(write_line(CalendarWriteStatus::Idle), None);
    assert_eq!(write_line(CalendarWriteStatus::Saved), None);
    assert_eq!(
        write_line(CalendarWriteStatus::Saving),
        Some(l10n::invitation_sending())
    );
    assert_eq!(
        write_line(CalendarWriteStatus::Failed),
        Some(l10n::invitation_failed())
    );
}

#[test]
fn the_reply_subject_names_the_answer_in_the_users_language() {
    assert_eq!(
        reply_subject(InvitationResponse::Accept, "Sprint planning"),
        l10n::invitation_reply_subject_accepted("Sprint planning")
    );
    assert_eq!(
        reply_subject(InvitationResponse::Tentative, "Sprint planning"),
        l10n::invitation_reply_subject_tentative("Sprint planning")
    );
    assert_eq!(
        reply_subject(InvitationResponse::Decline, "Sprint planning"),
        l10n::invitation_reply_subject_declined("Sprint planning")
    );
    // A titleless meeting borrows the card's own placeholder rather than putting "Accepted: " in a
    // stranger's inbox with nothing after the colon.
    assert_eq!(
        reply_subject(InvitationResponse::Accept, "   "),
        l10n::invitation_reply_subject_accepted(l10n::invitation_no_title())
    );
}

#[test]
fn a_timed_meeting_collapses_one_date_and_a_multi_day_one_names_both() {
    let same_day = when(
        "2026-08-20T08:00:00Z",
        "2026-08-20T09:30:00Z",
        false,
        ZONE,
        true,
    );
    assert!(same_day.contains("10:00 – 11:30"), "{same_day}");
    assert_eq!(same_day.matches("10:00").count(), 1);

    let across = when(
        "2026-08-20T22:00:00Z",
        "2026-08-21T06:00:00Z",
        false,
        ZONE,
        true,
    );
    assert!(across.contains("00:00 –"), "{across}");
    assert!(across.contains("08:00"), "{across}");
}

#[test]
fn an_all_day_meeting_never_names_its_exclusive_end() {
    // The stored end is EXCLUSIVE. Naming it would tell the user a one-day event lasts two.
    let one_day = when(
        "2026-08-20T00:00:00Z",
        "2026-08-21T00:00:00Z",
        true,
        "UTC",
        true,
    );
    assert!(!one_day.contains('–'), "{one_day}");

    let two_days = when(
        "2026-08-20T00:00:00Z",
        "2026-08-22T00:00:00Z",
        true,
        "UTC",
        true,
    );
    assert!(two_days.contains('–'), "{two_days}");
}

#[test]
fn an_unparseable_instant_draws_the_day_it_was_given_rather_than_nothing() {
    assert_eq!(
        when("not-a-time", "also-not", false, ZONE, true),
        String::new()
    );
    assert_eq!(
        meeting_minute_span("not-a-time", "also-not", ZONE),
        MinuteSpan { start: 0, end: 60 }
    );
}

#[test]
fn the_meeting_is_placed_in_the_zone_the_day_was_laid_out_in() {
    assert_eq!(
        meeting_minute_span("2026-08-20T08:00:00Z", "2026-08-20T09:00:00Z", ZONE),
        MinuteSpan {
            start: 600,
            end: 660
        }
    );
    // An end on a later day belongs to the end of this day's grid, not to minute zero.
    assert_eq!(
        meeting_minute_span("2026-08-20T21:00:00Z", "2026-08-21T05:00:00Z", ZONE),
        MinuteSpan {
            start: 23 * 60,
            end: 24 * 60
        }
    );
}

#[test]
fn the_band_is_the_meeting_its_clashes_and_an_hour_of_air() {
    // 10:00–11:00 with nothing around it: an hour each side, then grown to the six-hour floor so a
    // short meeting on an empty afternoon still has context.
    let band = preview_span(
        MinuteSpan {
            start: 600,
            end: 660,
        },
        &[],
    );
    assert_eq!(band.count(), 6);
    assert!(band.first <= 9 && band.last >= 12, "{band:?}");
}

#[test]
fn a_clash_that_starts_hours_earlier_drags_the_band_back_whole() {
    // Nothing the card counts may be cut off at the top edge with its title off-screen.
    let band = preview_span(
        MinuteSpan {
            start: 600,
            end: 660,
        },
        &[MinuteSpan {
            start: 420,
            end: 630,
        }],
    );
    assert!(band.first <= 6, "{band:?}");
    assert!(band.last >= 12, "{band:?}");
}

#[test]
fn back_to_back_is_not_a_clash_and_does_not_widen_the_band() {
    // Half-open on both sides, exactly as `count_conflicts` overlaps in the core.
    let touching = preview_span(
        MinuteSpan {
            start: 600,
            end: 660,
        },
        &[MinuteSpan {
            start: 120,
            end: 600,
        }],
    );
    let alone = preview_span(
        MinuteSpan {
            start: 600,
            end: 660,
        },
        &[],
    );
    assert_eq!(touching, alone);
}

#[test]
fn the_band_never_leaves_the_day() {
    let first_thing = preview_span(MinuteSpan { start: 0, end: 30 }, &[]);
    assert_eq!(first_thing.first, 0);
    assert_eq!(first_thing.count(), 6);

    let last_thing = preview_span(
        MinuteSpan {
            start: 23 * 60 + 30,
            end: 24 * 60,
        },
        &[],
    );
    assert_eq!(last_thing.last, 24);
    assert_eq!(last_thing.count(), 6);
}

#[test]
fn a_band_this_span_can_produce_still_titles_a_one_hour_block() {
    // The two formulas are the rule, composed; not a pinned constant. Up to the height cap, a
    // 60-minute block must keep its title at every band the span can produce.
    for hours in 6..=12 {
        let hour_height = preview_height(hours) / f64::from(hours);
        assert!(
            hour_height >= super::MINIMUM_TITLED_HEIGHT,
            "a {hours}-hour band gives an hour {hour_height}"
        );
    }
}

#[test]
fn past_the_height_cap_short_blocks_lose_their_titles_and_nothing_else() {
    // The documented trade, pinned so the boundary is visible rather than assumed: beyond the cap
    // the hour height falls below the titling threshold. Nothing is ever *clipped*, only
    // unlabelled, and every block keeps its spoken label (`docs/calendar.md` §4): which is why
    // the preview draws its labels into an overlay rather than only onto the Cairo surface.
    let hour_height = preview_height(21) / 21.0;
    assert!(hour_height < super::MINIMUM_TITLED_HEIGHT);
    assert!(hour_height > 0.0);
}

#[test]
fn the_preview_grows_only_when_the_band_cannot_be_narrow_and_stops_at_the_cap() {
    let ordinary = preview_height(6);
    assert!((preview_height(1) - ordinary).abs() < f64::EPSILON);
    assert!(preview_height(10) > ordinary);
    assert!((preview_height(24) - preview_height(20)).abs() < f64::EPSILON);
}

#[test]
fn a_squeezed_ruler_labels_fewer_hours_and_never_strides_by_zero() {
    assert_eq!(preview_stride(22.0), 1);
    assert_eq!(preview_stride(9.0), 2);
    assert_eq!(preview_stride(0.0), 1);
    assert_eq!(preview_stride(-4.0), 1);
}

#[test]
fn an_hour_span_counts_its_own_hours() {
    assert_eq!(HourSpan { first: 8, last: 14 }.count(), 6);
    // Saturating, so a malformed band is an empty one rather than a panic in a draw callback.
    assert_eq!(HourSpan { first: 14, last: 8 }.count(), 0);
}
