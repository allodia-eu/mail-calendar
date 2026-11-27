// The message reading view for the Android client: the top app bar (ReadingTopBar.kt), the header
// (ReadingHeader.kt), the attachment list (ReadingScreenAttachments.kt) and the sanitised
// HTML/plain-text body (ReadingScreenBody.kt), which supplies the unavoidably-native WebView
// hardening. Remote images are blocked by default behind a "load remote images" confirmation.
//
// The security gates here are a CROSS-PLATFORM CONTRACT, see docs/rendering-security.md. Any
// gate added/raised on one platform must be applied to all of them (and recorded there).
package eu.allodia.mailcal

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.LocalContentColor
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AttachmentRow
import uniffi.mailcal_bindings.Avatar
import uniffi.mailcal_bindings.CalendarWriteStatus
import uniffi.mailcal_bindings.ComposeOrigin
import uniffi.mailcal_bindings.InvitationResponse
import uniffi.mailcal_bindings.MessageScheme
import uniffi.mailcal_bindings.ReadingSnapshot
import uniffi.mailcal_bindings.ThreadMessage
import uniffi.mailcal_bindings.attachmentsOfferSaveAll
import uniffi.mailcal_bindings.messageCanvas

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
    /**
     * The received instant as the core gave it. The screen writes it out as it draws, in the zone
     * and on the clock in force then, so a change to either reaches a message already open
     * (docs/timestamps.md).
     */
    val date: String,
)

/**
 * The subject the reading screen shows: the row's, or the snapshot's once it has landed. A
 * notification tap can open a message the list does not hold yet, which leaves the row's empty,
 * and only the snapshot can fill it in. Empty when neither has one.
 */
