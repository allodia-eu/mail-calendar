// The invitation card's copy and its preview's geometry.
//
// Everything here is a rule a screenshot would not catch: whether "we haven't looked" reads
// differently from "nothing", whether the attendee buckets add up, whether the preview's hour band
// actually contains the meeting. The card itself is a Compose surface, but none of these rules are:
// which is the whole reason they live in plain functions (InvitationFormat.kt).
package eu.allodia.mailcal

import android.content.Context
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import uniffi.mailcal_bindings.AttendeeTally
import uniffi.mailcal_bindings.CalendarWriteStatus
import uniffi.mailcal_bindings.InvitationKind
import uniffi.mailcal_bindings.InvitationResponse
import uniffi.mailcal_bindings.ResponseStatus

private fun ctx(): Context = RuntimeEnvironment.getApplication()

private val EN = java.util.Locale.forLanguageTag("en-GB")

private fun tally(
    total: UInt,
    accepted: UInt = 0u,
    declined: UInt = 0u,
    tentative: UInt = 0u,
    needsAction: UInt = 0u,
) = AttendeeTally(
    total = total,
    accepted = accepted,
    declined = declined,
    tentative = tentative,
    needsAction = needsAction,
)

@RunWith(RobolectricTestRunner::class)
class InvitationFormatTest {

    // ---- The conflict line -----------------------------------------------------------------------

    @Test
    fun none_one_and_many_are_three_different_sentences() {
        // "0 other things" and "1 other things" are not sentences, so each gets its own wording.
        val none = invitationConflictLine(ctx(), 0u, known = true)
        val one = invitationConflictLine(ctx(), 1u, known = true)
        val many = invitationConflictLine(ctx(), 3u, known = true)
        assertNotEquals(none, one)
        assertNotEquals(one, many)
        assertTrue(one.contains("1"))
        assertTrue(many.contains("3"))
    }

    @Test
    fun an_unread_calendar_does_not_claim_the_day_is_free() {
        // The failure this guards actually shipped: mail syncs before calendars, so an invitation
        // opened on a cold start reached the card builder with nothing expanded, and the card said
        // "Nothing else in your calendar then" over a Monday holding two meetings.
        val unknown = invitationConflictLine(ctx(), 0u, known = false)
        assertNotEquals(invitationConflictLine(ctx(), 0u, known = true), unknown)
        // And the count is not merely unprinted, it is not consulted at all.
        assertEquals(unknown, invitationConflictLine(ctx(), 7u, known = false))
        assertFalse(unknown.contains("7"))
    }

    // ---- The attendee tally ----------------------------------------------------------------------

    @Test
    fun an_invitation_with_only_you_on_it_says_so() {
        // "1 of 1 accepted" is arithmetic, not a sentence about a meeting.
        assertEquals(
            listOf(L10n.invitation_attendees_one(ctx())),
            invitationAttendeeLines(ctx(), tally(total = 1u, needsAction = 1u)),
        )
    }

    @Test
    fun an_invitation_with_no_attendees_says_nothing() {
        assertTrue(invitationAttendeeLines(ctx(), tally(total = 0u)).isEmpty())
    }

    @Test
    fun every_non_empty_bucket_earns_a_phrase() {
        // The four buckets sum to the total, so a line that drops one reads as arithmetic that does
        // not add up, the user counts three names in a five-person meeting and distrusts the rest.
        val lines = invitationAttendeeLines(
            ctx(),
            tally(total = 5u, accepted = 2u, declined = 1u, tentative = 1u, needsAction = 1u),
        )
        assertEquals(4, lines.size)
        assertTrue(lines.first().contains("2"))
        assertTrue(lines.first().contains("5"))
        assertFalse(lines.contains(""))
    }

    @Test
    fun an_empty_bucket_is_left_out() {
        val lines = invitationAttendeeLines(ctx(), tally(total = 3u, accepted = 3u))
        assertEquals(1, lines.size)
    }

    // ---- "When" ----------------------------------------------------------------------------------

