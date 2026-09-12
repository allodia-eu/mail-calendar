import Foundation
import MailcalBindings

extension MailboxModel {
    /// Send a rich composer document to the entered `recipients`. Validation happens
    /// synchronously in Rust; attachment bytes are read by Rust from the host-selected file
    /// paths so they do not cross Swift FFI memory.
    ///
    /// `from` is the account the user picked in the composer's From dropdown; it decides both
    /// the `From:` identity and the outbox the draft goes out through. `nil` lets the core
    /// derive it. An id naming an account that is no longer configured fails the send rather
    /// than substituting another sender.
    @discardableResult
    func submitRich(
        _ recipients: Recipients,
        _ subject: String,
        _ documentJson: String,
        _ files: [ComposerFileAttachment],
        from: String?
    ) -> Bool {
        guard let app else { return false }
        do {
            try app.submitRichMailWithFiles(
                recipients: recipients,
                subject: subject,
                documentJson: documentJson,
                files: files,
                from: from
            )
            return true
        } catch {
            print("[Mailcal] rich composer submit failed: \(type(of: error))")
            return false
        }
    }

    /// Rich reply (or reply-all) to a message (by key, on its owning `account`): renders the
    /// shared composer document and submits to the user-confirmed `recipients`, letting the
    /// app derive the `Re:` subject and threading from the original.
    ///
    /// `from` is the sending account (the composer's From dropdown), which may differ from
    /// `account`: the core still resolves the original, and its `Re:` subject and
    /// `In-Reply-To`/`References` chain, in the account that holds it, so a cross-account
    /// reply still threads. `nil` replies from `account`.
    @discardableResult
    func submitRichReply(
        _ account: String,
        _ key: String,
        _ recipients: Recipients,
        _ subject: String,
        _ documentJson: String,
        _ files: [ComposerFileAttachment],
        from: String?
    ) -> Bool {
        guard let app else { return false }
        do {
            try app.submitRichReplyWithFiles(
                account: account,
                key: key,
                recipients: recipients,
                documentJson: documentJson,
                files: files,
                from: from,
                subject: subject
            )
            return true
        } catch {
            print("[Mailcal] rich reply submit failed: \(type(of: error))")
            return false
        }
    }

    /// Rich forward of a message (by key, on its owning `account`) to the entered
    /// `recipients`: renders the shared composer document and submits with a `Fwd:` subject.
    /// `from` is the sending account (the composer's From dropdown); `nil` forwards from
    /// `account`.
    @discardableResult
    func submitRichForward(
        _ account: String,
        _ key: String,
        _ recipients: Recipients,
        _ subject: String,
        _ documentJson: String,
        _ files: [ComposerFileAttachment],
        from: String?
    ) -> Bool {
        guard let app else { return false }
        do {
            try app.submitRichForwardWithFiles(
                account: account,
                key: key,
                recipients: recipients,
                documentJson: documentJson,
                files: files,
                from: from,
                subject: subject
            )
            return true
        } catch {
            print("[Mailcal] rich forward submit failed: \(type(of: error))")
            return false
        }
    }

    /// The recipients to pre-fill a reply (`replyAll == false`) or reply-all composer for the
    /// message `key` (on its owning `account`): the core returns the suggested To and Cc.
    func replyRecipients(_ account: String, _ key: String, _ replyAll: Bool) -> RecipientSuggestion? {
        app?.replyRecipients(account: account, key: key, replyAll: replyAll)
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

    /// The files the message `key` carries, staged for its forward composer to open holding
    /// them. `nil` means they could not be read, which the composer says: an empty list would
    /// read as a message with nothing attached.
    ///
    /// Off the main actor for the same reason as `saveAttachment`: the core decodes and writes
    /// every file, and a large one must not block the UI.
    func stageForwardedAttachments(
        _ account: String,
        _ key: String,
        into directory: URL
    ) async -> [ComposerFileAttachment]? {
        guard let app else { return nil }
        return await Task.detached {
            do {
                return try app.stageForwardedAttachments(
                    account: account,
                    key: key,
                    stagingDirectory: directory.path
                )
            } catch {
                print("[Mailcal] forward attachment staging failed: \(type(of: error))")
                return nil
            }
        }.value
    }
}
