// Every localised string and every layout decision the invitation card makes, as plain functions
// over plain values.
//
// The core decides *whether there is a card and what is on it* (docs/invitations.md); this file
// decides how those values read in the user's language and how tall the preview's hours are. It is
// deliberately composable-free so each rule is a check that can fail, the conflict wording, the
// attendee arithmetic and the preview's hour span are all things a screenshot would not catch, and
// a `@Composable` cannot be called from a plain JVM test.
//
// Times arrive as UTC instants because the core ships no display tzdata (docs/timestamps.md), so
// every function here takes the display zone as an argument rather than reading a global.
package eu.allodia.mailcal

import android.content.Context
import java.time.DateTimeException
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.time.format.DateTimeParseException
import java.time.format.FormatStyle
import java.util.Locale
import kotlin.math.ceil
import uniffi.mailcal_bindings.AttendeeTally
import uniffi.mailcal_bindings.CalendarWriteStatus
import uniffi.mailcal_bindings.InvitationKind
import uniffi.mailcal_bindings.InvitationResponse
import uniffi.mailcal_bindings.ResponseStatus

/** A span of wall-clock minutes from midnight, the unit the core's grid solver emits. */
internal data class MinuteSpan(val start: Int, val end: Int)

/** The card's heading: what this message is, before any detail. */
internal fun invitationTitle(ctx: Context, kind: InvitationKind): String = when (kind) {
    InvitationKind.RSVP -> L10n.invitation_title(ctx)
    InvitationKind.CANCELLED -> L10n.invitation_cancelled_title(ctx)
    InvitationKind.INFORMATIONAL -> L10n.invitation_informational_title(ctx)
    InvitationKind.SUPERSEDED -> L10n.invitation_superseded_title(ctx)
}

/**
 * The sentence under the heading saying why this card offers no answer, or null where none is owed.
 *
 * A superseded invitation is the one kind whose *missing* buttons need explaining: its details still
 * look answerable, so without this the card reads as broken rather than out of date. Distinct from
 * `invitation_cannot_respond`, which is a different fact, that the account can never answer at all,
 * rather than that this particular mail is no longer the current copy.
 */
internal fun invitationNotice(ctx: Context, kind: InvitationKind): String? = when (kind) {
    InvitationKind.SUPERSEDED -> L10n.invitation_superseded(ctx)
    InvitationKind.RSVP, InvitationKind.CANCELLED, InvitationKind.INFORMATIONAL -> null
}

/**
 * The subject line for the reply the core emails to the organiser, on an account whose calendar
 * server does no scheduling of its own.
 *
 * Composed in the client because the core carries no locale (AGENTS.md → "Localisation is
 * client-side"), and this is copy a stranger reads in their inbox. Passing null instead is safe
 * but silent: the core falls back to `Re:` plus the invitation's own subject, and the organiser's
 * message list then no longer says which way we answered.
 */
internal fun invitationReplySubject(
    ctx: Context,
    response: InvitationResponse,
    summary: String,
): String = when (response) {
    InvitationResponse.ACCEPT -> L10n.invitation_reply_subject_accepted(ctx, summary)
    InvitationResponse.TENTATIVE -> L10n.invitation_reply_subject_tentative(ctx, summary)
    InvitationResponse.DECLINE -> L10n.invitation_reply_subject_declined(ctx, summary)
}

/** How this account has answered so far, in words. */
internal fun invitationResponseLine(ctx: Context, status: ResponseStatus): String = when (status) {
    ResponseStatus.NEEDS_ACTION -> L10n.invitation_response_needs_action(ctx)
    ResponseStatus.ACCEPTED -> L10n.invitation_response_accepted(ctx)
    ResponseStatus.DECLINED -> L10n.invitation_response_declined(ctx)
    ResponseStatus.TENTATIVE -> L10n.invitation_response_tentative(ctx)
    ResponseStatus.DELEGATED -> L10n.invitation_response_delegated(ctx)
}

/**
 * What else is in the user's calendar over the meeting's window, **in words**.
 *
 * The count is stated rather than left to the preview grid: docs/calendar.md §4, a picture the user
 * has to read carefully is not a disclosure. Zero and one get their own wording because "0 other
 * things" and "1 other things" are not sentences.
 *
 * [known] `false` is **not** zero. It means the core could not read the calendar over this window:
 * on a cold start mail syncs before calendars, so an invitation opened straight away lands here:
 * and "Nothing else in your calendar then" would then be a confident lie over a calendar nobody
 * read. Same rule as the grid's `isMaterialized`.
 */
