// The invitation card's rules and its preview's geometry.
//
// Everything here is something a screenshot would not catch: whether "we haven't looked" reads
// differently from "nothing", whether the attendee buckets add up, whether the preview's hour band
// actually contains the meeting. The card itself is a WinUI surface, but none of these rules are,
// which is the whole reason they live in plain functions (InvitationFormat), and the reason those
// functions return a *choice* rather than a localised string: L10n.cs cannot be linked into this
// assembly, so a rule phrased as a string would be a rule no test could reach. The words are
// InvitationText's, one switch away, and that switch is exhaustive over these enums.
//
// The twin of Android's InvitationFormatTest.kt and Apple's InvitationFormatTests.swift.
using System.Globalization;
using Allodia.Mailcal.Calendar;
using uniffi.mailcal_bindings;
using Xunit;

namespace Allodia.Mailcal.Tests;

public class InvitationFormatTests
{
    // en-GB, so a day-month order and a 24-hour clock are the baseline the assertions read against.
    private static readonly CultureInfo En = new("en-GB");

    private static AttendeeTally Tally(
        uint total, uint accepted = 0, uint declined = 0, uint tentative = 0, uint needsAction = 0) =>
        new(total, accepted, declined, tentative, needsAction);

    // ---- The conflict line -------------------------------------------------------------------

    [Fact]
    public void None_one_and_many_are_three_different_sentences()
    {
        // "0 other things" and "1 other things" are not sentences, so each gets its own wording.
        Assert.Equal(ConflictLine.None, InvitationFormat.Conflicts(0, known: true));
        Assert.Equal(ConflictLine.One, InvitationFormat.Conflicts(1, known: true));
        Assert.Equal(ConflictLine.Many, InvitationFormat.Conflicts(3, known: true));
    }

    [Fact]
    public void An_unread_calendar_does_not_claim_the_day_is_free()
    {
        // The failure this guards actually shipped: mail syncs before calendars, so an invitation
        // opened on a cold start reached the card builder with nothing expanded, and the card said
        // "Nothing else in your calendar then" over a Monday holding two meetings.
        Assert.Equal(ConflictLine.Unknown, InvitationFormat.Conflicts(0, known: false));
        // And the count is not merely unprinted, it is not consulted at all.
        Assert.Equal(ConflictLine.Unknown, InvitationFormat.Conflicts(7, known: false));
    }

    // ---- The attendee tally ------------------------------------------------------------------

    [Fact]
    public void An_invitation_with_only_you_on_it_says_so()
    {
        // "1 of 1 accepted" is arithmetic, not a sentence about a meeting.
        Assert.Equal(
            new[] { AttendeeLine.OnlyYou },
            InvitationFormat.Attendees(Tally(total: 1, needsAction: 1)));
    }

    [Fact]
    public void An_invitation_with_no_attendees_says_nothing()
    {
        Assert.Empty(InvitationFormat.Attendees(Tally(total: 0)));
    }

    [Fact]
    public void Every_non_empty_bucket_earns_a_phrase()
    {
        // The four buckets sum to the total, so a line that drops one reads as arithmetic that does
        // not add up, the user counts three names in a five-person meeting and distrusts the rest.
        Assert.Equal(
            new[]
            {
                AttendeeLine.AcceptedOfTotal,
                AttendeeLine.TentativeOne,
                AttendeeLine.DeclinedOne,
                AttendeeLine.PendingOne,
            },
            InvitationFormat.Attendees(
                Tally(total: 5, accepted: 2, declined: 1, tentative: 1, needsAction: 1)));
    }

    [Fact]
    public void An_empty_bucket_is_left_out()
    {
        Assert.Equal(
            new[] { AttendeeLine.AcceptedOfTotal },
            InvitationFormat.Attendees(Tally(total: 3, accepted: 3)));
    }

    [Fact]
    public void One_person_is_not_described_in_the_plural()
    {
        // Dutch needs a different verb at one, "1 moeten nog antwoorden" is wrong, and the catalog
        // has no plural machinery, so each count-of-one is its own string. English reads fine either
        // way, which is exactly why this shipped unnoticed until the card was read in Dutch.
        Assert.Equal(
            new[]
            {
                AttendeeLine.AcceptedOfTotal,
                AttendeeLine.Tentative,
                AttendeeLine.Declined,
                AttendeeLine.Pending,
            },
            InvitationFormat.Attendees(
                Tally(total: 9, accepted: 1, declined: 2, tentative: 2, needsAction: 4)));
    }

    // ---- "When" ------------------------------------------------------------------------------