    @Test
    fun a_timed_meeting_names_the_day_once() {
        val line = invitationWhen(
            startsAt = "2026-01-19T09:30:00Z",
            endsAt = "2026-01-19T10:30:00Z",
            allDay = false,
            zone = "UTC",
            use24Hour = true,
            locale = EN,
        )
        assertTrue(line, line.contains("09:30"))
        assertTrue(line, line.contains("10:30"))
        // One date, not two, start and end share a day.
        assertEquals(1, Regex("2026").findAll(line).count())
    }

    @Test
    fun the_clock_follows_the_users_setting_not_the_locale() {
        // Mail and calendar must not disagree with each other, so this reads the app's own 12/24-hour
        // preference rather than what en-GB happens to default to.
        val twelve = invitationWhen(
            "2026-01-19T14:00:00Z", "2026-01-19T15:00:00Z",
            allDay = false, zone = "UTC", use24Hour = false, locale = EN,
        )
        assertTrue(twelve, twelve.contains("2:00 PM"))
    }

    @Test
    fun a_one_day_all_day_event_reads_as_one_date() {
        // The stored end is EXCLUSIVE: a single all-day event ends at the next midnight, and naming
        // both would tell the user it lasts two days.
        val line = invitationWhen(
            startsAt = "2026-01-19T00:00:00Z",
            endsAt = "2026-01-20T00:00:00Z",
            allDay = true,
            zone = "UTC",
            use24Hour = true,
            locale = EN,
        )
        assertFalse(line, line.contains("–"))
    }

    @Test
    fun a_multi_day_all_day_event_names_its_inclusive_last_day() {
        val line = invitationWhen(
            startsAt = "2026-01-19T00:00:00Z",
            endsAt = "2026-01-22T00:00:00Z",
            allDay = true,
            zone = "UTC",
            use24Hour = true,
            locale = EN,
        )
        assertTrue(line, line.contains("–"))
        assertTrue(line, line.contains("21"))
        assertFalse(line, line.contains("22"))
    }

    @Test
    fun the_display_zone_moves_the_clock() {
        val line = invitationWhen(
            "2026-01-19T09:30:00Z", "2026-01-19T10:30:00Z",
            allDay = false, zone = "Europe/Amsterdam", use24Hour = true, locale = EN,
        )
        assertTrue(line, line.contains("10:30"))
    }

    @Test
    fun an_unparseable_instant_yields_no_line_rather_than_a_wrong_one() {
        assertEquals("", invitationWhen("", "", false, "UTC", true, EN))
    }

    // ---- The preview's hour band -----------------------------------------------------------------

    @Test
    fun the_span_always_contains_the_meeting() {
        val meeting = MinuteSpan(start = 13 * 60, end = 14 * 60)
        val span = invitationPreviewSpan(meeting = meeting, others = emptyList())
        assertTrue(span.first <= 13)
        assertTrue(span.last + 1 >= 14)
    }

    @Test
    fun a_short_meeting_on_an_empty_day_still_gets_context_around_it() {
        // A 30-minute meeting padded an hour each side is a two-hour sliver with nothing to compare
        // it against; the floor is what makes the picture worth drawing.
        val span = invitationPreviewSpan(MinuteSpan(10 * 60, 10 * 60 + 30), emptyList())
        assertTrue("${span.first}..${span.last}", span.last + 1 - span.first >= 6)
    }

    @Test
    fun the_band_keeps_the_meeting_away_from_its_edges() {
        // Padding grown alternately, not all onto one end: a meeting pinned to the top of its own
        // preview reads as if the day starts there.
        val span = invitationPreviewSpan(MinuteSpan(14 * 60, 15 * 60), emptyList())
        assertTrue("${span.first}", span.first < 14)
        assertTrue("${span.last}", span.last + 1 > 16)
    }

    @Test
    fun a_block_ending_mid_hour_keeps_the_whole_hour() {
        // Ceil, not truncate: an event ending 09:15 whose hour was floored would be drawn past the
        // bottom edge of its own preview.
        val span = invitationPreviewSpan(MinuteSpan(8 * 60, 9 * 60 + 15), emptyList())
        assertTrue("${span.last}", span.last + 1 >= 10)
    }