internal fun invitationConflictLine(ctx: Context, count: UInt, known: Boolean): String {
    if (!known) return L10n.invitation_conflicts_unknown(ctx)
    return when (count) {
        0u -> L10n.invitation_conflicts_none(ctx)
        1u -> L10n.invitation_conflicts_one(ctx)
        else -> L10n.invitation_conflicts(ctx, count.toInt())
    }
}

/**
 * How the invitation's attendees have answered, as the phrases to join.
 *
 * Counts only, never a roster: the addresses belong to other people and are attacker-controlled
 * (docs/invitations.md). Every non-zero bucket earns a phrase, because the four sum to the total and
 * a line that leaves one out reads as arithmetic that does not add up. An invitation whose only
 * attendee is this account says so instead of "1 of 1 accepted".
 */
internal fun invitationAttendeeLines(ctx: Context, tally: AttendeeTally): List<String> {
    if (tally.total == 0u) return emptyList()
    if (tally.total == 1u) return listOf(L10n.invitation_attendees_one(ctx))
    val lines = mutableListOf(
        L10n.invitation_attendees(ctx, tally.accepted.toString(), tally.total.toString()),
    )
    // One is its own string, per count, because the catalog has no plural machinery and Dutch
    // needs a different verb: "1 moeten nog antwoorden" is wrong, and one outstanding reply is the
    // commonest case on a small meeting. English reads fine either way, which is exactly why this
    // was invisible until the card was looked at in Dutch. Same shape as the conflict count above.
    if (tally.tentative > 0u) {
        lines += if (tally.tentative == 1u) {
            L10n.invitation_attendees_tentative_one(ctx)
        } else {
            L10n.invitation_attendees_tentative(ctx, tally.tentative.toInt())
        }
    }
    if (tally.declined > 0u) {
        lines += if (tally.declined == 1u) {
            L10n.invitation_attendees_declined_one(ctx)
        } else {
            L10n.invitation_attendees_declined(ctx, tally.declined.toInt())
        }
    }
    if (tally.needsAction > 0u) {
        lines += if (tally.needsAction == 1u) {
            L10n.invitation_attendees_pending_one(ctx)
        } else {
            L10n.invitation_attendees_pending(ctx, tally.needsAction.toInt())
        }
    }
    return lines
}

/**
 * The meeting's "when", localised in [zone].
 *
 * All-day shows the inclusive day(s), the stored end is exclusive, so a one-day event whose end is
 * the next midnight must read as one date, not two. A timed meeting collapses the date when start
 * and end share one. The clock honours the user's 12/24-hour **setting** rather than the locale's
 * default, so mail and calendar cannot disagree with each other.
 *
 * Pure: no clock is read, so the same inputs always give the same string.
 */
internal fun invitationWhen(
    startsAt: String,
    endsAt: String,
    allDay: Boolean,
    zone: String,
    use24Hour: Boolean,
    locale: Locale,
): String {
    val start = parseUtcInstant(startsAt)?.atZone(resolveZoneId(zone)) ?: return ""
    val end = parseUtcInstant(endsAt)?.atZone(start.zone) ?: start
    val day = DateTimeFormatter.ofLocalizedDate(FormatStyle.FULL).withLocale(locale)

    if (allDay) {
        // The stored end is exclusive; name the inclusive last day.
        val lastDay = end.toLocalDate().minusDays(1)
        return if (lastDay <= start.toLocalDate()) {
            start.toLocalDate().format(day)
        } else {
            "${start.toLocalDate().format(day)} – ${lastDay.format(day)}"
        }
    }

    val from = clockTime(start.hour * 60 + start.minute, use24Hour)
    val to = clockTime(end.hour * 60 + end.minute, use24Hour)
    return if (start.toLocalDate() == end.toLocalDate()) {
        "${start.toLocalDate().format(day)}, $from – $to"
    } else {
        "${start.toLocalDate().format(day)} $from – ${end.toLocalDate().format(day)} $to"
    }
}