    [Fact]
    public void A_timed_meeting_names_the_day_once()
    {
        var line = InvitationFormat.When(
            "2026-01-19T09:30:00Z", "2026-01-19T10:30:00Z",
            allDay: false, zone: "UTC", use24Hour: true, culture: En);
        Assert.Contains("09:30", line, StringComparison.Ordinal);
        Assert.Contains("10:30", line, StringComparison.Ordinal);
        // One date, not two, start and end share a day.
        Assert.Equal(1, line.Split("2026").Length - 1);
    }

    [Fact]
    public void The_clock_follows_the_users_setting_not_the_culture()
    {
        // Mail and calendar must not disagree with each other, so this reads the app's own 12/24-hour
        // preference rather than what en-GB happens to default to.
        var twelve = InvitationFormat.When(
            "2026-01-19T14:00:00Z", "2026-01-19T15:00:00Z",
            allDay: false, zone: "UTC", use24Hour: false, culture: En);
        Assert.Contains("2:00", twelve, StringComparison.Ordinal);
        Assert.DoesNotContain("14:00", twelve, StringComparison.Ordinal);
    }

    [Fact]
    public void A_one_day_all_day_event_reads_as_one_date()
    {
        // The stored end is EXCLUSIVE: a single all-day event ends at the next midnight, and naming
        // both would tell the user it lasts two days.
        var line = InvitationFormat.When(
            "2026-01-19T00:00:00Z", "2026-01-20T00:00:00Z",
            allDay: true, zone: "UTC", use24Hour: true, culture: En);
        Assert.DoesNotContain("–", line, StringComparison.Ordinal);
    }

    [Fact]
    public void A_multi_day_all_day_event_names_its_inclusive_last_day()
    {
        var line = InvitationFormat.When(
            "2026-01-19T00:00:00Z", "2026-01-22T00:00:00Z",
            allDay: true, zone: "UTC", use24Hour: true, culture: En);
        Assert.Contains("–", line, StringComparison.Ordinal);
        Assert.Contains("21", line, StringComparison.Ordinal);
        Assert.DoesNotContain("22", line, StringComparison.Ordinal);
    }

    [Fact]
    public void The_display_zone_moves_the_clock()
    {
        var line = InvitationFormat.When(
            "2026-01-19T09:30:00Z", "2026-01-19T10:30:00Z",
            allDay: false, zone: "Europe/Amsterdam", use24Hour: true, culture: En);
        Assert.Contains("10:30", line, StringComparison.Ordinal);
        Assert.Contains("11:30", line, StringComparison.Ordinal);
    }

    [Fact]
    public void An_unparseable_instant_yields_no_line_rather_than_a_wrong_one()
    {
        Assert.Equal(string.Empty, InvitationFormat.When("", "", false, "UTC", true, En));
        Assert.Equal(string.Empty, InvitationFormat.When("not-a-date", "", false, "UTC", true, En));
    }

    [Fact]
    public void An_unresolvable_zone_still_draws_the_card()
    {
        // The core names an IANA zone; a host that cannot resolve one must show the meeting in the
        // wrong zone *visibly* rather than blank the "when" row and say nothing at all.
        Assert.NotEqual(
            string.Empty,
            InvitationFormat.When(
                "2026-01-19T09:30:00Z", "2026-01-19T10:30:00Z",
                allDay: false, zone: "Mars/Olympus_Mons", use24Hour: true, culture: En));
    }

    // ---- The preview's hour band -------------------------------------------------------------

    [Fact]
    public void The_span_always_contains_the_meeting()
    {
        var span = InvitationFormat.PreviewSpan(new MinuteSpan(13 * 60, 14 * 60), []);
        Assert.True(span.First <= 13);
        Assert.True(span.Last >= 14);
    }

    [Fact]
    public void A_short_meeting_on_an_empty_day_still_gets_context_around_it()
    {
        // A 30-minute meeting padded an hour each side is a two-hour sliver with nothing to compare
        // it against; the floor is what makes the picture worth drawing.
        var span = InvitationFormat.PreviewSpan(new MinuteSpan(10 * 60, (10 * 60) + 30), []);
        Assert.True(span.Count >= 6, $"{span.First}..{span.Last}");
    }

    [Fact]
    public void The_band_keeps_the_meeting_away_from_its_edges()
    {
        // Padding grown alternately, not all onto one end: a meeting pinned to the top of its own
        // preview reads as if the day starts there.
        var span = InvitationFormat.PreviewSpan(new MinuteSpan(14 * 60, 15 * 60), []);
        Assert.True(span.First < 14, $"{span.First}");
        Assert.True(span.Last > 16, $"{span.Last}");
    }