    @Test
    fun the_band_covers_every_clash_in_full() {
        // The one thing that may not fall outside it. A conflict is by definition an event
        // overlapping the meeting, and it has to be drawn *whole*, a long booking cut off at the
        // top edge loses its title with it, which is exactly what the band exists to show.
        val span = invitationPreviewSpan(
            meeting = MinuteSpan(14 * 60, 15 * 60),
            others = listOf(MinuteSpan(9 * 60, 16 * 60)),
        )
        assertTrue(span.first <= 9)
        assertTrue(span.last + 1 >= 16)
    }

    @Test
    fun the_band_leaves_out_the_rest_of_the_day() {
        // …and everything that does NOT clash is left out, which is what buys the hours their
        // height. The card states the count in words above the grid and the disclosure label says
        // "around this meeting", so nothing is hidden without saying so.
        val span = invitationPreviewSpan(
            meeting = MinuteSpan(14 * 60, 15 * 60),
            others = listOf(MinuteSpan(8 * 60, 9 * 60), MinuteSpan(21 * 60, 22 * 60)),
        )
        assertFalse("${span.first}..${span.last}", span.contains(8))
        assertFalse("${span.first}..${span.last}", span.contains(21))
        assertTrue(span.contains(14))
    }

    @Test
    fun a_block_ending_as_the_meeting_begins_is_not_a_clash() {
        // Half-open on both sides, exactly as the core's conflict count overlaps: back-to-back is
        // how a diary is packed, and widening the band for it would undo the zoom on every meeting
        // that follows another.
        val span = invitationPreviewSpan(
            meeting = MinuteSpan(14 * 60, 15 * 60),
            others = listOf(MinuteSpan(6 * 60, 14 * 60)),
        )
        assertFalse("${span.first}..${span.last}", span.contains(6))
    }

    @Test
    fun the_span_never_leaves_the_day() {
        val span = invitationPreviewSpan(MinuteSpan(23 * 60, 24 * 60), emptyList())
        assertTrue(span.first >= 0)
        assertTrue(span.last + 1 <= 24)
    }

    @Test
    fun a_squeezed_span_labels_fewer_hours_rather_than_overlapping_them() {
        assertEquals(1, invitationPreviewStride(hourHeightDp = 40f))
        assertTrue(invitationPreviewStride(hourHeightDp = 5f) > 1)
        // A degenerate height must not divide by zero or return a stride of 0 (an infinite loop).
        assertEquals(1, invitationPreviewStride(hourHeightDp = 0f))
    }

    // ---- The preview's height ---------------------------------------------------------------------

    @Test
    fun every_band_the_span_can_produce_can_name_a_one_hour_block() {
        // The one thing the preview has to say. The band and the box are two halves of one rule:
        // narrow the band, or grow the box, and only their *ratio* decides whether a block gets a
        // title. So compose them, rather than pinning either number.
        //
        // 11 dp is the title line drawPreviewChip measures, inside a block's 1 dp insets.
        for (hours in 6..12) {
            val hourHeight = invitationPreviewHeightDp(hours) / hours
            assertTrue(
                "a one-hour block must carry its title over a $hours-hour band, got $hourHeight dp",
                hourHeight - 2f >= 11f,
            )
        }
    }

    @Test
    fun the_box_only_grows_when_the_band_cannot_stay_narrow() {
        // The ordinary case is the plain height: the band is six hours, so there is nothing to fix.
        assertEquals(132f, invitationPreviewHeightDp(6), 0.01f)
        // A long booking the meeting sits inside forces a wider band; the box follows it…
        assertTrue(invitationPreviewHeightDp(10) > invitationPreviewHeightDp(6))
        // …but stops, rather than pushing the message itself off the screen.
        assertEquals(240f, invitationPreviewHeightDp(24), 0.01f)
    }

    // ---- The meeting's own window ----------------------------------------------------------------

