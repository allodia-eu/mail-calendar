// Keeping the message being composed on the server: the core verbs, as the composer calls them
// (`docs/drafts.md`).
//
// Every one of them names a composition, which is the host's handle on one open composer. The core
// mints none: a composer takes an id when it opens and keeps it until it closes, and that is what
// lets two composers save without superseding each other's draft.
package eu.allodia.mailcal

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
import uniffi.mailcal_bindings.ComposeOrigin
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

/** A fresh composition id, minted as a composer opens. */
internal fun newComposition(): String = UUID.randomUUID().toString()

/**
 * Where the core writes the files a composer opens holding: a forward's originals, a Drafts-folder
 * row's, or a queued send's moved back into Drafts. A directory of each composer's own, so two of
 * them never share a staged file; the cache is reclaimed by the OS.
 */
internal fun MainActivity.composerStagingDirectory(name: String): File =
    File(File(cacheDir, "composer-staging"), name)

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
    // Whether to say that a draft could not be opened back into a composer. Raised instead of
    // opening an empty one, whose next save would replace the draft.
    var draftOpenFailed by mutableStateOf(false)
}

/**
 * Opens a Drafts-folder row back into its composer, and every other row for reading.
 *
 * Told by the **folder**, not by the row: the core answers per message but the list does not carry
 * it, so a draft met in a search result or inside a thread opens read-only (`docs/drafts.md`,
 * known gaps).
 *
 * A draft that could not be opened is **said**, and the message opens for reading instead: a
 * composer opened without the draft's content would replace it with what was on screen the next
 * time it saved.
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
    openComposer(instance, ComposeOrigin.Draft(opened.account, opened.key)) {
        drafts.draftOpenFailed = true
        openMessage(instance, opened, conversation)
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
 * What a composer that could not be opened says: `title` is the draft-specific line for a
 * Drafts-folder row, and the general one for every other way in.
 *
 * One button, because there is nothing to decide. A draft opens for reading behind it, and
 * nothing else was lost: no composer was shown, so nothing was written.
 */
@Composable
internal fun ComposerOpenFailedDialog(title: String, onDismiss: () -> Unit) {
    val ctx = LocalContext.current
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(title) },
        confirmButton = {
            TextButton(onClick = onDismiss) { Text(L10n.action_close(ctx)) }
        },
    )
}
