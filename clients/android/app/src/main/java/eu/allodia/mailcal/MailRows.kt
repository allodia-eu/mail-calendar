// The mailbox-list row composables for the Android client: the flat message row (with its
// read/flag/overflow affordances) and the threaded conversation row. The swipe-to-act gesture
// that wraps a flat row is in MailRowsSwipe.kt.
package eu.allodia.mailcal

import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Badge
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.FlatRow
import uniffi.mailcal_bindings.ComposerFileAttachment
import uniffi.mailcal_bindings.Recipients
import uniffi.mailcal_bindings.RecipientMatch
import uniffi.mailcal_bindings.RecipientSuggestion
import uniffi.mailcal_bindings.ThreadRow

// Amber for the flagged star, mirroring the macOS client's orange `flag.fill`. Only the filled
// star is vendored, so flagged state is shown by tint/alpha on the one glyph rather than by
// swapping to an outline variant (adding `star` at fill 0 would be one more ic_*.xml).
private val FlaggedAmber = Color(0xFFFFB300)


// Widened from private: called from MailRowsSwipe.kt's SwipeableFlatMessageRow.
@androidx.compose.runtime.Composable
internal fun FlatMessageRow(
    message: FlatRow,
    activeZoneId: String?,
    inJunkFolder: Boolean,
    accounts: List<AccountRow>,
    onOpen: (OpenedMessage) -> Unit,
    onSetRead: (account: String, key: String, read: Boolean) -> Unit,
    onSetFlagged: (account: String, key: String, flagged: Boolean) -> Unit,
    onDelete: (account: String, key: String) -> Unit,
    onPermanentlyDelete: (account: String, key: String) -> Unit,
    onMarkAsSpam: (account: String, key: String) -> Unit,
    onMarkAsNotSpam: (account: String, key: String) -> Unit,
    onReply: (
        account: String,
        key: String,
        from: String?,
        recipients: Recipients,
        documentJson: String,
        files: List<ComposerFileAttachment>,
    ) -> Boolean,
    onForward: (
        account: String,
        key: String,
        from: String?,
        recipients: Recipients,
        documentJson: String,
        files: List<ComposerFileAttachment>,
    ) -> Boolean,
    replyRecipients: (account: String, key: String, replyAll: Boolean) -> RecipientSuggestion?,
    suggestionsFor: ((String) -> List<RecipientMatch>)? = null,
    // The signature library + lookups for the reply/forward composer, or null to leave signatures
    // out (a screenshot run, a test).
    signatures: ComposerSignatures? = null,
) {
    val ctx = LocalContext.current
    // The open rich composer for this row (null = closed, else the active mode). One dialog
    // serves reply/reply-all/forward; the mode picks the pre-filled recipients and which rich
    // submit Send calls. State lives per-row (cf. FlatMessageOverflow's own `expanded`).
    var composing by remember { mutableStateOf<RichComposeMode?>(null) }
    // Read the ambient clock setting here, in the composable body: the `clickable` lambda below is
    // not a composition scope and cannot read a CompositionLocal.
    val use24Hour = LocalUse24Hour.current
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .background(MaterialTheme.colorScheme.surface)
            .clickable {
                onOpen(
                    OpenedMessage(
                        account = message.account,
                        key = message.key,
                        subject = message.subject,
                        from = message.from,
                        avatar = message.avatar,
                        // The reading header keeps the full date; only the row shows the short one.
                        date = localDateTime(message.date, activeZoneId, use24Hour),
                    ),
                )
            }
            .padding(start = 16.dp, end = 4.dp, top = 8.dp, bottom = 8.dp),
        verticalAlignment = Alignment.Top,
    ) {
        // This client is single-pane: the list is always the compact layout, where bold subject
        // and sender carry unread state and the contract reserves no separate dot gutter.
        AvatarView(message.avatar, modifier = Modifier.testTag("mail-avatar"))
        Spacer(modifier = Modifier.width(12.dp))
        Column(modifier = Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    text = message.subject.ifEmpty { L10n.mail_no_subject(ctx) },
                    style = MaterialTheme.typography.bodyLarge,
                    fontWeight = if (message.unread) FontWeight.Bold else FontWeight.Normal,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                // A small amber star marks flagged messages at a glance (cf. macOS `flag.fill`).
                if (message.flagged) {
                    Spacer(modifier = Modifier.width(6.dp))
                    Icon(
                        painter = painterResource(R.drawable.ic_star),
                        contentDescription = L10n.a11y_flagged(ctx),
                        tint = FlaggedAmber,
                        modifier = Modifier.size(15.dp),
                    )
                }
                // A paperclip marks messages with a non-inline attachment, after the flag:
                // matching macOS (SF Symbol "paperclip") and Windows (Segoe glyph E723).
                if (message.hasAttachment) {
                    Spacer(modifier = Modifier.width(6.dp))
                    Icon(
                        painter = painterResource(R.drawable.ic_attachment),
                        contentDescription = L10n.a11y_has_attachment(ctx),
                        tint = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.size(15.dp),
                    )
                }
                // A compact relative timestamp trailing the subject (today → time, this week →
                // weekday, else date), far narrower than the full date, so the title gets the room.
                Spacer(modifier = Modifier.width(8.dp))
                Text(
                    text = relativeDate(message.date, activeZoneId, LocalUse24Hour.current),
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                )
            }
            // Sender + a body preview snippet, so a truncated subject still has context.
            Text(
                text = senderAndPreview(message.from, message.preview, unread = message.unread),
                style = MaterialTheme.typography.bodySmall,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
        }
        // Overflow only (flag/read/reply/forward/trash/spam/delete) keeps the row width for the
        // title + preview; the flagged state still shows as the inline amber star above.
        FlatMessageOverflow(
            unread = message.unread,
            flagged = message.flagged,
            inJunkFolder = inJunkFolder,
            onToggleRead = { onSetRead(message.account, message.key, message.unread) },
            onToggleFlag = { onSetFlagged(message.account, message.key, !message.flagged) },
            onReply = { composing = RichComposeMode.Reply },
            onReplyAll = { composing = RichComposeMode.ReplyAll },
            onForward = { composing = RichComposeMode.Forward },
            onDelete = { onDelete(message.account, message.key) },
            onMarkAsSpam = { onMarkAsSpam(message.account, message.key) },
            onMarkAsNotSpam = { onMarkAsNotSpam(message.account, message.key) },
            onPermanentlyDelete = { onPermanentlyDelete(message.account, message.key) },
        )
    }

    // Rendered as an overlay (own window), so calling it after the Row is fine. Reply/reply-all/
    // forward open the SAME hardened rich composer as new mail; the editor's rendered document
    // rides `submitRichReply`/`submitRichForward`, which dismiss on success. Reply/reply-all open
    // with To/Cc pre-filled from the core.
    composing?.let { mode ->
        val prefill = remember(mode) {
            if (mode == RichComposeMode.Reply || mode == RichComposeMode.ReplyAll) {
                replyRecipients(message.account, message.key, mode == RichComposeMode.ReplyAll)
            } else {
                null
            }
        }
        RichComposeMessageDialog(
            suggestionsFor = suggestionsFor,
            signatures = signatures,
            mode = mode,
            accounts = accounts,
            // A reply/forward opens on the account that received the mail, the address it was
            // sent to. The user can switch it in the From dropdown.
            initialFrom = message.account,
            initialTo = prefill?.to ?: "",
            initialCc = prefill?.cc ?: "",
            onDismiss = { composing = null },
            onSubmitRich = { from, recipients, _, documentJson, files ->
                val sent = if (mode == RichComposeMode.Forward) {
                    onForward(message.account, message.key, from, recipients, documentJson, files)
                } else {
                    onReply(message.account, message.key, from, recipients, documentJson, files)
                }
                if (sent) {
                    composing = null
                }
                sent
            },
        )
    }
}