    @Test
    fun the_meeting_window_is_wall_clock_minutes_in_the_layout_zone() {
        val span = meetingMinuteSpan(
            "2026-01-19T09:30:00Z",
            "2026-01-19T10:30:00Z",
            "Europe/Amsterdam",
        )
        assertEquals(MinuteSpan(10 * 60 + 30, 11 * 60 + 30), span)
    }

    @Test
    fun a_meeting_running_past_midnight_ends_at_the_bottom_of_its_day() {
        val span = meetingMinuteSpan("2026-01-19T22:00:00Z", "2026-01-20T01:00:00Z", "UTC")
        assertEquals(22 * 60, span.start)
        assertEquals(24 * 60, span.end)
    }

    @Test
    fun an_unparseable_instant_still_draws_the_day_it_was_given() {
        assertEquals(MinuteSpan(0, 60), meetingMinuteSpan("not-a-date", "", "UTC"))
    }

    // ---- The spoken hold -------------------------------------------------------------------------

    @Test
    fun a_hold_says_it_is_awaiting_an_answer_and_a_commitment_does_not() {
        // The dashed border and hatched gutter are invisible to a screen reader, so the label has to
        // carry the whole disclosure (docs/calendar.md §4).
        val answered =
            calendarEventLabel(ctx(), "Design review", "09:30 – 10:30", "Work", ResponseStatus.ACCEPTED)
        val hold =
            calendarEventLabel(ctx(), "Design review", "09:30 – 10:30", "Work", ResponseStatus.NEEDS_ACTION)
        assertFalse(answered.contains(L10n.a11y_invitation_awaiting_response(ctx())))
        assertTrue(hold.contains(L10n.a11y_invitation_awaiting_response(ctx())))
        assertTrue(hold.startsWith(answered))
    }

    @Test
    fun only_an_unanswered_invitation_is_a_hold() {
        assertTrue(isAwaitingResponse(ResponseStatus.NEEDS_ACTION))
        assertFalse(isAwaitingResponse(ResponseStatus.ACCEPTED))
        assertFalse(isAwaitingResponse(ResponseStatus.TENTATIVE))
        assertFalse(isAwaitingResponse(ResponseStatus.DELEGATED))
        // Declined never reaches a client, the core hides those from every calendar surface, but
        // if one ever did it is not a hold either.
        assertFalse(isAwaitingResponse(ResponseStatus.DECLINED))
    }

    // ---- The card's headings ---------------------------------------------------------------------

    @Test
    fun each_kind_of_card_names_itself_differently() {
        // Enumerated rather than listed by hand: a kind added later that forgot its own heading
        // would otherwise silently reuse another's, and the list this replaced had already fallen
        // one behind.
        val titles = InvitationKind.entries.map { invitationTitle(ctx(), it) }
        assertEquals(InvitationKind.entries.size, titles.toSet().size)
    }

    @Test
    fun only_a_superseded_card_explains_itself() {
        // The other kinds either offer buttons or say plainly what they are; a superseded card
        // still looks answerable, so its missing buttons are the one absence owing a sentence.
        assertNotNull(invitationNotice(ctx(), InvitationKind.SUPERSEDED))
        val quiet = InvitationKind.entries.filter { it != InvitationKind.SUPERSEDED }
        assertTrue(quiet.all { invitationNotice(ctx(), it) == null })
    }

    @Test
    fun each_answer_reads_differently() {
        val lines = ResponseStatus.entries.map { invitationResponseLine(ctx(), it) }
        assertEquals(ResponseStatus.entries.size, lines.toSet().size)
    }

    @Test
    fun each_answer_gets_its_own_reply_subject_naming_the_meeting() {
        // This one leaves the device: on an account whose calendar server does no scheduling,
        // the core emails the reply itself and this is the subject line an organiser reads. Three
        // answers that produced the same subject would have them guessing, and a subject that
        // dropped the summary would leave them guessing which meeting.
        val subjects = InvitationResponse.entries.map {
            invitationReplySubject(ctx(), it, "Sprint planning")
        }
        assertEquals(InvitationResponse.entries.size, subjects.toSet().size)
        assertTrue(subjects.all { it.contains("Sprint planning") })
    }

