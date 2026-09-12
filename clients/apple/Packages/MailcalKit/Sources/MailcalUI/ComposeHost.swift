// The composer, as a view rather than as a method on the shell.
//
// One `ComposeContext` in, the right `RichComposeView` out, and `dismiss` when the draft is gone.
// It was `ContentView.composeContent(_:)`, which is why the shell is still where a *new* draft is
// decided on; what moved here is the part that has three hosts and no business knowing which one
// it is in: the macOS detail column, the iPhone and iPad full-screen cover, and a detached
// composer window on the desktop (`docs/reading-window.md`).
//
// Every mode opens its From dropdown on `model.sendAccount(preferring:)`: the account that
// received the mail for a reply/forward, the selected mailbox's account for a new message (in the
// unified inbox there is no such context, so the app-level default send account decides), and the
// first configured account as the last resort.

import MailcalBindings
import SwiftUI

struct ComposeHost: View {
    var model: MailboxModel
    let context: ComposeContext
    /// Where this composer reports its edits, for a host that has to ask "discard this draft?"
    /// before taking it away. The shell passes one; a composer window passes none, because
    /// nothing takes its draft away except the person closing it (`docs/reading-window.md`).
    var probe: ComposeDraftProbe?
    /// Called once the draft is finished with, sent or cancelled. Closing the column, the cover
    /// or the window is the host's to do, because only the host knows which of those it is.
    let dismiss: () -> Void

    /// The composer's recipient autosuggest, handed to every compose mode. Built once here rather
    /// than inline at each call site so all four modes ask the core the same way.
    private var recipientSuggestions: (String) async -> [RecipientMatch] {
        { query in await model.recipientSuggestions(query) }
    }

    /// The signature library + the two lookups, handed to every compose mode. Built once here so
    /// all four modes seed, swap, and override signatures the same way; the composer decides which
    /// slot its mode reads (`MailboxModel.signatureSlot`).
    private var signatures: ComposerSignatures {
        ComposerSignatures(
            library: model.signatures.signatures,
            forAccount: { account, slot in model.accountSignature(account, slot) },
            byId: { id in model.signatureBody(id) }
        )
    }

    var body: some View {
        switch context {
        case .new:
            newMessage(title: L10n.compose_title_new(), from: model.selectedAccount)
        case let .reply(account, key, to, cc, subject, quote, quoteStyle):
            replyMessage(
                title: L10n.action_reply(), mode: .reply, account: account, key: key,
                to: to, cc: cc, subject: subject, quote: quote, quoteStyle: quoteStyle
            )
        case let .replyAll(account, key, to, cc, subject, quote, quoteStyle):
            replyMessage(
                title: L10n.action_reply_all(), mode: .replyAll, account: account, key: key,
                to: to, cc: cc, subject: subject, quote: quote, quoteStyle: quoteStyle
            )
        case let .agentDraft(request):
            // An assistant's draft, opened UNSENT so the user reads the recipients and the body
            // before anything leaves the machine. Structurally a new message, the same composer,
            // the same Send button, the same submit path, merely arriving prefilled.
            prefilled(
                from: request.draft.account,
                to: request.draft.to, cc: request.draft.cc, bcc: request.draft.bcc,
                subject: request.draft.subject, bodyText: request.draft.bodyText
            )
        case let .mailLink(request):
            // A mail link opens a new message someone else addressed. The core has already
            // dropped every header a link may not set (docs/composer-security.md, Gate 12), so
            // what reaches here is only To/Cc/Bcc/Subject/Body, and every field stays editable.
            // Nothing is sent: the user still presses Send.
            prefilled(
                from: nil,
                to: request.prefill.to, cc: request.prefill.cc, bcc: request.prefill.bcc,
                subject: request.prefill.subject, bodyText: request.prefill.body
            )
        case let .forward(account, key, subject, quote, quoteStyle, attachments):
            forwardMessage(
                account: account, key: key, subject: subject,
                quote: quote, quoteStyle: quoteStyle, attachments: attachments
            )
        }
    }

    private func newMessage(title: String, from: String?) -> some View {
        RichComposeView(
            title: title,
            mode: .new,
            accounts: model.accounts,
            initialFrom: model.sendAccount(preferring: from)?.id,
            probe: probe,
            suggestionsFor: recipientSuggestions,
            signatures: signatures
        ) { recipients, subject, documentJson, files, from in
            submitted(model.submitRich(recipients, subject, documentJson, files, from: from))
        } cancel: { dismiss() }
    }

    /// A new message someone else addressed: an assistant's draft, or a `mailto:` link.
    private func prefilled(
        from: String?,
        to: String, cc: String, bcc: String, subject: String, bodyText: String
    ) -> some View {
        RichComposeView(
            title: L10n.compose_title_new(),
            mode: .new,
            accounts: model.accounts,
            initialFrom: model.sendAccount(preferring: from)?.id,
            initialTo: to,
            initialCc: cc,
            initialBcc: bcc,
            initialSubject: subject,
            initialBody: bodyText,
            probe: probe,
            suggestionsFor: recipientSuggestions,
            signatures: signatures
        ) { recipients, subject, documentJson, files, from in
            submitted(model.submitRich(recipients, subject, documentJson, files, from: from))
        } cancel: { dismiss() }
    }

    private func replyMessage(
        title: String, mode: RichComposeMode, account: String, key: String,
        to: String, cc: String, subject: String, quote: String?, quoteStyle: QuoteStyleKind
    ) -> some View {
        RichComposeView(
            title: title,
            mode: mode,
            accounts: model.accounts,
            initialFrom: model.sendAccount(preferring: account)?.id,
            initialTo: to,
            initialCc: cc,
            initialSubject: subject,
            quote: quote,
            quoteStyle: quoteStyle,
            quoteStylePerMessage: model.quoteSettings.perMessage,
            probe: probe,
            suggestionsFor: recipientSuggestions,
            signatures: signatures
        ) { recipients, subject, documentJson, files, from in
            submitted(
                model.submitRichReply(
                    account, key, recipients, subject, documentJson, files, from: from
                )
            )
        } cancel: { dismiss() }
    }

    private func forwardMessage(
        account: String, key: String, subject: String,
        quote: String?, quoteStyle: QuoteStyleKind, attachments: ForwardAttachments
    ) -> some View {
        RichComposeView(
            title: L10n.action_forward(),
            mode: .forward,
            accounts: model.accounts,
            initialFrom: model.sendAccount(preferring: account)?.id,
            initialSubject: subject,
            initialAttachments: attachments.files,
            initialError: attachments.failed ? L10n.compose_forward_attachments_failed() : nil,
            quote: quote,
            quoteStyle: quoteStyle,
            quoteStylePerMessage: model.quoteSettings.perMessage,
            probe: probe,
            suggestionsFor: recipientSuggestions,
            signatures: signatures
        ) { recipients, subject, documentJson, files, from in
            submitted(
                model.submitRichForward(
                    account, key, recipients, subject, documentJson, files, from: from
                )
            )
        } cancel: { dismiss() }
    }

    /// Closes the composer when the submit was accepted, and answers what the editor asked: a
    /// refused submit (nothing to send to, no account) leaves the draft on screen with its error.
    private func submitted(_ accepted: Bool) -> Bool {
        if accepted { dismiss() }
        return accepted
    }
}