internal fun displayedSubject(message: OpenedMessage, reading: ReadingSnapshot?): String =
    message.subject.ifEmpty { reading?.takeIf { it.key == message.key }?.subject.orEmpty() }

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
    // Opens another message of the conversation, or resumes it when it is a draft.
    onOpenThreadMessage: (opened: OpenedMessage, draft: Boolean) -> Unit,
    // Opens the composer on a reply, a reply to all or a forward of this message. The composer is
    // the activity's (MainActivityComposer.kt), and quotes what the core fetches, so it does not
    // wait for this screen's body.
    onCompose: (ComposeOrigin) -> Unit,
    onSaveAttachment: (
        account: String,
        key: String,
        attachmentId: UInt,
        destinationPath: String,
    ) -> Boolean,
    // Every attachment into a folder: the paths written, or null when the save failed.
    onSaveAllAttachments: (account: String, key: String, directoryPath: String) -> List<String>?,
    onExportMessage: (account: String, key: String, destinationPath: String) -> Boolean,
    onArchive: (account: String, key: String) -> Unit,
    // Leaving the message is the caller's, once the delete is sent: in Trash it asks first.
    onDelete: (account: String, key: String) -> Unit,
    // Whether the reader left dark message backgrounds on, and how to change it: the overflow's
    // sun/moon sets the same preference as Settings → Reading (docs/reading-dark-mode.md).
    darkMessages: Boolean,
    onSetDarkMessages: (Boolean) -> Unit,
    // Screenshot only: opens a reply as soon as the message's body is shown, so the showcase's
    // reply screenshot needs no tap. False in every normal launch.
    opensReplyOnShow: Boolean = false,
) {
    BackHandler(onBack = onBack)
    val ctx = LocalContext.current
    val appDark = LocalAppDark.current
    val scheme = messageScheme(appDark, darkMessages)
    // Whether the user chose to load this message's remote images (reset per message, kept
    // across a visit to another tab).
    var loadRemoteImages by rememberSaveable(message.key) { mutableStateOf(false) }
    // Screenshot only: opened once, after the body, so the reading view behind the composer is
    // the finished one.
    if (opensReplyOnShow) {
        var replied by remember(message.key) { mutableStateOf(false) }
        LaunchedEffect(message.key, reading?.key) {
            if (!replied && reading?.key == message.key) {
                replied = true
                onCompose(ComposeOrigin.Reply(message.account, message.key, false))
            }
        }
    }
    var pendingSave by remember(message.key) { mutableStateOf<AttachmentRow?>(null) }
    val saveAttachment = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("*/*"),
    ) { uri ->
        val attachment = pendingSave
        pendingSave = null
        if (uri != null && attachment != null) {
            saveAttachmentTo(ctx, uri, message.account, message.key, attachment, onSaveAttachment)
        }
    }
    var savingAll by remember(message.key) { mutableStateOf(false) }
    val saveAllAttachments = rememberLauncherForActivityResult(
        ActivityResultContracts.OpenDocumentTree(),
    ) { tree ->
        if (tree != null) {
            savingAll = true
            saveAllAttachmentsTo(ctx, tree, message.account, message.key, onSaveAllAttachments) {
                savingAll = false
            }
        }
    }
    val headerDate = localDateTime(message.date, activeZoneId, LocalUse24Hour.current)
    // Drawn inside the tab scaffold, which clears the status bar above and the navigation bar
    // below, so the Mail, Calendar and Contacts bar stays while a message is open. Only a snapshot
    // for *this* message is its body (ignore a stale one for a previously-opened message).
    val body = reading?.takeIf { it.key == message.key }
    // The page a message is drawn on, for every state of an open (the gap before the body
    // lands, the spinner, a plain-text body, a load error) so a body arriving changes what is
    // written on the page and never the page itself. Left transparent, the gap punched a hole
    // in it: in dark theme the body area went white, black, white on every message opened,
    // which reads as a flicker rather than as a message opening (docs/sync-progress.md). The
    // colour is the core's, the same one it gives the HTML document in this scheme, and the
    // ink goes with the page rather than the app: a dark app reading with dark messages off
    // draws a white page, where the dark theme's own content colour would be white on white.
    // The two accented controls drawn on it, the spinner and Retry, take the page's scheme's
    // primary for the same reason: the dark scheme's is the pale lavender Material 3 pairs
    // with a near-black surface, and on a white page it is barely there.
    //
    // Resolved per scheme: the record crosses the FFI.
    val canvas = remember(scheme) { messageCanvas(scheme) }
    val page = remember(canvas) { parseHexColor(canvas.background) }
    val ink = remember(canvas) { parseHexColor(canvas.foreground) }
    val accent = remember(scheme) {
        if (scheme == MessageScheme.DARK) darkColorScheme().primary else lightColorScheme().primary
    }
    // How far the header has scrolled away, kept for this message across a visit to another tab.
    val headerOffset = rememberSaveable(message.key) { mutableFloatStateOf(0f) }
    val header = remember(headerOffset) { CollapsingHeader(headerOffset) }
    Column(modifier = Modifier.fillMaxSize()) {
        ReadingTopBar(
            onBack = onBack,
            // Archive/delete move the message out of the folder; pop back to the list (the row
            // leaves it immediately via the core's optimistic removal).
            onArchive = {
                onArchive(message.account, message.key)
                onBack()
            },
            // The caller leaves the message once the delete is sent: in Trash it asks first.
            onDelete = { onDelete(message.account, message.key) },
        ) {
            ReadingOverflowMenu(
                // The subject this screen DISPLAYS, so an untitled message exports under the
                // same words it is shown under.
                subject = displayedSubject(message, reading).ifEmpty { L10n.mail_no_subject(ctx) },
                account = message.account,
                key = message.key,
                onExportMessage = onExportMessage,
                onPrint = reading?.takeIf { it.key == message.key && isPrintable(it) }?.let { body ->
                    { printOpenMessage(ctx, message, body, headerDate, loadRemoteImages) }
                },
                // The first item, and only in a dark app (docs/reading-dark-mode.md).
                scheme = scheme.takeIf { appDark },
                onSetDarkMessages = onSetDarkMessages,
            )
        }
        ReadingPage(
            state = header,
            modifier = Modifier.weight(1f).fillMaxWidth(),
            header = {
                Column(modifier = Modifier.fillMaxWidth().background(MaterialTheme.colorScheme.surface)) {
                    ReadingSubject(message, reading)
                    // When the open message is part of a multi-message conversation, list the
                    // other messages under the subject they share, as part of the page; tapping
                    // one opens that message (accordion, one body at a time). A standalone or
                    // single-message conversation shows none.
                    conversation?.takeIf { it.size > 1 }?.let { thread ->
                        HorizontalDivider()
                        ConversationStrip(
                            conversation = thread,
                            focusedKey = message.key,
                            subject = message.subject,
                            activeZoneId = activeZoneId,
                            onOpen = onOpenThreadMessage,
                        )
                        Spacer(modifier = Modifier.height(12.dp))
                    }
                    ReadingSender(message, reading, activeZoneId)
                    HorizontalDivider()
                    // The sender's importance, under the header it qualifies (docs/importance.md).
                    ImportanceBanner(body?.importanceBanner)
                    // The meeting-invitation card, above everything the sender wrote. Present
                    // only when the core's two-condition RSVP gate says this message is one
                    // (docs/invitations.md), a published `.ics` keeps its attachment chip and gets
                    // no card.
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
                            offerSaveAll = attachmentsOfferSaveAll(body.attachments.size.toUInt()),
                            savingAll = savingAll,
                            onSaveAll = { saveAllAttachments.launch(null) },
                            onSave = { attachment ->
                                pendingSave = attachment
                                saveAttachment.launch(attachment.fileName.ifEmpty { "attachment" })
                            },
                            // Open hands the decoded file to the OS default viewer (which runs the
                            // OS's own file scan); we never render or execute attachment content
                            // in-app.
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
                    // The remote-images opt-in banner sits above the body, shown only for an
                    // HTML body that still has its remote images blocked.
                    if (body != null && !body.html.isNullOrEmpty() && body.hasRemoteImages && !loadRemoteImages) {
                        RemoteImagesBanner(onLoad = { loadRemoteImages = true })
                    }
                }
            },
            body = {
                // The page a message is drawn on fills the body for every state of an open.
                Box(modifier = Modifier.fillMaxSize().background(page)) {
                    CompositionLocalProvider(LocalContentColor provides ink) {
                        when {
                            // Nothing yet, and too soon to say so: the core announces a wait only
                            // once one has run long enough to notice, so a fast open draws no
                            // spinner at all rather than flashing one. The header above is already
                            // filled from the row.
                            body == null -> Unit
                            // Carries no body, this has to precede the branches that read one.
                            body.pending -> CenteredMessage { CircularProgressIndicator(color = accent) }
                            body.loadError -> CenteredMessage {
                                Column(
                                    horizontalAlignment = Alignment.CenterHorizontally,
                                    verticalArrangement = Arrangement.spacedBy(8.dp),
                                ) {
                                    Text(text = L10n.reading_load_error(ctx), color = LocalContentColor.current)
                                    TextButton(
                                        onClick = onRetry,
                                        colors = ButtonDefaults.textButtonColors(contentColor = accent),
                                    ) { Text(L10n.action_retry(ctx)) }
                                }
                            }
                            !body.html.isNullOrEmpty() ->
                                HtmlBody(fragment = body.html!!, loadRemoteImages = loadRemoteImages, scheme = scheme)
                            !body.plain.isNullOrEmpty() -> LinkifiedText(
                                text = body.plain!!,
                                modifier = Modifier
                                    .fillMaxSize()
                                    .verticalScroll(rememberScrollState())
                                    .padding(16.dp),
                                style = MaterialTheme.typography.bodyMedium,
                            )
                            else -> CenteredMessage {
                                Text(text = L10n.reading_no_content(ctx), color = LocalContentColor.current)
                            }
                        }
                    }
                }
            },
        )
        ReadingAnswers(
            defaultReplyAll = body?.defaultReplyAll ?: false,
            onReply = { onCompose(ComposeOrigin.Reply(message.account, message.key, false)) },
            onReplyAll = { onCompose(ComposeOrigin.Reply(message.account, message.key, true)) },
            onForward = { onCompose(ComposeOrigin.Forward(message.account, message.key)) },
        )
    }
}