    [Fact]
    public void A_block_ending_mid_hour_keeps_the_whole_hour()
    {
        // Ceil, not truncate: an event ending 09:15 whose hour was floored would be drawn past the
        // bottom edge of its own preview.
        var span = InvitationFormat.PreviewSpan(new MinuteSpan(8 * 60, (9 * 60) + 15), []);
        Assert.True(span.Last >= 10, $"{span.Last}");
    }

    [Fact]
    public void The_band_covers_every_clash_in_full()
    {
        // The one thing that may not fall outside it. A conflict is by definition an event
        // overlapping the meeting, and it has to be drawn *whole*, a long booking cut off at the
        // top edge loses its title with it, which is exactly what the band exists to show.
        var span = InvitationFormat.PreviewSpan(
            new MinuteSpan(14 * 60, 15 * 60),
            [new MinuteSpan(9 * 60, 16 * 60)]);
        Assert.True(span.First <= 9, $"{span.First}");
        Assert.True(span.Last >= 16, $"{span.Last}");
    }

    [Fact]
    public void The_band_leaves_out_the_rest_of_the_day()
    {
        // …and everything that does NOT clash is left out, which is what buys the hours their
        // height. The card states the count in words above the grid and the disclosure label says
        // "around this meeting", so nothing is hidden without saying so.
        var span = InvitationFormat.PreviewSpan(
            new MinuteSpan(14 * 60, 15 * 60),
            [new MinuteSpan(8 * 60, 9 * 60), new MinuteSpan(21 * 60, 22 * 60)]);
        Assert.True(span.First > 8, $"{span.First}");
        Assert.True(span.Last <= 21, $"{span.Last}");
    }

    [Fact]
    public void A_block_ending_as_the_meeting_begins_is_not_a_clash()
    {
        // Half-open on both sides, exactly as the core's conflict count overlaps: back-to-back is
        // how a diary is packed, and widening the band for it would undo the zoom on every meeting
        // that follows another.
        var span = InvitationFormat.PreviewSpan(
            new MinuteSpan(14 * 60, 15 * 60),
            [new MinuteSpan(6 * 60, 14 * 60)]);
        Assert.True(span.First > 6, $"{span.First}");
    }

    [Fact]
    public void The_span_never_leaves_the_day()
    {
        var span = InvitationFormat.PreviewSpan(new MinuteSpan(23 * 60, 24 * 60), []);
        Assert.True(span.First >= 0);
        Assert.True(span.Last <= 24);
    }

    [Fact]
    public void A_squeezed_span_labels_fewer_hours_rather_than_overlapping_them()
    {
        Assert.Equal(1, InvitationFormat.PreviewStride(40));
        Assert.True(InvitationFormat.PreviewStride(5) > 1);
        // A degenerate height must not divide by zero or return a stride of 0, the ruler takes a
        // modulo by it, so zero is an exception and not merely an ugly picture.
        Assert.Equal(1, InvitationFormat.PreviewStride(0));
        Assert.Equal(1, InvitationFormat.PreviewStride(-3));
    }

    // ---- The preview's height -----------------------------------------------------------------

    [Fact]
    public void Every_band_the_span_can_produce_can_title_a_one_hour_block()
    {
        // The one thing the preview has to say. The band and the box are two halves of one rule,
        // narrow the band, or grow the box, and only their *ratio* decides whether a block gets a
        // title. So compose them, rather than pinning either number.
        //
        // That composition is the whole reason MinimumTitledHeight moved out of the WinUI view and
        // into InvitationFormat: this assembly cannot link Views/.
        for (var hours = 6; hours <= 12; hours++)
        {
            var hourHeight = InvitationFormat.PreviewHeight(hours) / hours;
            Assert.True(
                hourHeight >= InvitationFormat.MinimumTitledHeight,
                $"a one-hour block must carry its title over a {hours}-hour band, got {hourHeight}");
        }
    }

    [Fact]
    public void The_box_only_grows_when_the_band_cannot_stay_narrow()
    {
        // The ordinary case is the plain height: the band is six hours, so there is nothing to fix.
        Assert.Equal(132, InvitationFormat.PreviewHeight(6));
        // A long booking the meeting sits inside forces a wider band; the box follows it…
        Assert.True(InvitationFormat.PreviewHeight(10) > InvitationFormat.PreviewHeight(6));
        // …but stops, rather than pushing the message itself off the screen.
        Assert.Equal(240, InvitationFormat.PreviewHeight(24));
    }

    // ---- The meeting's own window ------------------------------------------------------------

