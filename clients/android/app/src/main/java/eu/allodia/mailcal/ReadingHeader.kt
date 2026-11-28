// The reading header: the subject as the page's title, never cut short, then who sent it, when,
// and to whom, by name. Every address and the full date are one tap away, in the details, and the
// sender's own address is always shown, because it is what tells a real sender from one borrowing
// the name (docs/reading-actions.md, docs/timestamps.md, docs/avatars.md).
//
// Names and addresses come from the snapshot's structured fields; the joined `from`/`to` strings
// are never split here.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.MailPerson
import uniffi.mailcal_bindings.ReadingSnapshot

internal const val READING_DETAILS_TOGGLE_TAG = "reading-details-toggle"
internal const val READING_DETAILS_TAG = "reading-details"

/** The snapshot that describes `message`, once it carries a header: not a stale or pending one. */
internal fun loadedHeader(message: OpenedMessage, reading: ReadingSnapshot?): ReadingSnapshot? =
    reading?.takeIf { it.key == message.key && !it.pending && !it.loadError }

/** The sender by name: the loaded header's, else its address, else the row's while it loads. */
internal fun readingSenderName(message: OpenedMessage, header: ReadingSnapshot?): String =
    header?.fromName?.ifEmpty { null } ?: header?.fromAddress?.ifEmpty { null } ?: message.from

/** The sender's address, when the name line is not already it. */
internal fun readingSenderAddress(header: ReadingSnapshot?): String? =
    header?.takeIf { it.fromName.isNotEmpty() && it.fromAddress.isNotEmpty() }?.fromAddress

/** "To: Ada, Bob   Cc: Carol": names only, a person's address where they have none. */
internal fun recipientSummary(ctx: Context, header: ReadingSnapshot): String =
    listOf(
        L10n.compose_to(ctx) to header.toPeople,
        L10n.compose_cc(ctx) to header.ccPeople,
        L10n.compose_bcc(ctx) to header.bccPeople,
    )
        .filter { (_, people) -> people.isNotEmpty() }
        .joinToString("   ") { (label, people) ->
            "$label: " + people.joinToString(", ") { it.name.ifEmpty { it.address } }
        }

/** The subject as the page's title, in full. */
@Composable
internal fun ReadingSubject(message: OpenedMessage, reading: ReadingSnapshot?) {
    val ctx = LocalContext.current
    SelectionContainer {
        Text(
            text = displayedSubject(message, reading).ifEmpty { L10n.mail_no_subject(ctx) },
            style = MaterialTheme.typography.headlineSmall,
            modifier = Modifier
                .fillMaxWidth()
                .padding(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 12.dp)
                // The page's title, so TalkBack's heading navigation lands on it.
                .semantics { heading() }
                .testTag("reading-subject"),
        )
    }
}

/**
 * The sender's avatar, name and the list's relative date on one line, the address under it, and
 * the line naming the recipients that opens and closes the details.
 */
