// The composer, as a view rather than as a method on the shell.
//
// One `ComposeContext` in, the `RichComposeView` it describes out, and `dismiss` when the draft is
// gone. Three hosts mount it and none of them needs to know which it is: the macOS detail column,
// the iPhone full-screen cover and iPad page sheet (`taskPresentation`), and a detached composer window on the desktop
// (`docs/reading-window.md`).
//
// Every composer is built the same way, from what the core opened it with, whichever route reached
// it: the From dropdown opens on the opening's account as a sending account
// (`model.sendAccount(preferring:)`), and every composer sends through `submitComposition` on the
// composition it opened on.

import MailcalBindings
import SwiftUI

struct ComposeHost: View {
    var model: MailboxModel
    let context: ComposeContext
    /// Called once the draft is finished with, sent or cancelled. Closing the column, the cover
    /// or the window is the host's to do, because only the host knows which of those it is.
    let dismiss: () -> Void

    /// The composer's recipient autosuggest.
    private var recipientSuggestions: (String) async -> [RecipientMatch] {
        { query in await model.recipientSuggestions(query) }
    }

    /// The signature library + the two lookups; the composer decides which slot its mode reads
    /// (`signatureSlot(for:)`).
    private var signatures: ComposerSignatures {
        ComposerSignatures(
            library: model.signatures.signatures,
            forAccount: { account, slot in model.accountSignature(account, slot) },
            byId: { id in model.signatureBody(id) }
        )
    }

    /// The core verbs this composer keeps its message on the server with; the composition each
    /// one names is the one the composer opened on.
    private var drafts: ComposerDrafts {
        ComposerDrafts(
            save: { composition, recipients, subject, documentJson, files, from in
                model.saveDraft(composition, recipients, subject, documentJson, files, from: from)
            },
            leave: { composition, recipients, subject, documentJson, files, from in
                model.saveDraftAndClose(
                    composition, recipients, subject, documentJson, files, from: from
                )
            },
            discard: { model.discardDraft($0) },
            close: { model.closeComposition($0) },
            status: { model.draftStatus($0) },
            isStored: { model.draftIsStored($0) },
            version: model.draftStatusVersion
        )
    }

    var body: some View {
        let opening = context.opening
        RichComposeView(
            title: context.title,
            mode: context.mode,
            accounts: model.accounts,
            initialFrom: context.initialFrom,
            initialTo: opening.to,
            initialCc: opening.cc,
            initialBcc: opening.bcc,
            initialSubject: opening.subject,
            initialBody: context.plainBody ?? "",
            initialHTML: opening.bodyHtml,
            initialAttachments: opening.attachments,
            addedAttachments: context.addedAttachments,
            initialError: context.initialError,
            composition: opening.composition,
            quote: context.quote,
            quoteAttribution: context.quoteAttribution,
            quoteStyle: context.quoteStyle,
            quoteStylePerMessage: context.quoteStylePerMessage,
            suggestionsFor: recipientSuggestions,
            // A stored message carries the signature it was written with, and an assistant's
            // draft its author's own sign-off: neither is seeded one, offered the picker, or has
            // its signature region touched on a From change (`ComposeOpening.seedsSignature`).
            signatures: opening.seedsSignature ? signatures : nil,
            drafts: drafts
        ) { submission in
            submitted(model.submitComposition(submission))
        } cancel: { dismiss() }
    }

    /// Closes the composer when the submit was accepted, and answers what the editor asked: a
    /// refused submit (nothing to send to, no account) leaves the draft on screen with its error.
    private func submitted(_ accepted: Bool) -> Bool {
        if accepted { dismiss() }
        return accepted
    }
}
