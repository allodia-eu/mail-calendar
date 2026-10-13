// The compose-context plus the `SnapshotRow` list-identity helper: `ComposeContext` is what an open
// composer was opened with. Split out of Mailcal.swift to keep each file under the 500-line limit.
// (The calendar editor lives in Calendar/EventEditorView.swift.)

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

/// What the composer hands its parent when the user presses Send: everything
/// `submitComposition` names, in one value.
///
/// A value rather than positional arguments, because `from` and `composition` read alike at a call
/// site and mean opposite things: `from` is which account sends, `composition` is which composer
/// wrote it. Transposing them compiles.
struct ComposerSubmission {
    let recipients: Recipients
    /// The Subject field as edited. A reply and a forward open with the core's derived
    /// `Re:`/`Fwd:` already in it.
    let subject: String
    /// The rendered editor document.
    let documentJson: String
    /// The files the composer holds: picked, dropped, forwarded, shared or resumed.
    let files: [ComposerFileAttachment]
    /// The account picked in the From dropdown, or `nil` to let the core derive it.
    let from: String?
    /// The composition the composer opened on: the core joined it to the message a reply answers
    /// or a forward passes on, and an accepted send takes its stored draft out of Drafts
    /// (`docs/drafts.md`).
    let composition: String
}

/// One open composer: the core's answer about the message it opens on (`ComposeOpening`), and what
/// the host derived from it in the user's language.
///
/// One value for every way into a composer (a new message, a reply, a forward, a resumed draft, a
/// mail link, a share, an assistant's draft and an Outbox edit), so no route can open a composer
/// holding less than another (`docs/drafts.md`). Built by `MailboxModel.composeContext(for:)`.
///
/// Carries its own id: opening the same message twice is two composers, and iOS presents this
/// with `.fullScreenCover(item:)`, which does nothing for an item whose id equals the one already
/// presented. Equal by that id alone, for the same reason.
struct ComposeContext: Identifiable, Equatable {
    let id = UUID()
    /// What the core opened the composer with, on the composition it joined.
    let opening: ComposeOpening
    /// The account id the From picker opens on: the opening's account as a sending account.
    let initialFrom: String?
    /// The style a fresh quote is seeded in (the persisted app default).
    let quoteStyle: QuoteStyleKind
    /// Whether the user opted into choosing the style per message.
    let quoteStylePerMessage: Bool
    /// The `setComposerQuote` seed for a fresh reply or forward, or `nil`.
    let quote: String?
    /// The `completeComposerQuoteAttribution` argument for a reopened message whose original is
    /// known, or `nil`.
    let quoteAttribution: String?
    /// Files the host adds to what the core staged: a share's.
    var addedAttachments: [ComposerFileAttachment] = []
    /// A line the composer opens showing, for something the route could not bring: files a share
    /// could not hand over.
    var notice: String?
    /// Whether this composer holds a share another app handed over.
    var isShare = false

    /// Whether this is a new message, a reply, a reply-all or a forward.
    var mode: RichComposeMode { ComposeOpeningSeed.mode(opening.kind) }

    /// The text the editor opens with (`ComposeOpeningSeed.plainBody`).
    var plainBody: String? { ComposeOpeningSeed.plainBody(opening, quoteSeed: quote) }

    /// The error line the composer opens with: a forward whose originals could not be staged,
    /// else what the route brought.
    var initialError: String? {
        opening.attachmentsFailed ? L10n.compose_forward_attachments_failed() : notice
    }

    /// The composer's heading, the action it is performing.
    var title: String {
        switch mode {
        case .new: return L10n.compose_title_new()
        case .reply: return L10n.action_reply()
        case .replyAll: return L10n.action_reply_all()
        case .forward: return L10n.action_forward()
        }
    }

    /// What a window showing this draft is called: its subject, which is what the Window menu,
    /// ⌘-Tab and Mission Control read, and the only thing that tells two open drafts apart there
    /// (`docs/reading-window.md`). A draft that has no subject yet is named for what it is.
    var windowTitle: String {
        opening.subject.isEmpty ? L10n.compose_title_new() : opening.subject
    }

    static func == (lhs: Self, rhs: Self) -> Bool { lhs.id == rhs.id }
}

/// One `mailto:` link the OS handed us, already decoded by the shared core.
///
/// Carries its own id, so two arrivals of the same link are two requests rather than one value
/// seen twice; each opens a composer of its own (`ComposeContext.id`).
struct MailLinkRequest: Identifiable, Equatable {
    let id = UUID()
    let prefill: MailtoPrefill

    static func == (lhs: Self, rhs: Self) -> Bool { lhs.id == rhs.id }
}
