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
/// What a forward composer opens holding: the files the original carries, already staged by the
/// core, and whether staging failed. The two travel together because an empty list means opposite
/// things either way: nothing was attached, or everything was and none of it could be read.
struct ForwardAttachments {
    var files: [ComposerFileAttachment] = []
    var failed = false
}

/// What the composer hands its parent when the user presses Send: everything the three
/// `submitRich*` calls name, in one value.
///
/// A value rather than six positional arguments, because the last two read alike at a call site
/// and mean opposite things: `from` is which account sends, `composition` is which composer wrote
/// it. Transposing them compiles.
struct ComposerSubmission {
    let recipients: Recipients
    /// The Subject field as edited. A reply and a forward open with the core's derived
    /// `Re:`/`Fwd:` already in it.
    let subject: String
    /// The rendered editor document.
    let documentJson: String
    /// The files the composer holds: picked, dropped, forwarded or resumed.
    let files: [ComposerFileAttachment]
    /// The account picked in the From dropdown, or `nil` to let the core derive it.
    let from: String?
    /// The composition this message was written in, so an accepted send takes its stored draft
    /// out of Drafts (`docs/drafts.md`). `nil` only from a composer that keeps no draft.
    let composition: String?
}

/// A draft the core has opened back up, and the composition it was adopted into.
///
/// The composition travels with it because the core has already joined that id to the copy on the
/// server: a composer that minted one of its own would save a second draft beside the one it is
/// showing, and neither would supersede the other (`docs/drafts.md`).
///
/// Carries its own id for the reason `MailLinkRequest` does: opening the same draft twice must
/// open the composer twice rather than compare equal to the first and appear to do nothing.
struct ResumedDraftRequest: Identifiable, Equatable {
    let id = UUID()
    let composition: String
    let draft: DraftResume

    static func == (lhs: Self, rhs: Self) -> Bool { lhs.id == rhs.id }
}

enum ComposeContext: Identifiable {
    case new
    case reply(account: String, key: String, to: String, cc: String, subject: String, quote: String?, quoteStyle: QuoteStyleKind)
    case replyAll(account: String, key: String, to: String, cc: String, subject: String, quote: String?, quoteStyle: QuoteStyleKind)
    case forward(account: String, key: String, subject: String, quote: String?, quoteStyle: QuoteStyleKind, attachments: ForwardAttachments)
    case agentDraft(AgentDraftRequest)
    case mailLink(MailLinkRequest)
    case share(ShareOpenRequest)
    case resumedDraft(ResumedDraftRequest)

    var id: String {
        switch self {
        case .new: return "new"
        case .reply(_, let key, _, _, _, _, _): return "reply:\(key)"
        case .replyAll(_, let key, _, _, _, _, _): return "replyAll:\(key)"
        case .forward(_, let key, _, _, _, _): return "forward:\(key)"
        case .agentDraft(let request): return "agent:\(request.id)"
        case .mailLink(let request): return "mailLink:\(request.id)"
        case .share(let request): return "share:\(request.id)"
        case .resumedDraft(let request): return "draft:\(request.id)"
        }
    }

    /// What a window showing this draft is called: its subject, which is what the Window menu,
    /// ⌘-Tab and Mission Control read, and the only thing that tells two open drafts apart there
    /// (`docs/reading-window.md`). A draft that has no subject yet is named for what it is.
    var windowTitle: String {
        switch self {
        case let .reply(_, _, _, _, subject, _, _),
             let .replyAll(_, _, _, _, subject, _, _),
             let .forward(_, _, subject, _, _, _):
            return subject.isEmpty ? L10n.compose_title_new() : subject
        case let .resumedDraft(request):
            return request.draft.subject.isEmpty ? L10n.compose_title_new() : request.draft.subject
        case .new, .agentDraft, .mailLink, .share:
            return L10n.compose_title_new()
        }
    }
}

/// One `mailto:` link the OS handed us, already decoded by the shared core.
///
/// Carries its own id for the same reason `AgentDraftRequest` does: tapping the same link twice
/// must open the composer twice, and without an id the second request would compare equal to the
/// first and appear to do nothing.
struct MailLinkRequest: Identifiable, Equatable {
    let id = UUID()
    let prefill: MailtoPrefill

    static func == (lhs: Self, rhs: Self) -> Bool { lhs.id == rhs.id }
}
