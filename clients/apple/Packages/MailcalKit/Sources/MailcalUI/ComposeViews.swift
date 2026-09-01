// The compose-context plus the `SnapshotRow` list-identity helper: `ComposeContext` describes what
// the rich compose sheet is for (new / reply / forward). Split out of Mailcal.swift to keep each
// file under the 500-line limit. (The calendar editor lives in Calendar/EventEditorView.swift.)

import Foundation
import MailcalBindings
import SwiftUI

extension SnapshotRow {
    /// A stable identity for `List` diffing, so a row's view, and any in-flight swipe
    /// or destructive action, stays bound to its own message across a refresh, rather
    /// than to a positional index that a re-sync could reassign to a different message.
    var rowID: String {
        // The account is part of the identity: a provider key / thread id is unique only
        // WITHIN an account, so two accounts can collide on one in the unified view.
        switch self {
        case .flat(let message): return "m:\(message.account):\(message.key)"
        case .thread(let thread): return "t:\(thread.account):\(thread.threadId)"
        }
    }
}

extension EventRow {
    /// A stable identity for `List` diffing, the account scopes the key, which is unique
    /// only within an account, so two accounts' events never collide in the unified agenda.
    var rowID: String { "\(account):\(key)" }
}

/// What the rich compose sheet is for: a new message, a reply, a reply-all, or a forward.
/// Reply/reply-all/forward carry the original's owning `account` + `key`; reply and reply-all
/// also carry the `to`/`cc` recipients the core suggested (`replyRecipients`), computed once
/// when the sheet is opened rather than on every recomposition. The app derives the
/// `Re:`/`Fwd:` subject and (for a reply) threads from the stored original.
/// Reply/reply-all/forward also carry the quoted-original `quote` seed (a `Block::Quote`-shaped
/// JSON, or `nil` when the body isn't loaded for the message) and the `quoteStyle` to open the
/// composer's style toggle on, both computed once when the sheet opens.
/// `agentDraft` is a new message an AI assistant composed and asked the app to open, **unsent**
/// (docs/mcp.md). It carries its request's own id, so asking twice for the same message opens the
/// composer twice rather than the second request looking like the first and doing nothing.
enum ComposeContext: Identifiable {
    case new
    case reply(account: String, key: String, to: String, cc: String, subject: String, quote: String?, quoteStyle: QuoteStyleKind)
    case replyAll(account: String, key: String, to: String, cc: String, subject: String, quote: String?, quoteStyle: QuoteStyleKind)
    case forward(account: String, key: String, subject: String, quote: String?, quoteStyle: QuoteStyleKind)
    case agentDraft(AgentDraftRequest)

    var id: String {
        switch self {
        case .new: return "new"
        case .reply(_, let key, _, _, _, _, _): return "reply:\(key)"
        case .replyAll(_, let key, _, _, _, _, _): return "replyAll:\(key)"
        case .forward(_, let key, _, _, _): return "forward:\(key)"
        case .agentDraft(let request): return "agent:\(request.id)"
        }
    }
}