/**
 * The hour band the meeting-day preview draws: `first until last`, in whole hours.
 *
 * **The meeting, everything it clashes with, and an hour of air**, padded a whole hour each side
 * so nothing sits flush against an edge, and never narrower than [minimumHours] so a 30-minute
 * meeting on an empty afternoon still has context around it.
 *
 * It used to span the **whole day's** blocks, and that is the change: a normal working day runs
 * 08:00-22:00, so fourteen hours were squeezed into the preview's box and an hour came out under
 * ten dp, shorter than the title line, so the invitation's own block drew as an *unnamed*
 * rectangle beside a named one. A picture that shows *that* the afternoon is taken but not *by
 * what* answers the wrong question: the reader's next move is deciding whether the clash matters,
 * and they cannot without the title. Growing the box instead pushed the message itself off the
 * screen, which is worse, so the band narrows and the hours get taller.
 *
 * **Nothing that the card counts can fall outside this.** A conflict is by definition an event
 * overlapping the meeting's own window, so every one of them is in `clashing` and its *whole*
 * extent is inside the band, a long booking that starts before the meeting drags `first` back
 * with it rather than being drawn cut off at the top edge with its title off-screen. What is left
 * out is the rest of the day, which the card states in words above the grid, and which the
 * disclosure label names (`invitation_conflicts_preview`: "Around this meeting", not "that day").
 * docs/calendar.md §4, nothing is hidden without saying so; this says so.
 */
internal fun invitationPreviewSpan(
    meeting: MinuteSpan,
    others: List<MinuteSpan>,
    minimumHours: Int = 6,
): IntRange {
    // Half-open on both sides, exactly as `count_conflicts` overlaps in the core: back-to-back is
    // not a clash, so an event ending as the meeting starts does not widen the band.
    val clashing = others.filter { it.start < meeting.end && meeting.start < it.end }
    val spans = clashing + meeting
    val earliest = spans.minOf { it.start }
    val latest = spans.maxOf { it.end }
    var first = (earliest / 60 - 1).coerceAtLeast(0)
    // Ceil, so a block ending at 09:15 keeps the whole 09:00 hour, then pad.
    var last = ((latest + 59) / 60 + 1).coerceAtMost(HOURS_IN_DAY)
    // Alternating, later hour first, so the meeting sits near the middle of the band rather than
    // pinned to its top, the hours after a meeting are the more interesting of the two.
    var growAfter = true
    while (last - first < minimumHours && (first > 0 || last < HOURS_IN_DAY)) {
        if (growAfter && last < HOURS_IN_DAY) last++ else if (first > 0) first-- else last++
        growAfter = !growAfter
    }
    return first until last
}

/**
 * How many hours apart the preview's labelled gridlines sit.
 *
 * A squeezed span leaves no room to label every hour, two labels overlapping is worse than
 * three-hourly ones, so the stride is derived from the height a single hour actually gets.
 */
internal fun invitationPreviewStride(hourHeightDp: Float): Int {
    if (hourHeightDp <= 0f) return 1
    return ceil(PREVIEW_LABEL_HEIGHT_DP / hourHeightDp).toInt().coerceAtLeast(1)
}

/** The vertical room one preview hour label occupies, in dp, the stride's whole justification. */
private const val PREVIEW_LABEL_HEIGHT_DP = 12f

/**
 * How tall the meeting-day preview draws, in dp, for a band of [hours].
 *
 * **Normally just [PREVIEW_HEIGHT_DP]**, the band is narrow now ([invitationPreviewSpan] shows the
 * meeting and its clashes, not the whole day), so at six hours an hour already gets 22 dp and there
 * is nothing to fix. This exists for the case the band *cannot* be narrow: an all-morning booking
 * the meeting sits inside drags the band out to ten or twelve hours, and at a fixed height the
 * blocks around it would go back to being unnamed rectangles. So an hour is allowed
 * [PREVIEW_IDEAL_HOUR_DP] and the box grows, up to [PREVIEW_MAX_HEIGHT_DP], past which this stops
 * being a preview sitting above a message and starts pushing the message off the screen.
 *
 * Beyond that cap the hour height falls back below the ideal and short blocks quietly lose their
 * titles. That is the correct trade and not a hole in the rule above: nothing is ever *clipped*,
 * only unlabelled, and every block keeps its spoken label (docs/calendar.md §4).
 *
 * The three numbers are *layout*, and a platform may hold its own; the formula is the rule, and it
 * is the same in InvitationFormat.swift and InvitationFormat.cs.
 */
