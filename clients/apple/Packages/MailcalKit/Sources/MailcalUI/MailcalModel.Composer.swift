import Foundation
import MailcalBindings

extension MailboxModel {
    /// Sends what a composer holds, the one send every composer makes, on the composition it
    /// opened on. Validation happens synchronously in Rust; attachment bytes are read by Rust from
    /// the host-selected file paths so they do not cross Swift FFI memory.
    ///
    /// The core joined `submission.composition` to the message a reply answers or a forward passes
    /// on when the composer opened, so the send threads and carries the quoted pictures whichever
    /// way the composer was reached, and an accepted send takes its stored draft out of Drafts
    /// (`docs/drafts.md`).
    ///
    /// `submission.from` is the account the user picked in the composer's From dropdown; it
    /// decides both the `From:` identity and the outbox the message goes out through. `nil` lets
    /// the core derive it. An id naming an account that is no longer configured fails the send
    /// rather than substituting another sender.
    @discardableResult
    func submitComposition(_ submission: ComposerSubmission) -> Bool {
        guard let app else { return false }
        do {
            try app.submitComposition(
                composition: submission.composition,
                recipients: submission.recipients,
                subject: submission.subject,
                documentJson: submission.documentJson,
                files: submission.files,
                from: submission.from
            )
            return true
        } catch {
            print("[Mailcal] composer submit failed: \(type(of: error))")
            return false
        }
    }

    /// Save one decoded message attachment to the destination path chosen by the host. The
    /// core decodes the selected part and writes the whole file synchronously, so run it off
    /// the main actor (a detached task), a large attachment must not block the UI.
    func saveAttachment(_ account: String, _ key: String, _ attachment: AttachmentRow, to url: URL) async -> Bool {
        guard let app else { return false }
        return await Task.detached {
            do {
                try app.saveAttachment(
                    account: account,
                    key: key,
                    attachmentId: attachment.id,
                    destinationPath: url.path
                )
                return true
            } catch {
                print("[Mailcal] attachment save failed: \(type(of: error))")
                return false
            }
        }.value
    }

    /// Write the open message out as a `.eml` at the destination the host chose
    /// (docs/reading-actions.md). Off the main actor, like `saveAttachment`: the raw source may
    /// not be cached yet, and fetching it must not block the UI.
    func saveMessageSource(_ account: String, _ key: String, to url: URL) async -> Bool {
        guard let app else { return false }
        return await Task.detached {
            do {
                try app.saveMessageSource(account: account, key: key, destinationPath: url.path)
                return true
            } catch {
                print("[Mailcal] message export failed: \(type(of: error))")
                return false
            }
        }.value
    }
}
