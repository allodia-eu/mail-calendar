// The Outbox branch of the mailbox tab: the queued list, and the composer a withdrawn message
// comes back into (docs/sending.md).
//
// Split out of MainActivityMailboxTab.kt so each file stays under the 500-line limit, and because
// these two are one story: Edit is pressed on the list, and what answers it is a composer the list
// no longer holds anything for.
package eu.allodia.mailcal

import android.util.Log
import androidx.compose.material3.DrawerState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import kotlinx.coroutines.launch
import org.json.JSONObject
import uniffi.mailcal_bindings.ComposeRequest
import uniffi.mailcal_bindings.ComposerFileAttachment
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.SignatureRow

private const val TAG = "Mailcal"

/** Every account's unsent messages, and what each one offers. */
@Composable
internal fun MainActivity.OutboxPane(instance: MailcalApp, drawerState: DrawerState) {
    val scope = rememberCoroutineScope()
    OutboxScreen(
        queued = outbox,
        accounts = accounts,
        onOpenDrawer = { scope.launch { drawerState.open() } },
        onAct = { account, op, action ->
            // The composition is not known until the core answers, so the directory is named
            // for this request alone.
            val staging = resumedDraftDirectory(newComposition()).absolutePath
            instance.dispatch(Intent.Outbox(outboxIntent(account, op, action, staging)))
        },
    )
}

/**
 * The composer a queued send moved back into Drafts opens in, so the user can change it and send it
 * again.
 *
 * It opens as a resumed draft does (`ResumedDraftPane`): on the composition the core saved the
 * draft under, never a fresh one, and holding every file the core staged, because its first save
 * replaces that draft and a file left out would be taken off it. The request is dismissed only once
 * this composer has closed, so a client that never draws it leaves the offer standing.
 *
 * It seeds no signature: the body already carries whatever was on it when the message was queued,
 * and a second one would go out with it.
 */
@Composable
internal fun MainActivity.WithdrawnMessagePane(instance: MailcalApp, library: List<SignatureRow>) {
    val request = withdrawnMessage ?: return
    // The recipients, the subject and the body are the user's own mail; none is logged.
    Log.i(TAG, "outbox: a withdrawn message is going back into the composer")
    val seed = reopenedSeed(request)
    RichComposeMessageDialog(
        mode = RichComposeMode.New,
        accounts = accounts,
        initialFrom = seed.from,
        initialTo = seed.to,
        initialCc = seed.cc,
        initialBcc = seed.bcc,
        initialSubject = seed.subject,
        initialBody = seed.text,
        initialHtml = seed.html,
        initialAttachments = seed.attachments,
        composition = seed.composition,
        suggestionsFor = { prefix ->
            try {
                instance.recipientSuggestions(prefix)
            } catch (e: Exception) {
                Log.w(TAG, "recipient suggestions failed: ${e.javaClass.simpleName}")
                emptyList()
            }
        },
        signatures = composerSignatures(instance, library),
        // Saves over the draft the core stored, and sending takes it away (docs/drafts.md).
        drafts = composerDrafts(instance),
        onSubmitRich = { submission -> submitMail(instance, submission) },
        onDismiss = {
            withdrawnMessage = null
            instance.dispatch(Intent.DismissComposeRequest)
        },
    )
}

/** The fields a reopened message opens its composer with. */
internal data class ReopenedSeed(
    /** The composition the core joined the stored copy to, adopted rather than minted. */
    val composition: String,
    val from: String,
    val to: String,
    val cc: String,
    val bcc: String,
    val subject: String,
    /** The body's HTML, which the editor reads back into its document; empty when it has none. */
    val html: String,
    /** The body's text, which the editor opens when there is no HTML. */
    val text: String,
    val attachments: List<ComposerFileAttachment>,
)

/**
 * What a composer reopened on an existing message opens with: a draft resumed from Drafts
 * ([ResumedDraftPane]) or a message moved back out of the Outbox ([WithdrawnMessagePane]). The core
 * answers both with one [ComposeRequest], and both panes read it through this, so neither can open
 * less of the message than the other.
 *
 * A function the panes themselves call, rather than a description of what they do: the JVM suite
 * can read this without composing a WebView, and a mirror of the seeding would be free to drift
 * from it. The failure it guards is a field silently dropped on the way, which looks like an
 * emptier composer and nothing else.
 *
 * `from` is the account the message is stored on, never the selected mailbox's: this list holds
 * every account's mail at once, so the identity it was written on is the only right answer, and
 * the only one the recipient already expects.
 */
internal fun reopenedSeed(request: ComposeRequest): ReopenedSeed = ReopenedSeed(
    composition = request.composition,
    from = request.account,
    to = request.to,
    cc = request.cc,
    bcc = request.bcc,
    subject = request.subject,
    html = request.bodyHtml,
    text = request.bodyText,
    attachments = request.attachments,
)

/**
 * The argument `setComposerBody` takes for a reopened message: its HTML, which the editor reads
 * back into its document, and its text for when it has none, as one JSON object.
 */
internal fun reopenedBodySeed(html: String, text: String): String =
    JSONObject().put("html", html).put("text", text).toString()