internal fun invitationPreviewHeightDp(hours: Int): Float =
    (maxOf(hours, 1) * PREVIEW_IDEAL_HOUR_DP)
        .coerceAtLeast(PREVIEW_HEIGHT_DP)
        .coerceAtMost(PREVIEW_MAX_HEIGHT_DP)

/** The height one hour wants: enough room for a 60-minute block's 11 sp title plus its insets. */
private const val PREVIEW_IDEAL_HOUR_DP = 20f

/** What the preview normally is, short enough that the message body is still the thing on screen. */
private const val PREVIEW_HEIGHT_DP = 132f

/** The ceiling, for a band a long booking forced wide. A preview taller than this stops being one. */
private const val PREVIEW_MAX_HEIGHT_DP = 240f

/**
 * The spoken label for a calendar record, with the unanswered-hold disclosure appended.
 *
 * The dashed border and hatched gutter that mark an unanswered invitation are **invisible to a
 * screen reader**, so the label has to say it, docs/calendar.md §4, the spoken-grid rule. Shared by
 * the grid block, the all-day bar, the month chip and the agenda row so one rule covers every
 * surface that can show a hold.
 */
internal fun calendarEventLabel(
    ctx: Context,
    title: String,
    time: String,
    calendar: String,
    participation: ResponseStatus,
): String {
    val base = L10n.calendar_event_a11y(ctx, title, time, calendar)
    if (!isAwaitingResponse(participation)) return base
    return "$base, ${L10n.a11y_invitation_awaiting_response(ctx)}"
}

/**
 * Whether a calendar record is an invitation this account has not answered, the one condition that
 * turns on the provisional drawing (dashed border, hatched gutter, reduced fill).
 *
 * `DECLINED` never reaches a client: the core hides those from every calendar surface.
 */
internal fun isAwaitingResponse(participation: ResponseStatus): Boolean =
    participation == ResponseStatus.NEEDS_ACTION

/**
 * The meeting's UTC instants as wall-clock minutes from midnight in [zone].
 *
 * Returns a one-hour span at midnight for an instant that will not parse, the preview then draws
 * the day it was given rather than nothing at all, which is the same best-effort posture the core
 * takes when it cannot resolve a conflict window.
 */
internal fun meetingMinuteSpan(startsAt: String, endsAt: String, zone: String): MinuteSpan {
    val zoneId = resolveZoneId(zone)
    val start = parseUtcInstant(startsAt)?.atZone(zoneId) ?: return MinuteSpan(0, 60)
    val end = parseUtcInstant(endsAt)?.atZone(zoneId) ?: start
    val startMinutes = start.hour * 60 + start.minute
    // An end past midnight, or on a later day, belongs to the end of this day's grid.
    val endMinutes = if (start.toLocalDate() == end.toLocalDate()) {
        end.hour * 60 + end.minute
    } else {
        HOURS_IN_DAY * 60
    }
    return MinuteSpan(startMinutes, maxOf(endMinutes, startMinutes + 1))
}

/**
 * What to say about an answer that is on its way out, or null when there is nothing to say.
 *
 * `SAVED` and `IDLE` both return null on purpose: the card already shows the new answer by then
 * (it re-reads the calendar), so a second "Answer sent" line would be noise. `FAILED` is the one
 * that must never be silent, a reply the organiser never received, with the card quietly showing
 * the old answer, is exactly the failure the whole feature exists to prevent.
 */
internal fun invitationWriteLine(ctx: Context, status: CalendarWriteStatus): String? = when (status) {
    CalendarWriteStatus.SAVING -> L10n.invitation_sending(ctx)
    CalendarWriteStatus.FAILED -> L10n.invitation_failed(ctx)
    CalendarWriteStatus.SAVED, CalendarWriteStatus.IDLE -> null
}

/** The core's RFC 3339 UTC instant, or null when it will not parse. */
private fun parseUtcInstant(raw: String): Instant? = try {
    Instant.parse(raw)
} catch (_: DateTimeParseException) {
    null
}

/** An IANA zone id the device knows, falling back to its own rather than throwing at draw time. */
private fun resolveZoneId(zone: String): ZoneId = try {
    if (zone.isEmpty()) ZoneId.systemDefault() else ZoneId.of(zone)
} catch (_: DateTimeException) {
    ZoneId.systemDefault()
}
