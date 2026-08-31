// The message reading view for the Android client: the header, the toolbar, the attachment list
// (ReadingScreenAttachments.kt) and the sanitised HTML/plain-text body (ReadingScreenBody.kt),
// which supplies the unavoidably-native WebView hardening. Remote images are blocked by default
// behind a "load remote images" confirmation.
//
// The security gates here are a CROSS-PLATFORM CONTRACT, see docs/rendering-security.md. Any
// gate added/raised on one platform must be applied to all of them (and recorded there).
package eu.allodia.mailcal

import android.os.Handler
import android.os.Looper
import android.widget.Toast
import java.io.File
import java.util.UUID
import kotlin.concurrent.thread
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.AttachmentRow
import uniffi.mailcal_bindings.Avatar
import uniffi.mailcal_bindings.CalendarWriteStatus
import uniffi.mailcal_bindings.ComposerFileAttachment
import uniffi.mailcal_bindings.InvitationResponse
import uniffi.mailcal_bindings.QuoteSettings
import uniffi.mailcal_bindings.ReadingSnapshot
import uniffi.mailcal_bindings.Recipients
import uniffi.mailcal_bindings.RecipientMatch
import uniffi.mailcal_bindings.RecipientSuggestion
import uniffi.mailcal_bindings.ThreadMessage

// The header context for an opened message (the row the user tapped). The body itself is
// pulled from the core's `reading` snapshot, matched by `key`. `account` is the owning
// account's id (key-routed actions, open/reply/forward, must dispatch against it so a
// provider-key collision across accounts can't route to the wrong one).
internal data class OpenedMessage(
    val account: String,
    val key: String,
    val subject: String,
    val from: String,
    /** The row's avatar, shown until the matching reading snapshot arrives. */
    val avatar: Avatar,
    val date: String,
)

