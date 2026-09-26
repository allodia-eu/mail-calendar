// Keeping the message being composed on the server: the core verbs, as the composer calls them
// (`docs/drafts.md`).
//
// Every one of them names a composition, which is the host's handle on one open composer. The
// core mints none: a composer takes an id when it opens and keeps it until it closes, and that
// is what lets two composers save without superseding each other's draft.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// Stores what the composer holds in the Drafts folder, replacing what this composition's
    /// previous save left there.
    ///
    /// Both the idle timer and the Save button reach this, and the core cannot tell them apart.
    /// Fire and forget: it returns as soon as the document validates, and the outcome arrives as
    /// a `Surface::DraftStatus` signal. A save with no network is queued, not lost.
    ///
    /// Always the file-carrying call, even from a composer holding none. A save replaces the
    /// stored copy, so one that left the files out would take them off a draft the user is still
    /// writing, and a composer can pick a file at any moment.
    func saveDraft(
        _ composition: String,
        _ recipients: Recipients,
        _ subject: String,
        _ documentJson: String,
        _ files: [ComposerFileAttachment],
        from: String?
    ) {
        guard let app else { return }
        do {
            try app.saveDraftWithFiles(
                composition: composition,
                recipients: recipients,
                subject: subject,
                documentJson: documentJson,
                files: files,
                from: from
            )
        } catch {
            print("[Mailcal] draft save failed: \(type(of: error))")
        }
    }

    /// Removes this composition's stored draft from the server and forgets the composition.
    ///
    /// A composition that never saved reaches no server, so this is safe on a composer the user
    /// opened and typed nothing in.
    func discardDraft(_ composition: String) {
        do {
            try app?.discardDraft(composition: composition)
        } catch {
            print("[Mailcal] draft discard failed: \(type(of: error))")
        }
    }

    /// Forgets the composition, leaving the stored draft where it is.
    ///
    /// What closing a composer means: the draft stays in Drafts. Called however the composer
    /// went, sent, cancelled or dismissed, because without it the core keeps a record per
    /// composer for the life of the process.
    func closeComposition(_ composition: String) {
        do {
            try app?.closeComposition(composition: composition)
        } catch {
            print("[Mailcal] composition close failed: \(type(of: error))")
        }
    }

    /// How `composition`'s most recent save ended.
    ///
    /// A `Surface::DraftStatus` signal says that some composition's save moved, not which, so
    /// every open composer re-pulls its own by naming it. One that has saved nothing reads
    /// `.idle`, never the composer beside it.
    func draftStatus(_ composition: String) -> DraftStatus {
        guard let app, let status = try? app.draftStatus(composition: composition) else {
            return .idle
        }
        return status
    }

    /// Opens the stored draft `key` (on `account`) into `composition`, so the composer about to
    /// show it saves over that copy rather than beside it.
    ///
    /// `nil` means the draft could not be opened, which the caller says rather than showing an
    /// empty composer: nothing is adopted on a failure, and a composer opened without the
    /// draft's content would replace it with what is on screen.
    ///
    /// Off the main actor, and unlike a forward's staging it cannot count on a warm cache: a
    /// draft is opened from a list row, so the first open of one fetches the message.
    func resumeDraft(_ composition: String, _ account: String, _ key: String) async -> DraftResume? {
        guard let app else { return nil }
        let directory = draftStagingDirectory().path
        return await Task.detached {
            do {
                return try app.resumeDraft(
                    composition: composition,
                    account: account,
                    key: key,
                    stagingDirectory: directory
                )
            } catch {
                print("[Mailcal] draft resume failed: \(type(of: error))")
                return nil
            }
        }.value
    }

    /// A directory of this composer's own under the app's temporary storage, so two resumed
    /// drafts never share a staged file. The OS reclaims what is left behind, as it does for a
    /// forward's.
    private func draftStagingDirectory() -> URL {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("resumed-drafts", isDirectory: true)
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
    }
}
