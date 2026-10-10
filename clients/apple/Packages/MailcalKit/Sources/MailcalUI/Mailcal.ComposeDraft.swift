// Handing the macOS detail column from the composer to something else.
//
// On macOS the composer replaces the reading pane, so the sidebar and the message list stay live
// while you write, and a click on another message takes the column away from it. That is leaving
// the composer, and the composer keeps its own draft on the way out: it saves what was written and
// closes, without asking (`docs/drafts.md`). All the shell does is clear the column.
//
// iPhone and iPad keep the full-screen composer, where no row is clickable behind it, so the
// guard short-circuits there.

import MailcalBindings
import SwiftUI

/// The header state the composer watches, one `Equatable` value, so a single `onChange` covers
/// every field instead of one per field. Also the snapshot of what the composer opened with, which
/// is what "was anything written in it?" compares against.
struct DraftHeaders: Equatable {
    let to: String
    let cc: String
    let bcc: String
    let subject: String
    let attachments: Int
}

extension View {
    /// Reports every header edit, for the autosave timer, which restarts on each one
    /// (`docs/drafts.md`).
    ///
    /// The recipient pre-fill of a reply arrives as the field's *initial* value, so it raises no
    /// change. The body is not watched here: the editor is a web view, and the composer samples
    /// it instead.
    func composeDraftTracking(_ headers: DraftHeaders, edited: @escaping () -> Void) -> some View {
        onChange(of: headers) { _, _ in edited() }
    }
}

extension ContentView {
    /// Performs `open`, taking the detail column away from a composer that holds it first.
    ///
    /// The composer saves its draft as it goes and closes, so there is nothing to ask: a draft
    /// with something written in it is in Drafts, and one with nothing written in it had nothing
    /// to keep.
    func openGuardingDraft(_ open: @escaping () -> Void) {
        #if os(macOS)
        compose = nil
        #endif
        open()
    }

    /// Opens a `mailto:` link in the composer, pre-filled (docs/os-integration.md).
    ///
    /// Through the same handover a message click uses: a link arrives unprompted, from a web page
    /// or another app, and the composer it replaces keeps its draft on the way out. Linux and
    /// Windows hand over identically.
    ///
    /// A link arriving before there is an account to send from is put back on the model, and the
    /// shell opens it once accounts exist: the alternative is a composer with nothing in its From
    /// dropdown, which cannot send and cannot explain why.
    func openMailLink(_ request: MailLinkRequest) {
        guard !model.accounts.isEmpty else {
            model.pendingMailLink = request
            return
        }
        openGuardingDraft { compose = .mailLink(request) }
    }

    /// Opens an assistant's draft in the composer, unsent (docs/mcp.md).
    ///
    /// Through the same handover a message click uses. An assistant's draft arrives from another
    /// process, unprompted, at any moment, so the composer it replaces must keep what the user was
    /// writing, which it does by saving it to Drafts on the way out.
    func openDraft(_ request: AgentDraftRequest) {
        openGuardingDraft { compose = .agentDraft(request) }
    }
}

/// The two ways a `mailto:` link reaches the composer: the OS handing the shell one, and one that
/// arrived before there was an account to send from opening as soon as there is.
///
/// The URI is opaque and comes from somewhere we do not control, so the shared core decodes it:
/// only To/Cc/Bcc/Subject/Body are honoured and every other header a link may name is dropped
/// (docs/composer-security.md, Gate 12). Anything that is not a mail link is ignored rather than
/// opening a blank composer over whatever the user was doing.
///
/// `onOpenURL` is the app's only URL hook, used here and in `ShareRouting` for the Share
/// Extension's doorbell, each ignoring the other's scheme. The OAuth redirects never reach it: each
/// is captured inside its own `ASWebAuthenticationSession`, which is why there is no scheme dispatch
/// here of the kind Windows, Linux and Android each need to keep a sign-in from being mistaken for
/// a link.
struct MailLinkRouting: ViewModifier {
    let model: MailboxModel
    let open: (MailLinkRequest) -> Void

    func body(content: Content) -> some View {
        content
            .onOpenURL { url in
                guard let prefill = parseMailtoUri(uri: url.absoluteString) else { return }
                open(MailLinkRequest(prefill: prefill))
            }
            .onChange(of: model.accounts.count) { _, count in
                guard count > 0, let request = model.pendingMailLink else { return }
                model.pendingMailLink = nil
                open(request)
            }
    }
}
