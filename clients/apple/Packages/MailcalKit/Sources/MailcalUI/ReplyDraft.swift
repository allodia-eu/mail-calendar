// What a reply, a reply-all or a forward opens with: recipients, subject and the quoted original.
//
// On the model rather than on the shell because two hosts now ask for it: the pane's action row,
// and a detached reading window's (`docs/reading-window.md`). A window that built its own would be
// a second answer to "what does a reply look like", and the two would drift on the first change to
// either.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// The composer context a reply (or reply-all) to `account`/`key` opens with.
    ///
    /// The Subject is derived by the CORE, not here: the field is editable, so what the composer
    /// opens with is what gets sent unless the user changes it, and a client-side `"Re: " +
    /// subject` differs from the core's on a reply to a reply.
    func replyDraft(
        account: String,
        key: String,
        subject: String,
        all: Bool,
        quotingFrom opened: OpenedMessage?,
        body: ReadingSnapshot?
    ) -> ComposeContext {
        let prefill = replyRecipients(account, key, all)
        let seed = quoteSeed(account, key, quotingFrom: opened, body: body, isForward: false)
        let replySubject = MailcalBindings.replySubject(original: subject)
        return all
            ? .replyAll(
                account: account, key: key, to: prefill?.to ?? "", cc: prefill?.cc ?? "",
                subject: replySubject, quote: seed.quote, quoteStyle: seed.style)
            : .reply(
                account: account, key: key, to: prefill?.to ?? "", cc: prefill?.cc ?? "",
                subject: replySubject, quote: seed.quote, quoteStyle: seed.style)
    }

    /// The composer context a forward opens with, the original's files staged into it.
    ///
    /// Awaited before the composer opens, never after: a forward on screen holding nothing can be
    /// sent in the window before the files arrive, which is the forward without its attachments
    /// this ordering exists to prevent. Staging reads from the raw source the reading view has
    /// already cached, so in the ordinary case there is nothing to wait for.
    func forwardDraft(
        account: String,
        key: String,
        subject: String,
        quotingFrom opened: OpenedMessage?,
        body: ReadingSnapshot?
    ) async -> ComposeContext {
        let seed = quoteSeed(account, key, quotingFrom: opened, body: body, isForward: true)
        let staged = await stageForwardedAttachments(account, key, into: forwardStagingDirectory())
        return .forward(
            account: account,
            key: key,
            subject: MailcalBindings.forwardSubject(original: subject),
            quote: seed.quote,
            quoteStyle: seed.style,
            attachments: ForwardAttachments(files: staged ?? [], failed: staged == nil)
        )
    }

    /// The quoted-original seed for a reply/forward of `(account, key)`, plus the default style.
    ///
    /// There is a quote only when the reader that raised this action was showing *this* message
    /// and its body had arrived: a reply raised from a list row's context menu quotes nothing,
    /// because nothing has been read to quote. A showcase reply to the designated message also
    /// carries sample body text, so the store screenshot shows a written reply rather than an
    /// empty composer.
    private func quoteSeed(
        _ account: String,
        _ key: String,
        quotingFrom opened: OpenedMessage?,
        body: ReadingSnapshot?,
        isForward: Bool
    ) -> (quote: String?, style: QuoteStyleKind) {
        let style = quoteSettingsNow().style
        guard let opened, opened.account == account, opened.key == key else {
            return (nil, style)
        }
        let quote = ComposerQuote.seedJSON(
            style: style,
            message: opened,
            reading: body,
            isForward: isForward,
            initialText: isForward ? nil : ShowcaseMode.replyText(account: account, key: key)
        )
        return (quote, style)
    }

    /// A directory of this composer's own under the app's temporary storage, so two forwards
    /// never share a staged file. The OS reclaims what is left behind, as it does for an
    /// attachment opened from the reading view.
    private func forwardStagingDirectory() -> URL {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("forward-attachments", isDirectory: true)
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
    }
}
