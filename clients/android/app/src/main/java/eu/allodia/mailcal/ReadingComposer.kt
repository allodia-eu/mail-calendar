// The reading screen's composer overlay: reply, reply-all and forward over the open message.
// Split out of ReadingScreen.kt, which is at the line limit, and it is a seam worth having: this
// is the one place that decides what the composer opens holding, and the list rows have their own
// (MailRows.kt).
package eu.allodia.mailcal

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.platform.LocalContext
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.ComposerFileAttachment
import uniffi.mailcal_bindings.QuoteSettings
import uniffi.mailcal_bindings.ReadingSnapshot
import uniffi.mailcal_bindings.RecipientMatch
import uniffi.mailcal_bindings.RecipientSuggestion
import uniffi.mailcal_bindings.Recipients
import uniffi.mailcal_bindings.forwardSubject
import uniffi.mailcal_bindings.replySubject

// The rich composer over the open message (its own window), mirroring the list rows: the same
// hardened editor, its rendered document riding submitRichReply/submitRichForward. Reply and
// reply-all open with To/Cc pre-filled from the core.
//
// `forwardSeed` is what staging answered for a forward (ReadingScreen's Forward action). It is
// read through the mode rather than directly, so a reply opened after a forward does not inherit
// the files left behind in it.
@Composable
internal fun ReadingComposerOverlay(
    mode: RichComposeMode,
    message: OpenedMessage,
    reading: ReadingSnapshot?,
    accounts: List<AccountRow>,
    quoteSettings: QuoteSettings,
    forwardSeed: ForwardSeed,
    replyRecipients: (account: String, key: String, replyAll: Boolean) -> RecipientSuggestion?,
    suggestionsFor: ((String) -> List<RecipientMatch>)?,
    signatures: ComposerSignatures?,
    // Screenshot only: sample text to pre-fill the reply composer's body with (plain text).
    composerInitialText: String?,
    onClose: () -> Unit,
    onReply: (
        account: String,
        key: String,
        from: String?,
        recipients: Recipients,
        subject: String,
        documentJson: String,
        files: List<ComposerFileAttachment>,
    ) -> Boolean,
    onForward: (
        account: String,
        key: String,
        from: String?,
        recipients: Recipients,
        subject: String,
        documentJson: String,
        files: List<ComposerFileAttachment>,
    ) -> Boolean,
) {
    val ctx = LocalContext.current
    val prefill = remember(message.key, mode) {
        if (mode == RichComposeMode.Reply || mode == RichComposeMode.ReplyAll) {
            replyRecipients(message.account, message.key, mode == RichComposeMode.ReplyAll)
        } else {
            null
        }
    }
    val isForward = mode == RichComposeMode.Forward
    val seed = if (isForward) forwardSeed else ForwardSeed()
    // Seed the quoted original from this message's already-sanitised reading body (null when the
    // body hasn't arrived yet, the composer then opens empty, as on a list-row reply).
    val quote = ComposerQuote.seedJson(
        ctx = ctx,
        style = quoteSettings.style,
        message = message,
        reading = reading,
        isForward = isForward,
        initialText = if (isForward) null else composerInitialText,
    )
    RichComposeMessageDialog(
        suggestionsFor = suggestionsFor,
        signatures = signatures,
        mode = mode,
        accounts = accounts,
        // A reply/forward opens on the account that received the mail, the address it was sent
        // to. The user can switch it in the From dropdown.
        initialFrom = message.account,
        initialTo = prefill?.to ?: "",
        initialCc = prefill?.cc ?: "",
        // Derived by the CORE, not here: the field is editable, so what it opens with is what
        // gets sent unless the user changes it, and a client-side "Re: " + subject differs from
        // the core's on a reply to a reply.
        initialSubject = if (isForward) {
            forwardSubject(message.subject)
        } else {
            replySubject(message.subject)
        },
        initialAttachments = seed.files,
        initialError = if (seed.failed) L10n.compose_forward_attachments_failed(ctx) else null,
        quote = quote,
        quoteStyle = quoteSettings.style,
        quoteStylePerMessage = quoteSettings.perMessage,
        onDismiss = onClose,
        onSubmitRich = { from, recipients, subject, documentJson, files ->
            val sent = if (isForward) {
                onForward(message.account, message.key, from, recipients, subject, documentJson, files)
            } else {
                onReply(message.account, message.key, from, recipients, subject, documentJson, files)
            }
            if (sent) {
                onClose()
            }
            sent
        },
    )
}
