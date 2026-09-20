// The three rich-composer submits, as the mailbox tab, the reading tab and the Outbox all hand
// them to a composer.
//
// One definition rather than a copy per host. Each of them names the composition the message was
// written in, so an accepted send takes the stored draft out of Drafts (docs/drafts.md), and a
// copy that had been left without it would send correctly and leave a duplicate behind that
// nothing on screen would mention.
package eu.allodia.mailcal

import android.util.Log
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.MailcalException

private const val TAG = "Mailcal"

/** Queues a new message. False means it was refused before anything was sent. */
internal fun submitMail(instance: MailcalApp, submission: ComposerSubmission): Boolean = try {
    instance.submitRichMailWithFiles(
        submission.recipients,
        submission.subject,
        submission.documentJson,
        submission.files,
        submission.from,
        submission.composition,
    )
    true
} catch (e: MailcalException) {
    Log.w(TAG, "rich composer submit failed: ${e.javaClass.simpleName}")
    false
}

/**
 * Queues a reply (or reply-all) to the message `account`/`key`, letting the core derive the
 * threading from the original.
 *
 * `submission.from` may name a different account: the core still resolves the original, and its
 * `Re:` subject and `In-Reply-To`/`References` chain, in the account that holds it, so a
 * cross-account reply still threads.
 */
internal fun submitReply(
    instance: MailcalApp,
    account: String,
    key: String,
    submission: ComposerSubmission,
): Boolean = try {
    instance.submitRichReplyWithFiles(
        account,
        key,
        submission.recipients,
        submission.documentJson,
        submission.files,
        submission.from,
        submission.subject,
        submission.composition,
    )
    true
} catch (e: MailcalException) {
    Log.w(TAG, "rich reply submit failed: ${e.javaClass.simpleName}")
    false
}

/** Queues a forward of the message `account`/`key`, to recipients the user entered fresh. */
internal fun submitForward(
    instance: MailcalApp,
    account: String,
    key: String,
    submission: ComposerSubmission,
): Boolean = try {
    instance.submitRichForwardWithFiles(
        account,
        key,
        submission.recipients,
        submission.documentJson,
        submission.files,
        submission.from,
        submission.subject,
        submission.composition,
    )
    true
} catch (e: MailcalException) {
    Log.w(TAG, "rich forward submit failed: ${e.javaClass.simpleName}")
    false
}
