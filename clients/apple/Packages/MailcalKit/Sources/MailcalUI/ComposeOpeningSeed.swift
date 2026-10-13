// How a composer opens from the core's `ComposeOpening`, as plain rules: which mode it is in, what
// the editor is seeded with, and the quote's attribution. Free of SwiftUI and of the catalog, so
// the unit tests reach it directly; the localised words are passed in (`ComposerQuote.words`).

import Foundation
import MailcalBindings

/// A quote's attribution in both styles, in the user's language: the one-line "On … wrote:" and
/// the labelled header block.
struct QuoteWords {
    let line: String
    let headers: [(label: String, value: String)]
}

enum ComposeOpeningSeed {
    /// The composer mode the core's answer names: its heading and its signature slot.
    static func mode(_ kind: ComposeKind) -> RichComposeMode {
        switch kind {
        case .new: return .new
        case .reply: return .reply
        case .replyAll: return .replyAll
        case .forward: return .forward
        }
    }

    /// What an assistant's draft opens on: a reply when it names the message it answers, so it
    /// stays in that conversation, and a new message from the account it names, else `selected`,
    /// otherwise. Its body brings its author's own sign-off (`docs/mcp.md`), so the prefill says
    /// so and the account's signature is not seeded under it.
    static func agentDraftOpening(
        _ draft: AgentDraft,
        selected: String?
    ) -> (origin: ComposeOrigin, prefill: ComposePrefill) {
        let origin: ComposeOrigin
        if let account = draft.replyToAccount, let key = draft.replyToKey {
            origin = .reply(account: account, key: key, all: false)
        } else {
            origin = .new(account: draft.account ?? selected)
        }
        let prefill = ComposePrefill(
            to: draft.to, cc: draft.cc, bcc: draft.bcc, subject: draft.subject,
            bodyText: draft.bodyText, signed: true, account: draft.account
        )
        return (origin, prefill)
    }

    /// The seed for `setComposerQuote`: a fresh reply's or forward's quoted original, with the
    /// opening's text above it (what a prefill brought), or `nil` when the composer does not open
    /// that way. A stored message opens through its own HTML instead, quote included.
    static func quoteSeed(_ opening: ComposeOpening, words: QuoteWords?, style: String) -> String? {
        guard opening.bodyHtml.isEmpty, let quote = opening.quote, let words else { return nil }
        guard !(quote.bodyHtml.isEmpty && quote.bodyPlain.isEmpty) else { return nil }
        var payload: [String: Any] = [
            "style": style,
            "attribution": attribution(words),
            "body_html": quote.bodyHtml,
            "body_plain": quote.bodyPlain,
        ]
        if !opening.bodyText.isEmpty {
            payload["initial_text"] = opening.bodyText
        }
        return json(payload)
    }

    /// The argument for `completeComposerQuoteAttribution`: a reopened message's quote is missing
    /// the attribution of the style it was not saved in, and the original says what it is. `nil`
    /// for anything else.
    static func attributionCompletion(_ opening: ComposeOpening, words: QuoteWords?) -> String? {
        guard !opening.bodyHtml.isEmpty, opening.quote != nil, let words else { return nil }
        return json(attribution(words))
    }

    /// The text the editor opens with: a stored message's own text beside its HTML, or, when it
    /// is seeded neither with a stored body nor with a quote, a link's, a share's or an
    /// assistant's. `nil` when the text rides inside the quote seed instead.
    static func plainBody(_ opening: ComposeOpening, quoteSeed: String?) -> String? {
        if !opening.bodyHtml.isEmpty { return opening.bodyText }
        guard quoteSeed == nil, !opening.bodyText.isEmpty else { return nil }
        return opening.bodyText
    }

    /// How many of the files a composer opens holding count as nothing written: the ones the core
    /// staged (`opening`), a forward's originals or a stored draft's own, which are still in the
    /// mailbox, so closing over them unchanged loses nothing. The files a host adds (`added`, a
    /// share's) the user would have to share again, so they count as written from the start.
    static func baselineAttachmentCount(
        opening: [ComposerFileAttachment],
        added: [ComposerFileAttachment]
    ) -> Int {
        opening.count
    }

    /// The style of the first quote a composer's document holds, or `nil` when it holds none:
    /// what a reopened composer's style picker opens on.
    static func quoteStyle(ofDocument document: String?) -> QuoteStyleKind? {
        guard let document,
              let root = try? JSONSerialization.jsonObject(with: Data(document.utf8)) as? [String: Any],
              let blocks = root["blocks"] as? [[String: Any]]
        else {
            return nil
        }
        for block in blocks {
            guard let quote = block["Quote"] as? [String: Any] else { continue }
            switch quote["style"] as? String {
            case "LineAndHeader": return .lineAndHeader
            case "Indented": return .indented
            default: return nil
            }
        }
        return nil
    }

    private static func attribution(_ words: QuoteWords) -> [String: Any] {
        [
            "line": words.line,
            "headers": words.headers.map { ["label": $0.label, "value": $0.value] },
        ]
    }

    private static func json(_ object: [String: Any]) -> String? {
        guard let data = try? JSONSerialization.data(withJSONObject: object) else { return nil }
        return String(data: data, encoding: .utf8)
    }
}