/** The stable identity header above the late-loading body and recipient fields. */
@Composable
internal fun ReadingIdentityHeader(
    message: OpenedMessage,
    reading: ReadingSnapshot?,
    onBack: () -> Unit,
) {
    val ctx = LocalContext.current
    val body = reading?.takeIf { it.key == message.key }
    val senderLine = body?.from?.takeIf { it.isNotEmpty() } ?: message.from
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(start = 4.dp, end = 16.dp, top = 6.dp, bottom = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        IconButton(onClick = onBack) {
            Icon(
                painter = painterResource(R.drawable.ic_arrow_back),
                contentDescription = L10n.a11y_back(ctx),
            )
        }
        AvatarView(
            avatar = body?.avatar ?: message.avatar,
            modifier = Modifier.testTag("reading-avatar"),
        )
        Spacer(modifier = Modifier.width(12.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = message.subject.ifEmpty { L10n.mail_no_subject(ctx) },
                style = MaterialTheme.typography.titleMedium,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Text(
                text = senderLine,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Spacer(modifier = Modifier.width(8.dp))
        Text(
            text = message.date,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

// The reading screen: a header plus the fetched body, or a spinner until the body for this
// message arrives (the fetch is async, a network round-trip on the first open). The system
// back gesture/button returns to the list.
@Composable
internal fun ReadingScreen(
    message: OpenedMessage,
    reading: ReadingSnapshot?,
    // The whole conversation (newest-first) when the opened message belongs to a multi-message
    // thread, else null, drives the collapsed strip of the other messages above the open one.
    conversation: List<ThreadMessage>?,
    // The active display zone, to localise the strip cards' dates.
    activeZoneId: String?,
    quoteSettings: QuoteSettings,
    // The calendar write indicator, which an invitation answer drives too, it changes the
    // calendar, so it reports on the same one every other calendar write does.
    calendarWriteStatus: CalendarWriteStatus,
    // Answer the invitation this message carries. Named by the message, never by the event: the
    // answer goes out as the address the invitation matched, and only the core knows the address
    // set (docs/invitations.md §4).
    onRespondToInvitation: (
        account: String,
        key: String,
        response: InvitationResponse,
        comment: String?,
        notifyOrganizer: Boolean,
        replySubject: String,
    ) -> Unit,
    onBack: () -> Unit,
    onRetry: () -> Unit,
    // Opens another message of the same conversation from the strip (accordion, one body shown
    // at a time), keeping the conversation context so the strip persists.
    onOpenThreadMessage: (OpenedMessage) -> Unit,
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
    // Every configured account, for the composer's From dropdown.
    accounts: List<AccountRow>,
    onSaveAttachment: (
        account: String,
        key: String,
        attachmentId: UInt,
        destinationPath: String,
    ) -> Boolean,
    onArchive: (account: String, key: String) -> Unit,
    onDelete: (account: String, key: String) -> Unit,
    // Screenshot only: opens the composer in this mode as soon as the message is shown, so the
    // showcase's reply screenshot needs no tap. Null in every normal launch.
    initialComposing: RichComposeMode? = null,
    // Screenshot only: sample text to pre-fill the reply composer's body with (plain text).
    composerInitialText: String? = null,
) {
    BackHandler(onBack = onBack)
    val ctx = LocalContext.current
    // Whether the user chose to load this message's remote images (reset per message).
    var loadRemoteImages by remember(message.key) { mutableStateOf(false) }
    // The open rich composer (null = closed, else the active mode), like the list rows.
    var composing by remember(message.key) { mutableStateOf<RichComposeMode?>(null) }
    // Screenshot only: open it once the body has loaded, the quoted original (and the sample reply
    // text seeded above it) only exist then, exactly as for a user-driven reply.
    if (initialComposing != null) {
        LaunchedEffect(message.key, reading?.key) {
            if (composing == null && reading?.key == message.key) composing = initialComposing
        }
    }
    var pendingSave by remember(message.key) { mutableStateOf<AttachmentRow?>(null) }
    val saveAttachment = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("*/*"),
    ) { uri ->
        val attachment = pendingSave
        pendingSave = null
        if (uri == null || attachment == null) {
            return@rememberLauncherForActivityResult
        }
        // The core decodes + writes the whole part and then we copy it to the picked location:
        // both off the main thread (the connect path threads blocking core calls for the same
        // reason), reporting the result back on the main thread.
        val account = message.account
        val key = message.key
        thread(name = "mailcal-save-attachment") {
            val temp = File(ctx.cacheDir, "saved-attachments/${UUID.randomUUID()}.part")
            temp.parentFile?.mkdirs()
            val saved = onSaveAttachment(account, key, attachment.id, temp.absolutePath)
            val copied = if (saved) {
                try {
                    ctx.contentResolver.openOutputStream(uri)?.use { output ->
                        temp.inputStream().use { input -> input.copyTo(output) }
                    } != null
                } catch (_: Exception) {
                    false
                } finally {
                    temp.delete()
                }
            } else {
                temp.delete()
                false
            }
            Handler(Looper.getMainLooper()).post {
                Toast.makeText(
                    ctx,
                    if (copied) L10n.attachment_saved(ctx) else L10n.attachment_save_failed(ctx),
                    Toast.LENGTH_SHORT,
                ).show()
            }
        }
    }
    // This screen renders outside the Scaffold (which would pad its content), so it must clear
    // BOTH system bars itself, otherwise the header draws under the status bar and the end of the
    // message body under the navigation bar (edge-to-edge default). The composer overlay below is
    // a sibling of this Column, not a child, so it keeps its own Scaffold + imePadding.
    Column(modifier = Modifier.fillMaxSize().systemBarsPadding()) {
        ReadingIdentityHeader(message = message, reading = reading, onBack = onBack)
        // Message actions as a compact icon toolbar: reply/reply-all/forward on the left,
        // archive/delete on the right. Icons (not text) both save space and, crucially, keep
        // this bar's height/position fixed, it sits directly under the always-present header,
        // ABOVE the late-loading recipient header and body, so nothing reflows when the message
        // snapshot arrives (the old text bar jumped as the recipient lines popped in above it).
        // Every glyph here is a vendored Material Symbols drawable (res/drawable/ic_*.xml).
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 4.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = { composing = RichComposeMode.Reply }) {
                Icon(painterResource(R.drawable.ic_reply), contentDescription = L10n.action_reply(ctx))
            }
            IconButton(onClick = { composing = RichComposeMode.ReplyAll }) {
                Icon(painterResource(R.drawable.ic_reply_all), contentDescription = L10n.action_reply_all(ctx))
            }
            IconButton(onClick = { composing = RichComposeMode.Forward }) {
                Icon(painterResource(R.drawable.ic_forward), contentDescription = L10n.action_forward(ctx))
            }
            Spacer(modifier = Modifier.weight(1f))
            // Archive/delete move the message out of the folder; pop back to the list (the row
            // leaves it immediately via the core's optimistic removal).
            IconButton(onClick = {
                onArchive(message.account, message.key)
                onBack()
            }) {
                Icon(painterResource(R.drawable.ic_archive), contentDescription = L10n.action_archive(ctx))
            }
            IconButton(onClick = {
                onDelete(message.account, message.key)
                onBack()
            }) {
                Icon(
                    painter = painterResource(R.drawable.ic_delete),
                    contentDescription = L10n.action_delete(ctx),
                    tint = MaterialTheme.colorScheme.error,
                )
            }
        }
        HorizontalDivider()
        // When the open message is part of a multi-message conversation, list the other messages
        // as a collapsed strip above it; tapping one opens that message (accordion, one body at a
        // time). A standalone or single-message conversation shows no strip.
        conversation?.takeIf { it.size > 1 }?.let { thread ->
            ConversationStrip(
                conversation = thread,
                focusedKey = message.key,
                subject = message.subject,
                activeZoneId = activeZoneId,
                onOpen = onOpenThreadMessage,
            )
            HorizontalDivider()
        }
        // The recipient headers (To / Cc / Bcc), once this message's snapshot has arrived. Each
        // row shows only when non-empty; Bcc appears only on the user's own Sent/Drafts copies
        // (whose stored message carries a Bcc header), so they can see whom they Bcc'd.
        reading?.takeIf { it.key == message.key }?.let { snapshot ->
            RecipientHeader(snapshot.to, snapshot.cc, snapshot.bcc)
        }
        val body = reading?.takeIf { it.key == message.key }
        // The meeting-invitation card, above everything the sender wrote. Present only when the
        // core's two-condition RSVP gate says this message is one (docs/invitations.md), a
        // published `.ics` keeps its attachment chip and gets no card.
        InvitationBanner(
            snapshot = body,
            activeZoneId = activeZoneId,
            writeStatus = calendarWriteStatus,
            onRespond = { response, comment, notify, replySubject ->
                onRespondToInvitation(
                    message.account,
                    message.key,
                    response,
                    comment,
                    notify,
                    replySubject,
                )
            },
        )
        if (body != null && body.attachments.isNotEmpty()) {
            AttachmentList(
                attachments = body.attachments,
                onSave = { attachment ->
                    pendingSave = attachment
                    saveAttachment.launch(attachment.fileName.ifEmpty { "attachment" })
                },
                // Open hands the decoded file to the OS default viewer (which runs the OS's own
                // file scan); we never render or execute attachment content in-app.
                onOpen = { attachment ->
                    openAttachment(
                        ctx = ctx,
                        account = message.account,
                        key = message.key,
                        attachment = attachment,
                        onSaveAttachment = onSaveAttachment,
                    )
                },
            )
        }
        // The remote-images opt-in banner sits above the body as a fixed-height notice, shown
        // only for an HTML body that still has its remote images blocked.
        if (body != null && !body.html.isNullOrEmpty() && body.hasRemoteImages && !loadRemoteImages) {
            RemoteImagesBanner(onLoad = { loadRemoteImages = true })
        }
        // The body takes the remaining height (weight), which bounds the WebView to this region:
        // an AndroidView reports its own large content height, so as an unweighted fillMaxSize
        // child it could overlap the fixed header/toolbar on the first inflation. Only show the
        // body once the snapshot for *this* message has arrived (ignore a stale one for a
        // previously-opened message).
        Box(modifier = Modifier.weight(1f).fillMaxWidth()) {
            when {
                // Nothing yet, and too soon to say so: the core announces a wait only once
                // one has run long enough to notice, so a fast open draws no spinner at all
                // rather than flashing one. The header above is already filled from the row.
                body == null -> Unit
                // Carries no body, this has to precede the branches that read one.
                body.pending -> CenteredMessage { CircularProgressIndicator() }
                body.loadError -> CenteredMessage {
                    Column(
                        horizontalAlignment = Alignment.CenterHorizontally,
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Text(
                            text = L10n.reading_load_error(ctx),
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                        TextButton(onClick = onRetry) { Text(L10n.action_retry(ctx)) }
                    }
                }
                !body.html.isNullOrEmpty() ->
                    HtmlBody(fragment = body.html!!, loadRemoteImages = loadRemoteImages)
                !body.plain.isNullOrEmpty() -> Text(
                    text = body.plain!!,
                    modifier = Modifier
                        .fillMaxSize()
                        .verticalScroll(rememberScrollState())
                        .padding(16.dp),
                    style = MaterialTheme.typography.bodyMedium,
                )
                else -> CenteredMessage {
                    Text(
                        text = L10n.reading_no_content(ctx),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
    // The rich composer overlay for reply/reply-all/forward (its own window), mirroring the
    // list rows: the same hardened editor, its rendered document riding submitRichReply/
    // submitRichForward. Reply/reply-all open with To/Cc pre-filled from the core.
    composing?.let { mode ->
        val prefill = remember(message.key, mode) {
            if (mode == RichComposeMode.Reply || mode == RichComposeMode.ReplyAll) {
                replyRecipients(message.account, message.key, mode == RichComposeMode.ReplyAll)
            } else {
                null
            }
        }
        // Seed the quoted original from this message's already-sanitised reading body (null when
        // the body hasn't arrived yet, the composer then opens empty, as on a list-row reply).
        val quote = ComposerQuote.seedJson(
            ctx = ctx,
            style = quoteSettings.style,
            message = message,
            reading = reading,
            isForward = mode == RichComposeMode.Forward,
            initialText = if (mode == RichComposeMode.Forward) null else composerInitialText,
        )
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
            quote = quote,
            quoteStyle = quoteSettings.style,
            quoteStylePerMessage = quoteSettings.perMessage,
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
