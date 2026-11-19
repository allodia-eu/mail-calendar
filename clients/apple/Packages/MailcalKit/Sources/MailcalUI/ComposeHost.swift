// The composer, as a view rather than as a method on the shell.
//
// One `ComposeContext` in, the `RichComposeView` it describes out, and `dismiss` when the draft is
// gone. Three hosts mount it and none of them needs to know which it is: the macOS detail column,
// the iPhone full-screen cover and iPad page sheet (`taskPresentation`), and a detached composer window on the desktop
// (`docs/reading-window.md`).
//
// Every composer is built the same way, from what the core opened it with, whichever route reached
// it: the From dropdown opens on the opening's account, which the core has already checked can
// send, and every composer sends through `submitComposition` on the composition it opened on.

import MailcalBindings
import SwiftUI

struct ComposeHost: View {
    var model: MailboxModel
    let context: ComposeContext
    /// Called once the draft is finished with, sent or cancelled. Closing the column, the cover
    /// or the window is the host's to do, because only the host knows which of those it is.
    let dismiss: () -> Void
    /// Moves the composer into a window of its own, answering whether it moved; `nil` where there
    /// is nowhere to move it: a phone, an iPad, and a composer already in a window.
    var popOut: ((ComposerState) async -> Bool)?
    #if os(iOS)
    /// Settings opened over the composer for a Draft a reply remedy, at that remedy's page.
    @State private var remedySettings: SettingsCategory?
    #endif

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
            save: { composition, recipients, subject, documentJson, from in
                model.saveDraft(composition, recipients, subject, documentJson, from: from)
            },
            leave: { composition, recipients, subject, documentJson, from in
                model.saveDraftAndClose(composition, recipients, subject, documentJson, from: from)
            },
            discard: { model.discardDraft($0) },
            close: { model.closeComposition($0) },
            status: { model.draftStatus($0) },
            isStored: { model.draftIsStored($0) },
            version: model.draftStatusVersion
        )
    }

    /// Draft a reply's core verbs. A remedy the composer cannot take in place opens Settings at
    /// its page with the composer kept: over the composer on a phone or a tablet, whose composer
    /// is itself a sheet, and in the window's own Settings on a Mac, where it sits in the window.
    private var replyDrafts: ComposerReplyDrafts {
        ComposerReplyDrafts(
            read: { model.composerReplyDraft($0, from: $1) },
            change: { model.changeReplyDraft($0) },
            report: { model.reportReplyDraft($0, $1) },
            remedy: { remedy in
                guard let category = settingsCategory(for: remedy) else {
                    if remedy == .signInToAllodia { model.signInToAllodia() }
                    return
                }
                #if os(iOS)
                remedySettings = category
                #else
                model.settingsRequest = category
                #endif
            },
            version: model.replyDraftSignals
        )
    }

    var body: some View {
        let opening = context.opening
        // A moved composer offers the signature picker exactly when the one it left did; nothing
        // is seeded either way, because its document already carries what it had.
        let offersSignatures = opening.seedsSignature
            || (opening.moved.map { $0.signature != .untouched } ?? false)
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
            initialError: context.initialError,
            composition: opening.composition,
            quote: context.quote,
            quoteAttribution: context.quoteAttribution,
            quoteStyle: context.quoteStyle,
            quoteStylePerMessage: context.quoteStylePerMessage,
            suggestionsFor: recipientSuggestions,
            // A stored message carries the signature it was written with, and an assistant's
            // draft its author's own sign-off: neither is seeded one, offered the picker, or has
            // its signature region touched on a From change (`ComposeOpening.seedsSignature`). A
            // moved composer is offered what the one it left was (`offersSignatures`).
            signatures: offersSignatures ? signatures : nil,
            drafts: drafts,
            files: ComposerFileVerbs(
                read: { model.composerFiles($0) },
                add: { await model.addComposerFiles($0, $1) },
                remove: { model.removeComposerFile($0, $1) }
            ),
            importance: ComposerImportanceVerbs(
                read: { model.composerImportance($0) },
                set: { model.setComposerImportance($0, $1) }
            ),
            moved: opening.moved,
            popOut: popOut,
            replyDrafts: replyDrafts
        ) { submission in
            submitted(model.submitComposition(submission))
        } cancel: { dismiss() }
        #if os(iOS)
        .sheet(item: $remedySettings) { category in
            SettingsHubView(
                model: model, close: { remedySettings = nil },
                removeAccount: { model.removeAccount($0) }, openOn: category)
        }
        #endif
    }

    /// Closes the composer when the submit was accepted, and answers what the editor asked: a
    /// refused submit (nothing to send to, no account) leaves the draft on screen with its error.
    private func submitted(_ accepted: Bool) -> Bool {
        if accepted { dismiss() }
        return accepted
    }
}
