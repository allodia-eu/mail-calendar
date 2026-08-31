// The meeting-invitation card, drawn above the message body.
//
// Everything on it was decided by the core (docs/invitations.md): whether there is a card at all,
// the organiser line, the attendee tally, the conflict count, and the meeting-day preview's
// geometry. This file localises and arranges; it computes no counts of its own, so this client and
// the next cannot disagree about whether a meeting clashes.
//
// SECURITY (Gate 8, docs/rendering-security.md), the summary, location, description and organizer
// name are attacker-controlled sender content, and they reach the screen without passing the HTML
// sanitiser, the CSP or a WebView. Compose's `Text(String)` renders them as text and nothing else:
// styling on Android requires an `AnnotatedString`, which a plain `String` can never become by
// accident, so there is no markdown/markup path to fall into here. (The equivalent traps elsewhere
// are SwiftUI's `LocalizedStringKey` overload and GTK's `use_markup(true)`.) Nothing on this card is
// ever passed to `HtmlCompat.fromHtml`, `buildAnnotatedString` or a WebView.
//
// The conflict count is stated in WORDS beside the preview grid, always, docs/calendar.md §4: a
// picture the user has to read carefully is not a disclosure.
package eu.allodia.mailcal

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextMeasurer
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.drawText
import androidx.compose.ui.text.rememberTextMeasurer
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import java.util.Locale
import uniffi.mailcal_bindings.CalendarRow
import uniffi.mailcal_bindings.CalendarWriteStatus
import uniffi.mailcal_bindings.InvitationCard
import uniffi.mailcal_bindings.InvitationKind
import uniffi.mailcal_bindings.InvitationPreview
import uniffi.mailcal_bindings.InvitationResponse
import uniffi.mailcal_bindings.ReadingSnapshot
import uniffi.mailcal_bindings.Swatch

// The preview carries no calendar list, it is one already-loaded day, not a page, so every block
// falls back to the neutral swatch. The preview is about *when*, not about which calendar.
private val NO_CALENDARS = emptyList<CalendarRow>()

private val CARD_CORNER = 8.dp
private val PREVIEW_GUTTER = 30.dp

/**
 * The card's slot in the reading screen: the card when the open message is an invitation, and
 * nothing at all otherwise.
 *
 * The decision is entirely the core's, its two-condition RSVP gate (a scheduling `METHOD` **and** an
 * `ATTENDEE` matching one of this account's own addresses, docs/invitations.md), so a published
 * `.ics` produces no card here and keeps its attachment chip instead. A stale snapshot for a
 * previously-opened message is already filtered out upstream.
 */
@Composable
internal fun InvitationBanner(
    snapshot: ReadingSnapshot?,
    activeZoneId: String?,
    writeStatus: CalendarWriteStatus,
    onRespond: (InvitationResponse, String?, Boolean, String) -> Unit,
) {
    val card = snapshot?.invitation ?: return
    InvitationCardView(
        card = card,
        zone = activeZoneId.orEmpty(),
        writeStatus = writeStatus,
        onRespond = onRespond,
    )
}