@Composable
internal fun ReadingSender(message: OpenedMessage, reading: ReadingSnapshot?, activeZoneId: String?) {
    val ctx = LocalContext.current
    val use24Hour = LocalUse24Hour.current
    val header = loadedHeader(message, reading)
    // Closed on each open, like the remote-images choice.
    var detailsOpen by rememberSaveable(message.key) { mutableStateOf(false) }
    Column(modifier = Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 12.dp)) {
        Row(verticalAlignment = Alignment.Top) {
            AvatarView(
                avatar = reading?.takeIf { it.key == message.key }?.avatar ?: message.avatar,
                modifier = Modifier.testTag("reading-avatar"),
            )
            Spacer(modifier = Modifier.width(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        text = readingSenderName(message, header),
                        style = MaterialTheme.typography.titleSmall,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                    Spacer(modifier = Modifier.width(8.dp))
                    // The list's own label; the full date is in the details (docs/timestamps.md).
                    Text(
                        text = relativeDate(message.date, activeZoneId, use24Hour),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                readingSenderAddress(header)?.let { address ->
                    SelectionContainer {
                        Text(
                            text = address,
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }
                if (header != null) {
                    DetailsToggle(
                        summary = recipientSummary(ctx, header),
                        open = detailsOpen,
                        onToggle = { detailsOpen = !detailsOpen },
                    )
                }
            }
        }
        if (header != null) {
            AnimatedVisibility(visible = detailsOpen) {
                MessageDetails(header, localDateTime(message.date, activeZoneId, use24Hour))
            }
        }
    }
}

/**
 * One line naming the recipients, cut short when long: everything it leaves out is what it opens.
 * Who received it is what TalkBack reads; what a press does is the click label.
 */
@Composable
private fun DetailsToggle(summary: String, open: Boolean, onToggle: () -> Unit) {
    val ctx = LocalContext.current
    val hint = if (open) L10n.a11y_hide_message_details(ctx) else L10n.a11y_show_message_details(ctx)
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .heightIn(min = 48.dp)
            .clickable(onClickLabel = hint, role = Role.Button, onClick = onToggle)
            .semantics { contentDescription = summary.ifEmpty { hint } }
            .testTag(READING_DETAILS_TOGGLE_TAG),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(
            // A message with no recipient the header names still has a sender and a date to show.
            text = summary.ifEmpty { hint },
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
            // Said once, by the row's own description, rather than again as the text it holds.
            modifier = Modifier.weight(1f, fill = false).clearAndSetSemantics {},
        )
        Spacer(modifier = Modifier.width(4.dp))
        Icon(
            painter = painterResource(R.drawable.ic_keyboard_arrow_down),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.size(18.dp).rotate(if (open) 180f else 0f),
        )
    }
}

/** Every person with their address, and the full date under the label a printout gives it. */
@Composable
private fun MessageDetails(header: ReadingSnapshot, date: String) {
    val ctx = LocalContext.current
    val rows = listOf(
        L10n.compose_from(ctx) to listOf(MailPerson(header.fromName, header.fromAddress)),
        L10n.compose_to(ctx) to header.toPeople,
        L10n.compose_cc(ctx) to header.ccPeople,
        L10n.compose_bcc(ctx) to header.bccPeople,
    ).map { (label, people) -> label to people.filter { it.name.isNotEmpty() || it.address.isNotEmpty() } }
        .filter { (_, people) -> people.isNotEmpty() }
    SelectionContainer {
        LabelledRows(
            modifier = Modifier
                .padding(top = 4.dp)
                .fillMaxWidth()
                .background(MaterialTheme.colorScheme.surfaceContainerHigh, RoundedCornerShape(12.dp))
                // A long Cc list scrolls inside the box rather than pushing the message off screen.
                .heightIn(max = 280.dp)
                .verticalScroll(rememberScrollState())
                .padding(12.dp)
                .testTag(READING_DETAILS_TAG),
        ) {
            rows.forEach { (label, people) ->
                DetailLabel(label)
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    people.forEach { person ->
                        Column {
                            Text(person.name.ifEmpty { person.address }, style = MaterialTheme.typography.bodySmall)
                            if (person.name.isNotEmpty()) {
                                Text(
                                    person.address,
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }
                }
            }
            DetailLabel(L10n.quote_sent(ctx))
            Text(date, style = MaterialTheme.typography.bodySmall)
        }
    }
}

@Composable
private fun DetailLabel(label: String) {
    Text(label, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
}

/**
 * A two-column grid: the children alternate label and value, and every label takes the width of the
 * widest, so the values start in one column whichever language names the rows.
 */
@Composable
private fun LabelledRows(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
    Layout(content = content, modifier = modifier) { measurables, constraints ->
        val gap = 12.dp.roundToPx()
        val rowGap = 10.dp.roundToPx()
        val loose = Constraints(maxWidth = constraints.maxWidth)
        val labels = measurables.filterIndexed { i, _ -> i % 2 == 0 }.map { it.measure(loose) }
        val labelWidth = labels.maxOfOrNull { it.width } ?: 0
        val valueWidth = (constraints.maxWidth - labelWidth - gap).coerceAtLeast(0)
        val values = measurables.filterIndexed { i, _ -> i % 2 == 1 }
            .map { it.measure(Constraints(maxWidth = valueWidth)) }
        val heights = labels.indices.map { maxOf(labels[it].height, values.getOrNull(it)?.height ?: 0) }
        val height = heights.sum() + rowGap * (heights.size - 1).coerceAtLeast(0)
        layout(constraints.maxWidth, height) {
            var y = 0
            labels.forEachIndexed { i, label ->
                // End-aligned, so each label sits against the value it names.
                label.placeRelative(labelWidth - label.width, y)
                values.getOrNull(i)?.placeRelative(labelWidth + gap, y)
                y += heights[i] + rowGap
            }
        }
    }
}
