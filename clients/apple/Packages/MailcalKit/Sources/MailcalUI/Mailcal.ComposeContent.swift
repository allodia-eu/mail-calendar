import MailcalBindings
import SwiftUI

extension ContentView {
    /// The composer for a new message / reply / reply-all / forward, rendered **in the detail
    /// column** on macOS (in place of the reading pane) and as a full-screen cover on iOS. Same
    /// view, different host: only where it is mounted differs.
    ///
    /// Not `private`: macOSLayout's `detailColumn` mounts it, and that lives in Mailcal.Layout.swift
    /// (Swift's `private` is file-scoped).
    ///
    /// Every mode opens its From dropdown on `model.sendAccount(preferring:)`: the account that
    /// received the mail for a reply/forward, the selected mailbox's account for a new message
    /// (in the unified inbox there is no such context, so the app-level default send account
    /// decides), and the first configured account as the last resort.
    /// The composer's recipient autosuggest, handed to every compose mode. Built once here rather
    /// than inline at each call site so all four modes ask the core the same way.
    var recipientSuggestions: (String) async -> [RecipientMatch] {
        { query in await model.recipientSuggestions(query) }
    }

    /// The signature library + the two lookups, handed to every compose mode. Built once here so
    /// all four modes seed, swap, and override signatures the same way; the composer decides which
    /// slot its mode reads (`MailboxModel.signatureSlot`).
    var composerSignatures: ComposerSignatures {
        ComposerSignatures(
            library: model.signatures.signatures,
            forAccount: { account, slot in model.accountSignature(account, slot) },
            byId: { id in model.signatureBody(id) }
        )
    }

    @ViewBuilder
    func composeContent(_ context: ComposeContext) -> some View {
        switch context {
        case .new:
            RichComposeView(
                title: L10n.compose_title_new(),
                mode: .new,
                accounts: model.accounts,
                initialFrom: model.sendAccount(preferring: model.selectedAccount)?.id,
                probe: draftProbe,
                suggestionsFor: recipientSuggestions,
                signatures: composerSignatures
            ) { recipients, subject, documentJson, files, from in
                if model.submitRich(recipients, subject, documentJson, files, from: from) {
                    compose = nil
                    return true
                }
                return false
            } cancel: { compose = nil }
        case let .reply(account, key, to, cc, quote, quoteStyle):
            RichComposeView(
                title: L10n.action_reply(),
                mode: .reply,
                accounts: model.accounts,
                initialFrom: model.sendAccount(preferring: account)?.id,
                initialTo: to,
                initialCc: cc,
                quote: quote,
                quoteStyle: quoteStyle,
                quoteStylePerMessage: model.quoteSettings.perMessage,
                probe: draftProbe,
                suggestionsFor: recipientSuggestions,
                signatures: composerSignatures
            ) { recipients, _, documentJson, files, from in
                if model.submitRichReply(account, key, recipients, documentJson, files, from: from) {
                    compose = nil
                    return true
                }
                return false
            } cancel: { compose = nil }
        case let .replyAll(account, key, to, cc, quote, quoteStyle):
            RichComposeView(
                title: L10n.action_reply_all(),
                mode: .replyAll,
                accounts: model.accounts,
                initialFrom: model.sendAccount(preferring: account)?.id,
                initialTo: to,
                initialCc: cc,
                quote: quote,
                quoteStyle: quoteStyle,
                quoteStylePerMessage: model.quoteSettings.perMessage,
                probe: draftProbe,
                suggestionsFor: recipientSuggestions,
                signatures: composerSignatures
            ) { recipients, _, documentJson, files, from in
                if model.submitRichReply(account, key, recipients, documentJson, files, from: from) {
                    compose = nil
                    return true
                }
                return false
            } cancel: { compose = nil }
        case let .agentDraft(request):
            // An assistant's draft, opened UNSENT so the user reads the recipients and the body
            // before anything leaves the machine. Structurally a new message, the same composer,
            // the same Send button, the same submit path, merely arriving prefilled.
            RichComposeView(
                title: L10n.compose_title_new(),
                mode: .new,
                accounts: model.accounts,
                initialFrom: model.sendAccount(preferring: request.draft.account)?.id,
                initialTo: request.draft.to,
                initialCc: request.draft.cc,
                initialBcc: request.draft.bcc,
                initialSubject: request.draft.subject,
                initialBody: request.draft.bodyText,
                probe: draftProbe,
                suggestionsFor: recipientSuggestions
            ) { recipients, subject, documentJson, files, from in
                if model.submitRich(recipients, subject, documentJson, files, from: from) {
                    compose = nil
                    return true
                }
                return false
            } cancel: { compose = nil }
        case let .forward(account, key, quote, quoteStyle):
            RichComposeView(
                title: L10n.action_forward(),
                mode: .forward,
                accounts: model.accounts,
                initialFrom: model.sendAccount(preferring: account)?.id,
                quote: quote,
                quoteStyle: quoteStyle,
                quoteStylePerMessage: model.quoteSettings.perMessage,
                probe: draftProbe,
                suggestionsFor: recipientSuggestions,
                signatures: composerSignatures
            ) { recipients, _, documentJson, files, from in
                if model.submitRichForward(account, key, recipients, documentJson, files, from: from) {
                    compose = nil
                    return true
                }
                return false
            } cancel: { compose = nil }
        }
    }
}