@Composable
private fun InvitationCardView(
    card: InvitationCard,
    zone: String,
    writeStatus: CalendarWriteStatus,
    onRespond: (InvitationResponse, String?, Boolean, String) -> Unit,
) {
    val ctx = LocalContext.current
    val configuration = LocalConfiguration.current
    val locale = remember(configuration) {
        configuration.locales.takeIf { !it.isEmpty }?.get(0) ?: Locale.getDefault()
    }
    val use24Hour = LocalUse24Hour.current
    val tint = invitationTint(card.kind)
    // Open whenever the calendar was actually read.
    //
    // It used to open only when the count was non-zero, "there is nothing to see, so save the
    // room". That is wrong about what the grid is FOR: the question a person answering an invitation
    // is asking is "what does my day look like", and the answer is the picture, not the number.
    // "Nothing else in your calendar then" over a drawn, visibly empty day is a STRONGER answer than
    // the same words over a collapsed row.
    //
    // Still gated on conflictsKnown: an empty grid drawn over a calendar we have not read looks
    // identical to a free day, which is the one thing this must never say (docs/calendar.md §4).
    var showPreview by remember(card.startsAt, card.summary) {
        mutableStateOf(card.conflictsKnown)
    }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 12.dp, vertical = 6.dp)
            .clip(RoundedCornerShape(CARD_CORNER))
            .background(tint.copy(alpha = 0.08f))
            .border(1.dp, tint.copy(alpha = 0.3f), RoundedCornerShape(CARD_CORNER))
            .padding(10.dp)
            .semantics { contentDescription = L10n.a11y_invitation_card(ctx, card.organizer) },
        verticalArrangement = Arrangement.spacedBy(4.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Icon(
                painter = painterResource(R.drawable.ic_calendar_month),
                contentDescription = null,
                tint = tint,
            )
            Spacer(modifier = Modifier.width(6.dp))
            Text(
                text = invitationTitle(ctx, card.kind),
                style = MaterialTheme.typography.labelLarge,
                color = tint,
            )
        }
        invitationNotice(ctx, card.kind)?.let { notice ->
            Text(
                text = notice,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Text(
            text = card.summary.ifEmpty { L10n.invitation_no_title(ctx) },
            style = MaterialTheme.typography.titleSmall,
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
        )
        DetailLine(L10n.invitation_organizer(ctx), card.organizer)
        DetailLine(
            L10n.invitation_when(ctx),
            invitationWhen(
                startsAt = card.startsAt,
                endsAt = card.endsAt,
                allDay = card.allDay,
                zone = zone,
                use24Hour = use24Hour,
                locale = locale,
            ),
        )
        if (card.location.isNotEmpty()) DetailLine(L10n.invitation_where(ctx), card.location)
        if (card.recurring) {
            Text(
                text = L10n.invitation_repeats(ctx),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        // The organiser's notes. Already truncated by the core (Gmail sends a wall of filler), and
        // the card says so rather than implying the text ends there.
        if (card.description.isNotEmpty()) {
            Text(
                text = card.description,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 4,
                overflow = TextOverflow.Ellipsis,
            )
            if (card.descriptionTruncated) {
                Text(
                    text = L10n.invitation_description_shortened(ctx),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        // This account's own answer, and how everyone else answered, both read from the
        // *calendar's* copy where there is one, never from the invitation email, which is frozen
        // at the moment it was sent and would keep saying "you haven't answered" after you had.
        Text(
            text = invitationResponseLine(ctx, card.myResponse),
            style = MaterialTheme.typography.bodySmall,
        )
        val attendees = invitationAttendeeLines(ctx, card.attendees)
        if (attendees.isNotEmpty()) {
            Text(
                text = attendees.joinToString(" · "),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        // Only an RSVP card has anything to answer: a cancellation and a plain notice do not.
        if (card.kind == InvitationKind.RSVP) {
            InvitationRespond(card = card, status = writeStatus, onRespond = onRespond)
        }
        // What else is in the calendar then, stated in words, then shown. The preview is offered
        // only when the calendar was actually read: an empty grid drawn over an unread calendar
        // looks exactly like a free day, which is the whole failure this guards.
        Text(
            text = invitationConflictLine(ctx, card.conflictCount, card.conflictsKnown),
            style = MaterialTheme.typography.bodySmall,
            color = if (card.conflictsKnown && card.conflictCount > 0u) {
                MaterialTheme.colorScheme.onSurface
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant
            },
        )
        if (card.conflictsKnown) {
            Text(
                text = L10n.invitation_conflicts_preview(ctx),
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier
                    .fillMaxWidth()
                    .clickable { showPreview = !showPreview }
                    .padding(vertical = 4.dp),
            )
            if (showPreview) {
                InvitationPreviewGrid(
                    preview = card.preview,
                    meeting = meetingMinuteSpan(
                        startsAt = card.startsAt,
                        endsAt = card.endsAt,
                        // The layout zone the core solved the day in; the display zone only when it
                        // did not say. Reading the meeting in a different zone from the blocks
                        // beside it would put it in the wrong row of its own preview.
                        zone = card.preview.timezone.ifEmpty { zone },
                    ),
                    use24Hour = use24Hour,
                )
            }
        }
    }
}

/** A cancellation is the one kind that has to be unmissable, a stale hold otherwise sits in the
 *  calendar looking like a commitment. */
@Composable
private fun invitationTint(kind: InvitationKind): Color = when (kind) {
    InvitationKind.RSVP -> MaterialTheme.colorScheme.primary
    InvitationKind.CANCELLED -> MaterialTheme.colorScheme.error
    InvitationKind.INFORMATIONAL -> MaterialTheme.colorScheme.onSurfaceVariant
    // Tertiary rather than error: nothing was lost, there is simply a newer copy to open.
    InvitationKind.SUPERSEDED -> MaterialTheme.colorScheme.tertiary
}

@Composable
private fun DetailLine(label: String, value: String) {
    Row(modifier = Modifier.fillMaxWidth()) {
        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.width(72.dp),
        )
        Text(text = value, style = MaterialTheme.typography.bodySmall)
    }
}

/**
 * The meeting-day preview: one day of the user's own calendar.
 *
 * Laid out by the same `calendar::grid::build` every calendar surface uses, so the preview and the
 * real grid cannot disagree, and an unanswered hold is dashed here for the same reason it is dashed
 * there.
 *
 * The hour height is derived from the span rather than fixed, so every block on that day fits: the
 * preview never clips, which is what lets it stay a picture with no "and 2 more" caveat.
 */
@Composable
private fun InvitationPreviewGrid(
    preview: InvitationPreview,
    meeting: MinuteSpan,
    use24Hour: Boolean,
) {
    val ctx = LocalContext.current
    val measurer = rememberTextMeasurer()
    val dark = LocalAppDark.current
    val outline = MaterialTheme.colorScheme.outlineVariant
    val labelStyle = MaterialTheme.typography.labelSmall.copy(
        fontSize = 9.sp,
        lineHeight = 11.sp,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    val noTitle = L10n.event_no_title(ctx)

    val span = invitationPreviewSpan(
        meeting = meeting,
        others = preview.timed.map {
            MinuteSpan(it.startMinutes.toInt(), it.endMinutes.toInt())
        },
    )
    val spanHours = span.last - span.first + 1

    Column(modifier = Modifier.fillMaxWidth()) {
        // One day, so a bar spans the full width and the banner is as tall as the core's lane count.
        // No "+N" overflow: a single day's all-day events fit, and capping them here would hide
        // something.
        if (preview.allDayLanes > 0u) {
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(start = PREVIEW_GUTTER)
                    .height(LANE_HEIGHT * preview.allDayLanes.toInt())
                    .drawBehind {
                        val laneHeight = size.height / preview.allDayLanes.toInt()
                        preview.allDay.forEach { band ->
                            val top = laneHeight * band.lane.toInt()
                            drawPreviewChip(
                                rect = Rect(0f, top, size.width, top + laneHeight),
                                swatch = NO_CALENDARS
                                    .rowFor(band.account, band.calendar)
                                    .swatchOrFallback(dark),
                                text = band.title.ifEmpty { noTitle },
                                style = labelStyle,
                                measurer = measurer,
                                awaiting = isAwaitingResponse(band.participation),
                            )
                        }
                    },
            )
        }
        Box(
            modifier = Modifier
                .fillMaxWidth()
                // Tall enough that the meeting's own block can carry its title, see
                // invitationPreviewHeightDp, which is why this is derived from the span.
                .height(invitationPreviewHeightDp(spanHours).dp)
                .drawBehind {
                    val gutter = PREVIEW_GUTTER.toPx()
                    val hourHeight = size.height / spanHours
                    drawPreviewRuler(span, hourHeight, gutter, use24Hour, labelStyle, measurer)
                    val dayWidth = size.width - gutter
                    clipRect(gutter, 0f, size.width, size.height) {
                        // The blocks position themselves from midnight, as they do on the real grid;
                        // the whole day is laid out and slid up so the span starts at the top. Same
                        // multiplication, no second solver.
                        translate(left = gutter, top = -hourHeight * span.first) {
                            for (hour in span.first..span.last + 1) {
                                val y = hourHeight * hour
                                drawLine(outline, Offset(0f, y), Offset(dayWidth, y), 1f)
                            }
                            preview.timed.forEach { segment ->
                                val columns = segment.columns.toInt().coerceAtLeast(1)
                                val columnWidth = dayWidth / columns
                                val left = columnWidth * segment.column.toInt()
                                drawPreviewChip(
                                    rect = Rect(
                                        left = left,
                                        top = hourHeight * (segment.startMinutes.toInt() / 60f),
                                        right = left + columnWidth,
                                        bottom = hourHeight * (segment.endMinutes.toInt() / 60f),
                                    ),
                                    swatch = NO_CALENDARS
                                        .rowFor(segment.account, segment.calendar)
                                        .swatchOrFallback(dark),
                                    text = segment.title.ifEmpty { noTitle },
                                    style = labelStyle,
                                    measurer = measurer,
                                    awaiting = isAwaitingResponse(segment.participation),
                                )
                            }
                        }
                    }
                },
        )
    }
}

/** The hour labels down the left edge, every [invitationPreviewStride] hours. */
@Suppress("LongParameterList")
private fun DrawScope.drawPreviewRuler(
    span: IntRange,
    hourHeight: Float,
    gutter: Float,
    use24Hour: Boolean,
    style: TextStyle,
    measurer: TextMeasurer,
) {
    val stride = invitationPreviewStride(hourHeight / density)
    clipRect(0f, 0f, gutter, size.height) {
        for (hour in span) {
            if ((hour - span.first) % stride != 0) continue
            val text = hourLabel(hour, use24Hour)
            if (text.isEmpty()) continue
            val line = measurer.measure(text, style)
            drawText(
                line,
                topLeft = Offset(
                    gutter - line.size.width - 4.dp.toPx(),
                    // A label straddles its own gridline, as the full grid's ruler does, except the
                    // first, which has no line above it and would hang off the top.
                    (hourHeight * (hour - span.first) - line.size.height / 2f).coerceAtLeast(0f),
                ),
            )
        }
    }
}

/** One preview block or all-day bar: a rounded fill, the hold treatment, then a clipped title. */
@Suppress("LongParameterList")
private fun DrawScope.drawPreviewChip(
    rect: Rect,
    swatch: Swatch,
    text: String,
    style: TextStyle,
    measurer: TextMeasurer,
    awaiting: Boolean,
) {
    val pad = 1.dp.toPx()
    val inner = Rect(rect.left + pad, rect.top + pad, rect.right - pad, rect.bottom - pad)
    if (inner.width <= 0f || inner.height <= 0f) return
    val corner = CORNER_RADIUS.toPx()
    val edge = parseHexColor(swatch.border)
    drawRoundRect(
        color = parseHexColor(swatch.background).holdFill(awaiting),
        topLeft = Offset(inner.left, inner.top),
        size = Size(inner.width, inner.height),
        cornerRadius = CornerRadius(corner),
    )
    drawHold(inner, edge, corner, awaiting)
    val room = inner.width - 8.dp.toPx()
    if (room <= 0f) return
    val line = measurer.measure(
        text = text,
        style = style.copy(color = parseHexColor(swatch.text)),
        overflow = TextOverflow.Ellipsis,
        softWrap = false,
        maxLines = 1,
        constraints = Constraints(maxWidth = room.toInt()),
    )
    // A block too short for its own title gets none, rather than one sliced through the middle:
    // the same rule the full grid applies at a low zoom.
    if (line.size.height > inner.height) return
    clipRect(inner.left, inner.top, inner.right, inner.bottom) {
        drawText(line, topLeft = Offset(inner.left + 4.dp.toPx(), inner.top + pad))
    }
}
