// Keeping the message being composed on the server: the core verbs, as the composer calls them
// (`docs/drafts.md`).
//
// Every one of them names a composition, which is the host's handle on one open composer. The core
// mints none: a composer takes an id when it opens and keeps it until it closes, and that is what
// lets two composers save without superseding each other's draft.
package eu.allodia.mailcal

import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import java.io.File
import java.util.UUID
import kotlin.concurrent.thread
import uniffi.mailcal_bindings.DraftResume
import uniffi.mailcal_bindings.DraftStatus
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.draftAutosaveIdleSeconds
import uniffi.mailcal_bindings.ThreadMessage
import uniffi.mailcal_bindings.ThreadRow

private const val TAG = "Mailcal"

/**
 * The verbs a composer keeps its draft with, bound to this activity's core instance.
 *
 * Built once per recomposition so every composer on screen saves, discards and closes through the
 * same calls, and carries the draft-status counter so each of them can pull its own state.
 */
internal fun MainActivity.composerDrafts(instance: MailcalApp) = ComposerDrafts(
    save = { composition, content -> saveDraft(instance, composition, content, close = false) },
    saveAndClose = { composition, content ->
        saveDraft(instance, composition, content, close = true)
    },
    discard = { composition ->
        try {
            instance.discardDraft(composition)
        } catch (e: MailcalException) {
            Log.w(TAG, "draft discard failed: ${e.javaClass.simpleName}")
        }
    },
    close = { composition ->
        try {
            instance.closeComposition(composition)
        } catch (e: MailcalException) {
            Log.w(TAG, "composition close failed: ${e.javaClass.simpleName}")
        }
    },
    status = { composition ->
        try {
            instance.draftStatus(composition)
        } catch (e: MailcalException) {
            DraftStatus.IDLE
        }
    },
    isStored = { composition ->
        try {
            instance.draftIsStored(composition)
        } catch (e: MailcalException) {
            // Read as stored, so Discard asks rather than removing a copy it could not account for.
            true
        }
    },
    version = drafts.draftStatusVersion,
    idleSeconds = draftAutosaveIdleSeconds(),
)

/**
 * Stores what the composer holds in the Drafts folder, replacing what this composition's previous
 * save left there.
 *
 * Always the file-carrying call, even from a composer holding none. A save replaces the stored
 * copy, so one that left the files out would take them off a draft the user is still writing, and
 * a composer can pick a file at any moment.
 *
 * Fire and forget: it returns as soon as the document validates, and the outcome arrives as a
 * `Surface::DraftStatus` signal. A save with no network is queued, not lost. With `close`, the core
 * forgets the composition once the save has settled, which is what leaving the composer means.
 */
private fun saveDraft(
    instance: MailcalApp,
    composition: String,
    content: ComposerSubmission,
    close: Boolean,
) {
    try {
        val save = if (close) instance::saveDraftAndClose else instance::saveDraftWithFiles
        save(
            composition,
            content.recipients,
            content.subject,
            content.documentJson,
            content.files,
            content.from,
        )
    } catch (e: MailcalException) {
        Log.w(TAG, "draft save failed: ${e.javaClass.simpleName}")
    }
}

/**
 * Opens the stored draft `key` (on `account`) into a composition of its own, so the composer about
 * to show it saves over that copy rather than beside it.
 *
 * Null means the draft could not be opened, which the caller says rather than showing an empty
 * composer: nothing is adopted on a failure, and a composer opened without the draft's content
 * would replace it with what is on screen the next time it saved.
 *
 * Blocks on the core's runtime, so it is called off the main thread; unlike a forward's staging it
 * cannot count on a warm cache, because a draft is opened from a list row rather than from the
 * reading view, so the first open of one fetches the message.
 */
internal fun resumeDraft(
    instance: MailcalApp,
    composition: String,
    account: String,
    key: String,
    stagingDirectory: File,
): DraftResume? = try {
    stagingDirectory.mkdirs()
    instance.resumeDraft(composition, account, key, stagingDirectory.absolutePath)
} catch (e: MailcalException) {
    Log.w(TAG, "draft resume failed: ${e.javaClass.simpleName}")
    null
}

/** A fresh composition id, minted as a composer opens. */
internal fun newComposition(): String = UUID.randomUUID().toString()

