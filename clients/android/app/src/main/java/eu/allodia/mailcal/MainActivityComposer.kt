// Opening a composer, for every way into one: a new message, a reply, a reply to all, a forward, a
// Drafts-folder row, a mail link, a share and an Outbox edit. Each asks the core once, off the main
// thread, and draws what it answered through one factory, so no route can open a composer holding
// less than another (docs/drafts.md).
package eu.allodia.mailcal

import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import kotlin.concurrent.thread
import uniffi.mailcal_bindings.ComposeOpening
import uniffi.mailcal_bindings.ComposeOrigin
import uniffi.mailcal_bindings.ComposePrefill
import uniffi.mailcal_bindings.ComposerFileAttachment
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp

private const val TAG = "Mailcal"

/** A composer on screen: what the core opened it with, and the files a share added to it. */
internal data class OpenComposer(
    val opening: ComposeOpening,
    val extraAttachments: List<ComposerFileAttachment> = emptyList(),
)

/** The composer the activity is showing, or opening. */
internal class ComposerUiState {
    var open by mutableStateOf<OpenComposer?>(null)
    // Whether the core is still answering an open. A composer appears when the answer does,
    // holding everything: one shown before its files arrived could be sent without them.
    var pending by mutableStateOf(false)
    // Whether to say that a composer could not be opened (`compose_open_failed`).
    var openFailed by mutableStateOf(false)

    /** Whether a composer opened here is up or on its way. */
    val busy: Boolean get() = open != null || pending
}

/**
 * Whether any composer is up or on its way, an Outbox edit's included: a mail link or a share waits
 * rather than open a second composer over it.
 */
internal val MainActivity.composerBusy: Boolean get() = composer.busy || withdrawnMessage != null

/**
 * Opens a composer on `origin`, with `prefill` laid over what it opens with and a share's files
 * added to the ones the core staged.
 *
 * The core is asked off the main thread: a reply may fetch the message it quotes, a forward stages
 * its files and a draft is read from the server. Nothing opens on a failure, which `onFailed`
 * says: a draft composer opened empty would save over the draft.
 */
internal fun MainActivity.openComposer(
    instance: MailcalApp,
    origin: ComposeOrigin,
    prefill: ComposePrefill? = null,
    extraAttachments: List<ComposerFileAttachment> = emptyList(),
    onFailed: () -> Unit = { composer.openFailed = true },
) {
    if (composerBusy) {
        return
    }
    composer.pending = true
    val composition = newComposition()
    val directory = composerStagingDirectory(composition)
    val laidOver = prefill ?: showcaseReplyPrefill(origin)
    thread(name = "mailcal-open-composer") {
        val opening = try {
            directory.mkdirs()
            instance.openComposer(composition, origin, directory.absolutePath, laidOver)
        } catch (e: Exception) {
            // Any exception, not only the core's own: one that escaped would leave `pending`
            // set, and with it every later composer, link and share waiting forever.
            Log.w(TAG, "composer open failed: ${e.javaClass.simpleName}")
            null
        }
        Handler(Looper.getMainLooper()).post {
            composer.pending = false
            if (opening == null) {
                onFailed()
            } else {
                composer.open = OpenComposer(opening, extraAttachments)
            }
        }
    }
}

/**
 * The sample answer showcase mode puts above a reply's quote, so the store screenshot shows a
 * written reply rather than an empty composer. Null for every other message and every normal run.
 */
private fun MainActivity.showcaseReplyPrefill(origin: ComposeOrigin): ComposePrefill? {
    val reply = origin as? ComposeOrigin.Reply ?: return null
    val text = ShowcaseMode.replyText(this, reply.account, reply.key) ?: return null
    return ComposePrefill("", "", "", "", text, false, null)
}

