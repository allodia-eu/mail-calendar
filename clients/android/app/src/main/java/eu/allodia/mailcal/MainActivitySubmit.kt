// The one send every composer makes, whichever way it was opened (docs/drafts.md).
//
// It names the composition the composer opened on: the conversation a reply or forward belongs to
// and the parts its quoted pictures came from were joined to it when the composer opened, and an
// accepted send takes the stored draft it may have saved out of Drafts.
package eu.allodia.mailcal

import android.util.Log
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.MailcalException

private const val TAG = "Mailcal"

/** Queues what a composer holds. False means it was refused before anything was sent. */
internal fun submitComposition(instance: MailcalApp, submission: ComposerSubmission): Boolean = try {
    instance.submitComposition(
        submission.composition,
        submission.recipients,
        submission.subject,
        submission.documentJson,
        submission.files,
        submission.from,
    )
    true
} catch (e: MailcalException) {
    Log.w(TAG, "rich composer submit failed: ${e.javaClass.simpleName}")
    false
}