/** What the activity holds about Drafts between snapshots. */
internal class DraftUiState {
    // Whether the list is showing the account's Drafts folder, so a row opens into a composer that
    // saves over it rather than into the reading view (docs/drafts.md). The core answers it from
    // the folder's role, never from its name.
    var showingDrafts by mutableStateOf(false)
    // Bumped on every Surface.DRAFT_STATUS signal, so each open composer re-pulls its own
    // composition's state. The signal says that *some* composition's save moved, not which, and
    // there is nothing else to publish.
    var draftStatusVersion by mutableStateOf(0)
    // A draft the core has opened back up, waiting for its composer to be drawn. Null the rest of
    // the time; set only by a tap on a Drafts-folder row (docs/drafts.md).
    var resumedDraft by mutableStateOf<ResumedDraft?>(null)
    // Whether to say that a draft could not be opened back into a composer. Raised instead of
    // opening an empty one, whose next save would replace the draft.
    var draftOpenFailed by mutableStateOf(false)
}

/** A draft the core has opened back up, and the composition it was adopted into. */
internal data class ResumedDraft(val composition: String, val draft: DraftResume)

/**
 * Opens a Drafts-folder row back into its composer, and every other row for reading.
 *
 * Told by the **folder**, not by the row: the core answers per message but the list does not carry
 * it, so a draft met in a search result or inside a thread opens read-only (`docs/drafts.md`,
 * known gaps).
 *
 * Resuming is a round trip and, unlike a forward, cannot count on the message being cached: a
 * draft is opened from a list row, so the first open of one fetches it. It runs on a thread of its
 * own for that reason, and the composer appears when the answer does. A draft that could not be
 * opened is **said**, and the message opens for reading instead: a composer opened without the
 * draft's content would replace it with what was on screen the next time it saved.
 */
internal fun MainActivity.openOrResume(
    instance: MailcalApp,
    opened: OpenedMessage,
    conversation: List<ThreadMessage>? = null,
) {
    if (!drafts.showingDrafts) {
        openMessage(instance, opened, conversation)
        return
    }
    val composition = newComposition()
    // A directory of this composer's own, so two resumed drafts never share a staged file. The
    // cache is reclaimed by the OS, as a forward's staging is.
    val directory = File(File(cacheDir, "resumed-drafts"), composition)
    thread(name = "mailcal-resume-draft") {
        val resumed = resumeDraft(instance, composition, opened.account, opened.key, directory)
        Handler(Looper.getMainLooper()).post {
            if (resumed == null) {
                drafts.draftOpenFailed = true
                openMessage(instance, opened, conversation)
            } else {
                drafts.resumedDraft = ResumedDraft(composition, resumed)
            }
        }
    }
}

/**
 * The conversation half of [openOrResume]: a thread row in the Drafts folder opens its latest
 * message back into its composer, exactly as a flat row does.
 */
internal fun MainActivity.openOrResumeThread(instance: MailcalApp, thread: ThreadRow) {
    if (!drafts.showingDrafts) {
        openThread(instance, thread)
        return
    }
    openOrResume(instance, threadHead(thread), thread.messages)
}

/**
 * What a draft that could not be opened back into a composer says.
 *
 * One button, because there is nothing to decide: the message opens for reading behind it either
 * way, and the only thing it cannot do is be edited.
 */
@Composable
internal fun DraftOpenFailedDialog(onDismiss: () -> Unit) {
    val ctx = LocalContext.current
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(L10n.compose_draft_open_failed(ctx)) },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text(L10n.action_close(ctx)) }
        },
    )
}

/**
 * The composer a draft already on the server opens in, saving **over** that copy rather than
 * beside it.
 *
 * Two things separate it from every other new message. The composition is the one the core adopted
 * the stored draft into, never a fresh one, or the composer's first save would store a second copy
 * beside the one it is showing. And it seeds no signature, for the reason [WithdrawnMessagePane]
 * does: the body came back as the text of a message that was signed when it was first written, so
 * seeding one would put a second signature under it and the next save would store that.
 */
@Composable
internal fun MainActivity.ResumedDraftPane(instance: MailcalApp) {
    val resumed = drafts.resumedDraft ?: return
    val draft = resumed.draft
    RichComposeMessageDialog(
        mode = RichComposeMode.New,
        accounts = accounts,
        initialFrom = draft.account,
        initialTo = draft.to,
        initialCc = draft.cc,
        initialBcc = draft.bcc,
        initialSubject = draft.subject,
        initialBody = draft.bodyText,
        initialAttachments = draft.attachments,
        composition = resumed.composition,
        drafts = composerDrafts(instance),
        suggestionsFor = { prefix ->
            try {
                instance.recipientSuggestions(prefix)
            } catch (e: Exception) {
                Log.w(TAG, "recipient suggestions failed: ${e.javaClass.simpleName}")
                emptyList()
            }
        },
        onSubmitRich = { submission -> submitMail(instance, submission) },
        onDismiss = { drafts.resumedDraft = null },
    )
}
