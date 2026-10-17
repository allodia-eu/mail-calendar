// Opening a composer: the one way every route into one asks the core, and the one place its answer
// becomes a `ComposeContext` (`docs/drafts.md`).
//
// On the model rather than on the shell because more than one host opens a composer: the pane's
// action row, the list's context menu, a Drafts row, a link, a share, an assistant, the Outbox, and
// a detached reading window (`docs/reading-window.md`). Each asks here, so none can open a composer
// that holds less than another.

import Foundation
import MailcalBindings

extension MailboxModel {
    /// Opens a composer on `origin` and answers with what it holds, or `nil` when it could not
    /// open, which the caller says (`composeOpenFailedMessage(for:)`) rather than showing an empty
    /// composer: for a draft, that composer's first save would replace the stored copy.
    ///
    /// The core is asked off the main actor: a reply may fetch the message it quotes, and a draft
    /// the copy on the server. The composer appears when the answer does, holding everything: one
    /// shown before its files arrived could be sent, or a draft saved, without them.
    func openComposer(
        _ origin: ComposeOrigin,
        prefill: ComposePrefill? = nil,
        addedAttachments: [ComposerFileAttachment] = [],
        notice: String? = nil,
        isShare: Bool = false
    ) async -> ComposeContext? {
        guard let app else { return nil }
        let composition = UUID().uuidString
        let directory = composerStagingDirectory().path
        let opening: ComposeOpening? = await Task.detached(priority: .userInitiated) {
            do {
                return try app.openComposer(
                    composition: composition,
                    origin: origin,
                    stagingDirectory: directory,
                    prefill: prefill
                )
            } catch {
                print("[Mailcal] composer open failed: \(type(of: error))")
                return nil
            }
        }.value
        guard let opening else { return nil }
        var context = composeContext(for: opening)
        context.addedAttachments = addedAttachments
        context.notice = notice
        context.isShare = isShare
        return context
    }

    /// The composer's context for what the core opened it with: what `openComposer` answers, and
    /// what an Outbox edit (`Surface::ComposeRequest`) opens with.
    ///
    /// The quote's attribution is worded here, in the user's language and with the original's date
    /// as the reading header shows it: for a fresh reply or forward it seeds the quote, and for a
    /// reopened one it completes the style the stored quote does not carry.
    func composeContext(for opening: ComposeOpening) -> ComposeContext {
        let settings = quoteSettingsNow()
        let isForward = ComposeOpeningSeed.mode(opening.kind) == .forward
        let words = opening.quote.map { quote in
            ComposerQuote.words(
                quote,
                date: localDateTime(quote.date, in: activeZone, use24Hour: use24Hour),
                isForward: isForward
            )
        }
        return ComposeContext(
            opening: opening,
            initialFrom: sendAccount(preferring: opening.account)?.id,
            quoteStyle: settings.style,
            quoteStylePerMessage: settings.perMessage,
            quote: ComposeOpeningSeed.quoteSeed(
                opening, words: words, style: ComposerQuote.token(settings.style)
            ),
            quoteAttribution: ComposeOpeningSeed.attributionCompletion(opening, words: words)
        )
    }

    /// A directory of this composer's own under the app's temporary storage, so two composers
    /// never share a staged file. A queued send moved back into Drafts for editing stages its
    /// files here too. The OS reclaims what is left behind, as it does for an attachment opened
    /// from the reading view.
    func composerStagingDirectory() -> URL {
        FileManager.default.temporaryDirectory
            .appendingPathComponent("composer-staging", isDirectory: true)
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
    }
}

/// What to say when a composer could not open: a draft's own line, because the draft is still
/// there to read, and the general one for every other origin.
func composeOpenFailedMessage(for origin: ComposeOrigin) -> String {
    if case .draft = origin { return L10n.compose_draft_open_failed() }
    return L10n.compose_open_failed()
}

/// A reply's origin, with the sample answer showcase mode puts above its quote, so the store
/// screenshot shows a written reply rather than an empty composer.
func replyOrigin(account: String, key: String, all: Bool) -> (ComposeOrigin, ComposePrefill?) {
    let origin = ComposeOrigin.reply(account: account, key: key, all: all)
    guard let text = ShowcaseMode.replyText(account: account, key: key) else { return (origin, nil) }
    return (origin, ComposePrefill(to: "", cc: "", bcc: "", subject: "", bodyText: text, signed: false, account: nil))
}
