// Keeping the composer's message on the server: what the composer hands its host, and the four
// core verbs it keeps a draft with (`docs/drafts.md`).
//
// Its own file rather than an addition to RichComposeScreen.kt, which is at the 500-line limit,
// and because the JVM suite can hold these values without composing anything.
package eu.allodia.mailcal

import android.content.Context
import android.webkit.WebView
import androidx.compose.runtime.Composable
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import kotlin.coroutines.resume
import kotlin.coroutines.suspendCoroutine
import kotlinx.coroutines.delay
import uniffi.mailcal_bindings.ComposerFileAttachment
import uniffi.mailcal_bindings.DraftStatus
import uniffi.mailcal_bindings.Recipients

/**
 * What the composer hands its host when the user presses Send: everything the three submit calls
 * name, in one value.
 *
 * A value rather than six positional arguments, because it is passed down four layers of
 * composables that do nothing with it but forward it, and because two of its fields read alike at
 * a call site and mean opposite things: [from] is which account sends, [composition] is which
 * composer wrote it. Transposing them compiles.
 */
internal data class ComposerSubmission(
    /** The account id picked in the From dropdown, or null to let the core derive it. */
    val from: String?,
    val recipients: Recipients,
    /** The Subject field as edited; a reply and a forward open with the core's derived one. */
    val subject: String,
    /** The rendered editor document. */
    val documentJson: String,
    /** The files the composer holds: picked, dropped, shared, forwarded or resumed. */
    val files: List<ComposerFileAttachment>,
    /**
     * The composition this message was written in, so an accepted send takes its stored draft out
     * of Drafts. Null only from a composer that keeps no draft.
     */
    val composition: String?,
)

/**
 * The four core verbs a composer keeps its draft with, each naming the composition the composer
 * minted for itself.
 *
 * Passed as a value rather than the activity, as [ComposerSignatures] is, so the composer stays
 * free of it; null turns draft saving off entirely, which is what a screenshot run and a test
 * want.
 */
internal class ComposerDrafts(
    /** Stores what the composer holds, superseding this composition's previous save. */
    val save: (composition: String, content: ComposerSubmission) -> Unit,
    /** Removes the stored copy from the server and forgets the composition. */
    val discard: (composition: String) -> Unit,
    /** Forgets the composition, leaving the stored draft in Drafts: what closing a composer means. */
    val close: (composition: String) -> Unit,
    /** How this composition's most recent save ended. */
    val status: (composition: String) -> DraftStatus,
    /**
     * The activity's count of `Surface::DraftStatus` signals. The signal names no composition, so
     * a composer watches this and then asks for its own state.
     */
    val version: Int,
    /**
     * How long the composer waits after the last change before it stores the draft.
     *
     * The core's constant, so the four clients cannot disagree about it, but read by the
     * **activity** and passed in rather than called for here: it is an FFI call, and the JVM suite
     * renders this composer without the cdylib loaded.
     */
    val idleSeconds: ULong,
)

/**
 * The composer's draft lifetime: noticing a change the host cannot see, storing the draft once the
 * composer has gone quiet, and keeping the hint in step.
 *
 * Every header field is Compose state and recomposes on its own, so the composer counts those
 * itself; the message body is a WebView and the page has no channel back to the host, so the
 * shared editor bundle counts its own mutations and the sampler here reads the count.
 *
 * A composable rather than three effects inline in RichComposeScreen.kt, which is at the 500-line
 * limit, and so the whole of the timer reads in one place.
 */
@Composable
internal fun ComposerDraftEffects(
    drafts: ComposerDrafts?,
    composition: String,
    webView: WebView?,
    /** How many changes the composer has seen. Every one restarts the idle interval. */
    changes: Int,
    onChanged: () -> Unit,
    onStatus: (DraftStatus) -> Unit,
    onIdle: () -> Unit,
) {
    if (drafts == null) return
    LaunchedEffect(webView) {
        val sample = draftSampleMillis(drafts.idleSeconds)
        var seen = -1
        while (true) {
            delay(sample)
            val view = webView ?: continue
            val now = suspendCoroutine { resume ->
                view.evaluateJavascript("window.composerRevision()") { encoded ->
                    resume.resume(encoded?.toIntOrNull() ?: -1)
                }
            }
            if (now >= 0 && now != seen) {
                // The first reading is the baseline the editor loaded with, never an edit:
                // without it every composer would store a draft moments after opening.
                if (seen >= 0) onChanged()
                seen = now
            }
        }
    }
    // The whole timer is a `delay` keyed on the change count: every change cancels and restarts
    // this effect, so the trigger is the pause and not the clock, and a composer nobody has
    // touched starts no timer at all.
    LaunchedEffect(changes) {
        if (changes > 0) {
            delay(drafts.idleSeconds.toLong() * 1000L)
            onIdle()
        }
    }
    // A `Surface::DraftStatus` signal says that *some* composition's save moved, not which, so
    // this composer asks for its own.
    LaunchedEffect(drafts.version) { onStatus(drafts.status(composition)) }
}

/**
 * The quiet line under the composer's header, or null when there is nothing to say.
 *
 * A hint and never a gate: no state here stops the composer being closed, and none of it is worth
 * a dialog. A composer that has saved nothing says nothing.
 */
internal fun composerDraftHint(status: DraftStatus, ctx: Context): String? = when (status) {
    DraftStatus.IDLE -> null
    DraftStatus.SAVING -> L10n.compose_draft_saving(ctx)
    DraftStatus.SAVED -> L10n.compose_draft_saved(ctx)
    DraftStatus.QUEUED -> L10n.compose_draft_queued(ctx)
    DraftStatus.FAILED -> L10n.compose_draft_failed(ctx)
}

/**
 * The two quiet lines under the composer's header: whatever went wrong, and how the draft's last
 * save ended.
 *
 * One composable because they are one place on screen and the composer should not have to decide
 * their order. Both are hints and neither is a gate: no state here stops the composer being
 * closed, and none of it is worth a dialog.
 */
@Composable
internal fun ComposerNotices(error: String?, draftStatus: DraftStatus) {
    val ctx = LocalContext.current
    error?.let { message ->
        Text(
            text = message,
            color = MaterialTheme.colorScheme.error,
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.padding(bottom = 8.dp),
        )
    }
    composerDraftHint(draftStatus, ctx)?.let { hint ->
        Text(
            text = hint,
            color = if (draftStatus == DraftStatus.FAILED) {
                MaterialTheme.colorScheme.error
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant
            },
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.padding(bottom = 8.dp),
        )
    }
}

/**
 * How long a composer waits after the last change before it stores the draft, and how often it
 * looks at the editor to notice one.
 *
 * The interval is the core's constant, so the four clients cannot disagree about it. The sample is
 * a third of it, which is what the lag costs: a draft reaches the server between one and
 * one-and-a-third intervals after the last keystroke, and never during typing.
 *
 * Sampled rather than pushed because the message body is a WebView with no channel back to the
 * host: the shared editor bundle counts its own mutations and this reads the count.
 */
internal fun draftSampleMillis(idleSeconds: ULong): Long =
    maxOf(1L, idleSeconds.toLong() / 3L) * 1000L
