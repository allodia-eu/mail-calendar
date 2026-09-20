// Opening the composer for a share (docs/os-integration.md).
//
// The twin of the mail-link path in Mailcal.ComposeDraft.swift, and it behaves identically where
// the two meet: a share arriving before the first account is kept until one exists, and a share
// arriving over a draft asks before replacing it, because a launch the user did not aim at this
// window must never throw away what they were writing. Windows and Linux each do the same.
//
// What differs is the payload, and how it gets here. The composer opens already holding
// attachments, which only a forward otherwise does; and the share was read by a separate process,
// the Share Extension, which left it in the App Group container (MailcalShareBox). So the trigger
// is not a URL but an **activation**: whatever brought the app forward, it looks in its box.

import MailcalBindings
import MailcalShareBox
import SwiftUI

#if os(macOS)
import AppKit
#else
import UIKit
#endif

/// One share the extension left for us, already decoded by the shared core.
///
/// Carries its own id for the same reason `MailLinkRequest` does: sharing twice must open the
/// composer twice, and without an id the second request would compare equal to the first and
/// appear to do nothing.
struct ShareOpenRequest: Identifiable, Equatable {
    let id = UUID()
    let prefill: SharePrefill

    static func == (lhs: Self, rhs: Self) -> Bool { lhs.id == rhs.id }
}

/// The drop box, read.
enum ShareInbox {

    /// Takes the share that has waited longest and asks the core what it means, or answers `nil`
    /// when there is nothing worth opening a composer for.
    ///
    /// Everything from the manifest on is the core's: the names, the media types, the cap, and
    /// which items it will not take. Nothing here inspects a file.
    @MainActor static func take() -> ShareOpenRequest? {
        guard let box = ShareBox.shared(appID: Brand.appID), let drop = box.take() else {
            return nil
        }
        let prefill = prefillFromShare(
            request: ShareRequest(
                files: drop.files.map {
                    SharedFile(
                        path: $0.path,
                        suggestedName: $0.suggestedName,
                        declaredMediaType: $0.declaredMediaType
                    )
                },
                text: drop.text,
                subject: drop.subject
            ))
        // Counts only: the names are the user's own files (docs/logging.md).
        logAppleLifecycle(
            "share received: \(prefill.attachments.count) file(s), \(prefill.rejected.count) refused"
        )
        guard opensComposer(prefill) else { return nil }
        return ShareOpenRequest(prefill: prefill)
    }
}

/// The two moments a share reaches the composer: the app being activated, which is what the
/// extension's doorbell causes and what a manual launch also does, and an account finally existing
/// for a share that arrived before there was one.
///
/// Activation rather than the doorbell URL itself, deliberately. The URL carries nothing, so
/// nothing is lost by not reading it; and the extension's `open` is best effort, so a share whose
/// doorbell went unanswered is still picked up the next time the user brings the app forward.
struct ShareRouting: ViewModifier {
    let model: MailboxModel
    let open: (ShareOpenRequest) -> Void

    #if os(macOS)
    private static let activated = NSApplication.didBecomeActiveNotification
    #else
    private static let activated = UIApplication.didBecomeActiveNotification
    #endif

    func body(content: Content) -> some View {
        content
            // The cold start: activation has already happened by the time this view exists.
            .task { drain() }
            .onReceive(NotificationCenter.default.publisher(for: Self.activated)) { _ in drain() }
            .onChange(of: model.accounts.count) { _, count in
                guard count > 0, let request = model.pendingShare else { return }
                model.pendingShare = nil
                open(request)
            }
    }

    private func drain() {
        // Not while one is already waiting for its first account. Taking a share spends it, so a
        // second activation would overwrite a set of files the user watched leave a share sheet
        // and has not seen since. It keeps its place in the box instead.
        guard model.pendingShare == nil, let request = ShareInbox.take() else { return }
        open(request)
    }
}

extension ContentView {
    /// Opens a share in the composer, holding what was shared and fully editable.
    ///
    /// Behind the same discard guard a message click and a mail link use, for the same reason: a
    /// share arrives from another app, unprompted, and must not be able to throw away a
    /// half-written message.
    ///
    /// A share arriving before there is an account to send from is put back on the model, and the
    /// shell opens it once accounts exist: the alternative is a composer with nothing in its From
    /// dropdown, which cannot send and cannot explain why.
    func openShare(_ request: ShareOpenRequest) {
        guard !model.accounts.isEmpty else {
            model.pendingShare = request
            return
        }
        openGuardingDraft { compose = .share(request) }
    }
}

/// Whether a decoded share is worth opening a composer for.
///
/// A share that carried nothing usable is dropped: the user asked to send *those files*, so a
/// blank composer over whatever they were doing is a worse answer than none, and Windows and
/// Linux drop an empty share too. A share whose files were **all refused** is a different
/// question, and it opens: `SharePrefill::is_empty` answers "nothing to seed", not "nothing to
/// say", the app has already been brought forward by the extension's doorbell, and a file the
/// user watched go into a share sheet must be named rather than left to be noticed
/// (docs/os-integration.md).
func opensComposer(_ prefill: SharePrefill) -> Bool {
    !prefill.isEmpty || !prefill.rejected.isEmpty
}

/// What the composer says it could not take.
///
/// A file the user watched go into a share sheet and never saw again is one they will assume was
/// attached, so every refusal is named (docs/os-integration.md). The names are the core's,
/// normalised exactly as an accepted file's would be, so they are safe to put on screen and read
/// as the files the user chose.
func shareRefusalNotice(_ rejected: [RejectedShare]) -> String? {
    guard !rejected.isEmpty else { return nil }
    return L10n.compose_share_attachments_failed(
        files: ListFormatter.localizedString(byJoining: rejected.map(\.name)))
}