    // ---- Localisation ----------------------------------------------------------------------------

    @Test
    @Config(qualifiers = "nl")
    fun the_cards_copy_is_translated() {
        // **Assert that it is Dutch, not which Dutch**, cf. CalendarFormatTest: the exact glyphs of
        // a localised date belong to the JDK's CLDR and move under us. These are our own catalog
        // strings, so they are safe to pin; the date format is not, and is not asserted here.
        assertEquals("Uitnodiging voor een afspraak", invitationTitle(ctx(), InvitationKind.RSVP))
        assertEquals(
            "We hebben je agenda nog niet bekeken",
            invitationConflictLine(ctx(), 0u, known = false),
        )
        assertEquals("Wacht op je antwoord", L10n.a11y_invitation_awaiting_response(ctx()))
        // The reply subject is the one string here a *stranger* reads, in an inbox we do not
        // control, so it being translated matters more than any of the above, not less.
        assertEquals(
            "Geaccepteerd: Sprint planning",
            invitationReplySubject(ctx(), InvitationResponse.ACCEPT, "Sprint planning"),
        )
    }
}

@RunWith(RobolectricTestRunner::class)
class InvitationWriteLineTest {
    private fun ctx(): Context = ApplicationProvider.getApplicationContext()

    @Test
    fun a_failed_answer_says_so_and_a_finished_one_says_nothing() {
        // The asymmetry is the whole rule. Once the answer lands the card already shows it, it
        // re-reads the calendar, so a second "sent" line is noise. A *failure* is the one that
        // must never be silent: the card would otherwise sit there showing the previous answer
        // while the organiser heard nothing, which is exactly the outcome this feature exists to
        // prevent.
        assertNull(invitationWriteLine(ctx(), CalendarWriteStatus.SAVED))
        assertNull(invitationWriteLine(ctx(), CalendarWriteStatus.IDLE))
        assertNotNull(invitationWriteLine(ctx(), CalendarWriteStatus.SAVING))
        assertNotNull(invitationWriteLine(ctx(), CalendarWriteStatus.FAILED))
        assertNotEquals(
            invitationWriteLine(ctx(), CalendarWriteStatus.SAVING),
            invitationWriteLine(ctx(), CalendarWriteStatus.FAILED),
        )
    }
}

@RunWith(RobolectricTestRunner::class)
class InvitationAttendeeSingularTest {
    private fun ctx(): Context = ApplicationProvider.getApplicationContext()

    private fun tally(tentative: UInt, declined: UInt, pending: UInt) = AttendeeTally(
        total = 5u,
        accepted = 1u,
        declined = declined,
        tentative = tentative,
        needsAction = pending,
    )

    @Test
    fun one_person_is_not_described_in_the_plural() {
        // Dutch needs a different verb at one, "1 moeten nog antwoorden" is wrong, and the
        // catalog has no plural machinery, so each count-of-one is its own string. English reads
        // fine either way, which is exactly why this shipped unnoticed until the card was looked
        // at in Dutch on a phone.
        //
        // Asserted against the singular *strings themselves* rather than by comparing the one-
        // and many- wordings: that comparison only distinguishes them in a language where the
        // plural differs, so it would pass or fail on which locale the test JVM happened to pick
        // the CLDR trap AGENTS.md records. This pins the rule in any language.
        val lines = invitationAttendeeLines(ctx(), tally(tentative = 1u, declined = 1u, pending = 1u))
        assertEquals(4, lines.size)
        assertEquals(L10n.invitation_attendees_tentative_one(ctx()), lines[1])
        assertEquals(L10n.invitation_attendees_declined_one(ctx()), lines[2])
        assertEquals(L10n.invitation_attendees_pending_one(ctx()), lines[3])

        val many = invitationAttendeeLines(ctx(), tally(tentative = 2u, declined = 2u, pending = 2u))
        assertEquals(L10n.invitation_attendees_pending(ctx(), 2), many[3])
    }
}