@androidx.compose.runtime.Composable
private fun FlatMessageOverflow(
    unread: Boolean,
    flagged: Boolean,
    inJunkFolder: Boolean,
    onToggleRead: () -> Unit,
    onToggleFlag: () -> Unit,
    onReply: () -> Unit,
    onReplyAll: () -> Unit,
    onForward: () -> Unit,
    onDelete: () -> Unit,
    onMarkAsSpam: () -> Unit,
    onMarkAsNotSpam: () -> Unit,
    onPermanentlyDelete: () -> Unit,
) {
    val ctx = LocalContext.current
    var expanded by remember { mutableStateOf(false) }
    Box {
        IconButton(onClick = { expanded = true }) {
            Icon(
                painter = painterResource(R.drawable.ic_more_vert),
                contentDescription = L10n.a11y_more_actions(ctx),
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        // Reply/Reply all/Forward live here as text items rather than as three more icons on the
        // row: a menu entry is the cleanest affordance and reuses the overflow already in the row.
        DropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            DropdownMenuItem(
                text = { Text(if (unread) L10n.action_mark_read(ctx) else L10n.action_mark_unread(ctx)) },
                onClick = {
                    expanded = false
                    onToggleRead()
                },
            )
            DropdownMenuItem(
                text = { Text(if (flagged) L10n.action_unflag(ctx) else L10n.action_flag(ctx)) },
                onClick = {
                    expanded = false
                    onToggleFlag()
                },
            )
            DropdownMenuItem(
                text = { Text(L10n.action_reply(ctx)) },
                onClick = {
                    expanded = false
                    onReply()
                },
            )
            DropdownMenuItem(
                text = { Text(L10n.action_reply_all(ctx)) },
                onClick = {
                    expanded = false
                    onReplyAll()
                },
            )
            DropdownMenuItem(
                text = { Text(L10n.action_forward(ctx)) },
                onClick = {
                    expanded = false
                    onForward()
                },
            )
            DropdownMenuItem(
                text = { Text(L10n.action_move_to_trash(ctx)) },
                onClick = {
                    expanded = false
                    onDelete()
                },
            )
            if (inJunkFolder) {
                DropdownMenuItem(
                    text = { Text(L10n.action_mark_as_not_spam(ctx)) },
                    onClick = {
                        expanded = false
                        onMarkAsNotSpam()
                    },
                )
            } else {
                DropdownMenuItem(
                    text = { Text(L10n.action_mark_as_spam(ctx)) },
                    onClick = {
                        expanded = false
                        onMarkAsSpam()
                    },
                )
            }
            DropdownMenuItem(
                text = {
                    Text(
                        text = L10n.action_delete_permanently(ctx),
                        color = MaterialTheme.colorScheme.error,
                    )
                },
                onClick = {
                    expanded = false
                    onPermanentlyDelete()
                },
            )
        }
    }
}

// A conversation row: a header (subject, message count, latest sender). Tapping it opens the
// conversation's latest message (received or sent) in the reading screen, where the older
// messages sit as a collapsed strip that opens each on tap (Gmail/Outlook-mobile style), rather
// than expanding inline in the list. Long-press the header to archive the conversation. Only real
// multi-message conversations reach here: the core projects a lone message as a flat row.
@OptIn(ExperimentalFoundationApi::class)
@androidx.compose.runtime.Composable
internal fun ThreadConversationRow(
    thread: ThreadRow,
    activeZoneId: String?,
    onOpenThread: () -> Unit,
    onArchiveThread: () -> Unit,
) {
    val ctx = LocalContext.current
    var menuOpen by remember { mutableStateOf(false) }
    Column(modifier = Modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .combinedClickable(
                    onClick = onOpenThread,
                    onLongClick = { menuOpen = true },
                )
                .padding(start = 16.dp, end = 16.dp, top = 8.dp, bottom = 8.dp),
            verticalAlignment = Alignment.Top,
        ) {
            // The latest sender is who the row names. The count badge already marks a conversation;
            // bold subject and sender carry unread state on this compact, single-pane list.
            AvatarView(thread.avatar, modifier = Modifier.testTag("thread-avatar"))
            Spacer(modifier = Modifier.width(12.dp))
            Column(modifier = Modifier.weight(1f)) {
                Row(verticalAlignment = Alignment.CenterVertically) {
                    Text(
                        text = thread.subject.ifEmpty { L10n.mail_no_subject(ctx) },
                        style = MaterialTheme.typography.bodyLarge,
                        fontWeight = if (thread.unreadCount > 0u) FontWeight.Bold else FontWeight.Normal,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                        modifier = Modifier.weight(1f),
                    )
                    // The message count marks this as a conversation at a glance (a real thread is
                    // always > 1 here, the core projects a lone message as a flat row).
                    if (thread.messageCount > 1u) {
                        Spacer(modifier = Modifier.width(6.dp))
                        Badge { Text("${thread.messageCount}") }
                    }
                    // A paperclip when any message in the conversation has an attachment (matching
                    // macOS/Windows thread rows).
                    if (thread.hasAttachment) {
                        Spacer(modifier = Modifier.width(6.dp))
                        Icon(
                            painter = painterResource(R.drawable.ic_attachment),
                            contentDescription = L10n.a11y_has_attachment(ctx),
                            tint = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.size(15.dp),
                        )
                    }
                    Spacer(modifier = Modifier.width(8.dp))
                    Text(
                        text = relativeDate(thread.latestDate, activeZoneId, LocalUse24Hour.current),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                    )
                }
                // Latest sender + a preview snippet of the representative message.
                Text(
                    text = senderAndPreview(
                        thread.latestFrom,
                        thread.preview,
                        unread = thread.unreadCount > 0u,
                    ),
                    style = MaterialTheme.typography.bodySmall,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
            }
        }
        // Long-press → "Archive conversation" (the core archives the received side only, leaving
        // any Sent copies in Sent).
        DropdownMenu(expanded = menuOpen, onDismissRequest = { menuOpen = false }) {
            DropdownMenuItem(
                text = { Text(L10n.thread_archive(ctx)) },
                onClick = {
                    menuOpen = false
                    onArchiveThread()
                },
            )
        }
    }
}

/**
 * How heavily a list row's **sender** is drawn.
 *
 * Its own function, and not an inline `if`, because it is the one part of the unread treatment that
 * is a decision rather than a layout: the subject and the sender have to move together, on this
 * platform and on the others, and a rule three clients implement separately is a rule that drifts.
 * Bold and Medium rather than Bold and Normal, the sender line is already secondary text, so
 * dropping it to Normal when read would make the read rows harder to scan than they are today.
 */
internal fun unreadSenderWeight(unread: Boolean): FontWeight =
    if (unread) FontWeight.Bold else FontWeight.Medium

// The second list-row line: the sender, slightly emphasised, followed by a lighter body preview
// snippet, so a truncated subject still carries context (cf. Thunderbird). The snippet's runs of
// whitespace are collapsed so it stays a clean single stretch of text.
@androidx.compose.runtime.Composable
private fun senderAndPreview(
    sender: String,
    preview: String,
    unread: Boolean = false,
): androidx.compose.ui.text.AnnotatedString {
    val senderColor = MaterialTheme.colorScheme.onSurfaceVariant
    val previewColor = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.7f)
    val snippet = preview.replace(Regex("\\s+"), " ").trim()
    return androidx.compose.ui.text.buildAnnotatedString {
        if (sender.isNotEmpty()) {
            // The weight is on the SENDER SPAN, not the whole line: sender and preview share one
            // Text here (they wrap as one paragraph), so bolding the string would bold the snippet
            // too, and a mailbox of unread rows in solid bold distinguishes nothing.
            withStyle(
                androidx.compose.ui.text.SpanStyle(
                    color = senderColor,
                    fontWeight = unreadSenderWeight(unread),
                )
            ) {
                append(sender)
            }
        }
        if (snippet.isNotEmpty()) {
            withStyle(androidx.compose.ui.text.SpanStyle(color = previewColor)) {
                if (sender.isNotEmpty()) append(" ,  ")
                append(snippet)
            }
        }
    }
}
