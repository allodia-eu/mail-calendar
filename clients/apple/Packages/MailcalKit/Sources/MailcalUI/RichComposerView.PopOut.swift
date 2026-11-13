// Popping the composer out: the inline composer moved into a window of its own, holding exactly
// what it held (`docs/reading-window.md`, "Popping a composer out").
//
// A child of RichComposerView.swift, whose `RichComposeView` this extends.

import MailcalBindings
import SwiftUI

extension RichComposeView {
    /// The pop-out button, at the composer's top right. Only where a host offers somewhere to move
    /// the composer to: the main window's inline composer on the Mac.
    @ViewBuilder var popOutButton: some View {
        if popOut != nil {
            Button {
                popOutTapped()
            } label: {
                Label(L10n.action_open_in_window(), systemImage: "arrow.up.forward.app")
            }
            .labelStyle(.iconOnly)
            .imageScale(.large)
            .buttonStyle(.borderless)
            .help(L10n.action_open_in_window())
        }
    }

    /// Reads everything this composer holds and hands it to the host to open in a window.
    ///
    /// The composition is the window's from here on, so this composer lets go of it **before**
    /// the host closes it: leaving would otherwise save and close the composition the window
    /// has just opened on. A move that did not happen takes it back.
    func popOutTapped() {
        guard let popOut else { return }
        editor.documentJSON { result in
            guard case let .success(documentJson) = result else {
                composerError = L10n.compose_open_failed()
                return
            }
            Task { @MainActor in
                let state = ComposerState(
                    from: resolvedFrom,
                    to: to,
                    cc: cc,
                    bcc: bcc,
                    subject: subject,
                    documentJson: documentJson,
                    signature: movedSignature,
                    written: await isWritten()
                )
                draftReleased = true
                guard await popOut(state) else {
                    draftReleased = false
                    composerError = L10n.compose_open_failed()
                    return
                }
            }
        }
    }

    /// What this composer is doing with the signature, as a moved one carries it on.
    private var movedSignature: MovedSignature {
        guard signatures != nil else { return .untouched }
        switch signatureChoice {
        case nil: return .followsAccount
        case .noSignature: return .noSignature
        case let .signature(id): return .chosen(id: id)
        }
    }

    /// The choice a moved composer opens on: `nil` follows the account, as it does in a composer
    /// nobody has touched the picker in.
    static func signatureChoice(_ moved: MovedSignature) -> SignatureChoice? {
        switch moved {
        case .untouched, .followsAccount: return nil
        case .noSignature: return .noSignature
        case let .chosen(id): return .signature(id)
        }
    }
}