    [Fact]
    public void The_meeting_window_is_wall_clock_minutes_in_the_layout_zone()
    {
        Assert.Equal(
            new MinuteSpan((10 * 60) + 30, (11 * 60) + 30),
            InvitationFormat.MeetingMinuteSpan(
                "2026-01-19T09:30:00Z", "2026-01-19T10:30:00Z", "Europe/Amsterdam"));
    }

    [Fact]
    public void A_meeting_running_past_midnight_ends_at_the_bottom_of_its_day()
    {
        var span = InvitationFormat.MeetingMinuteSpan(
            "2026-01-19T22:00:00Z", "2026-01-20T01:00:00Z", "UTC");
        Assert.Equal(22 * 60, span.Start);
        Assert.Equal(24 * 60, span.End);
    }

    [Fact]
    public void An_unparseable_instant_still_draws_the_day_it_was_given()
    {
        Assert.Equal(
            new MinuteSpan(0, 60), InvitationFormat.MeetingMinuteSpan("not-a-date", "", "UTC"));
    }

    // ---- The hold, and the write -------------------------------------------------------------

    [Fact]
    public void Only_an_unanswered_invitation_is_a_hold()
    {
        Assert.True(InvitationFormat.IsAwaitingResponse(ResponseStatus.NeedsAction));
        Assert.False(InvitationFormat.IsAwaitingResponse(ResponseStatus.Accepted));
        Assert.False(InvitationFormat.IsAwaitingResponse(ResponseStatus.Tentative));
        Assert.False(InvitationFormat.IsAwaitingResponse(ResponseStatus.Delegated));
        // Declined never reaches a client, the core hides those from every calendar surface, but
        // if one ever did it is not a hold either.
        Assert.False(InvitationFormat.IsAwaitingResponse(ResponseStatus.Declined));
    }

    [Fact]
    public void A_failed_answer_says_so_and_a_finished_one_says_nothing()
    {
        // The asymmetry is the whole rule. Once the answer lands the card already shows it, it
        // re-reads the calendar, so a second "sent" line is noise. A *failure* is the one that must
        // never be silent: the card would otherwise sit there showing the previous answer while the
        // organiser heard nothing, which is exactly the outcome this feature exists to prevent.
        Assert.Equal(WriteLine.None, InvitationFormat.Write(CalendarWriteStatus.Saved));
        Assert.Equal(WriteLine.None, InvitationFormat.Write(CalendarWriteStatus.Idle));
        Assert.Equal(WriteLine.Sending, InvitationFormat.Write(CalendarWriteStatus.Saving));
        Assert.Equal(WriteLine.Failed, InvitationFormat.Write(CalendarWriteStatus.Failed));
    }

    [Fact]
    public void A_hold_is_faded_and_a_commitment_is_untouched()
    {
        // The fade is what makes a hold read as provisional beside a booked commitment. It is applied
        // when the page is built, never inside a frame (§7), this is the arithmetic that does it.
        Assert.Equal(255, InvitationFormat.HoldAlpha(255, awaiting: false));
        Assert.Equal(102, InvitationFormat.HoldAlpha(255, awaiting: true));
        // Already-faded input fades again from where it is, rather than snapping to a fixed value.
        Assert.Equal(40, InvitationFormat.HoldAlpha(100, awaiting: true));
        // A colour that is already invisible cannot be made more so, and must not wrap round a byte.
        Assert.Equal(0, InvitationFormat.HoldAlpha(0, awaiting: true));
        Assert.Equal(1, InvitationFormat.HoldAlpha(2, awaiting: true));
    }

    [Fact]
    public void A_hold_says_so_in_the_spoken_label_and_a_commitment_does_not()
    {
        // This is the half of the treatment that is NOT optional. A dashed border and a hatched
        // gutter are invisible to a screen reader, so a grid that only draws the hold tells a
        // screen-reader user that an unanswered invitation is a settled commitment
        // (docs/calendar.md §4). Every surface that can show a hold runs through here.
        const string label = "Standup, 09:00-09:15, Work";
        const string awaiting = "Awaiting your response";

        Assert.Equal(
            "Standup, 09:00-09:15, Work, Awaiting your response",
            InvitationFormat.SpokenWithHold(label, awaiting, ResponseStatus.NeedsAction));
        Assert.Equal(label, InvitationFormat.SpokenWithHold(label, awaiting, ResponseStatus.Accepted));
        Assert.Equal(label, InvitationFormat.SpokenWithHold(label, awaiting, ResponseStatus.Tentative));
        Assert.Equal(label, InvitationFormat.SpokenWithHold(label, awaiting, ResponseStatus.Delegated));
    }
}