/**
 * Opens the composer a mail link or a share asked for, once no other composer is up or on its way:
 * the link or the files stay pending until then, so neither can replace a draft the user is in the
 * middle of. Drawn by the mailbox, so neither opens over the welcome or setup screens.
 *
 * Both open as a new message on the selected account, with what they named laid over it; the core
 * has already dropped every header a link may not set, and named every shared file.
 */
@Composable
internal fun MainActivity.LinkAndShareComposers(instance: MailcalApp) {
    val busy = composerBusy
    LaunchedEffect(pendingMailto, busy) {
        val link = pendingMailto
        if (link != null && !busy) {
            pendingMailto = null
            openComposer(
                instance,
                ComposeOrigin.New(selectedAccount),
                ComposePrefill(link.to, link.cc, link.bcc, link.subject, link.body, false, null),
            )
        }
    }
    LaunchedEffect(pendingShare, busy) {
        val share = pendingShare
        if (share != null && !busy) {
            pendingShare = null
            openComposer(
                instance,
                ComposeOrigin.New(selectedAccount),
                ComposePrefill(share.to, share.cc, share.bcc, share.subject, share.body, false, null),
                share.attachments,
            )
        }
    }
}

/**
 * The composer on screen, over whichever screen is behind it, and the message a queued send the
 * user chose to edit comes back as. The withdrawn message exists nowhere else by the time it
 * arrives, so the request is dismissed only once its composer has closed (docs/sending.md).
 */
@Composable
internal fun MainActivity.ComposerPane(instance: MailcalApp) {
    val ctx = LocalContext.current
    composer.open?.let { open ->
        key(open.opening.composition) {
            ComposerFor(instance, open.opening, open.extraAttachments) { composer.open = null }
        }
    }
    withdrawnMessage?.let { opening ->
        // The recipients, the subject and the body are the user's own mail; none is logged.
        LaunchedEffect(opening.composition) {
            Log.i(TAG, "outbox: a withdrawn message is going back into the composer")
        }
        key(opening.composition) {
            ComposerFor(instance, opening) {
                withdrawnMessage = null
                instance.dispatch(Intent.DismissComposeRequest)
            }
        }
    }
    if (composer.openFailed) {
        ComposerOpenFailedDialog(L10n.compose_open_failed(ctx)) { composer.openFailed = false }
    }
    // The message opens for reading behind this, so nothing is lost; what it cannot do is be edited.
    if (drafts.draftOpenFailed) {
        ComposerOpenFailedDialog(L10n.compose_draft_open_failed(ctx)) { drafts.draftOpenFailed = false }
    }
}

/** The one factory every composer is drawn through, from what the core opened it with. */
@Composable
private fun MainActivity.ComposerFor(
    instance: MailcalApp,
    opening: ComposeOpening,
    extraAttachments: List<ComposerFileAttachment> = emptyList(),
    onDismiss: () -> Unit,
) {
    val ctx = LocalContext.current
    val use24Hour = LocalUse24Hour.current
    val zone = timeZone?.active
    val request = remember(opening) {
        composeContext(
            ctx = ctx,
            opening = opening,
            initialFrom = sendAccountFor(opening.account, defaultSendAccount, accounts),
            quoteSettings = quoteSettings,
            dateText = { localDateTime(it, zone, use24Hour) },
            extraAttachments = extraAttachments,
        )
    }
    RichComposeMessageDialog(
        request = request,
        accounts = accounts,
        // Ranked addresses for a partially-typed recipient, drawn from synced contacts and from
        // people the user has written to. A local in-memory read, safe per keystroke.
        suggestionsFor = { prefix ->
            try {
                instance.recipientSuggestions(prefix)
            } catch (e: Exception) {
                Log.w(TAG, "recipient suggestions failed: ${e.javaClass.simpleName}")
                emptyList()
            }
        },
        signatures = composerSignatures(instance, signatures?.signatures.orEmpty()),
        drafts = composerDrafts(instance),
        onSubmitRich = { submission -> submitComposition(instance, submission) },
        onDismiss = onDismiss,
    )
}
