// The composer's action bar: Send and Discard on the Mac, Attach, Signature and the draft's
// actions everywhere. Split from RichComposerView.swift, which stays under the line limit.

import MailcalBindings
import SwiftUI

extension RichComposeView {
    /// The composer's action bar, above the editor, Outlook's arrangement, and the reason the
    /// signature is *here* rather than as a labelled row among From/To/Cc: it is an action you take
    /// on the message, not a field you address it with. Attaching a file is the same kind of thing,
    /// so it moved out of its own row below the editor and joined it.
    ///
    /// Send and Discard lead the bar on macOS, where the composer is an inline pane with no window
    /// chrome of its own. On iOS Send stays in the navigation bar beside Close, the platform puts
    /// confirm/cancel there, and Discard joins Save as draft here, because Close is leaving and
    /// keeps the draft (`docs/drafts.md`).
    @ViewBuilder var actionBar: some View {
        #if os(macOS)
        HStack(spacing: 10) {
            Button {
                prepareAndSend()
            } label: {
                Label(L10n.action_send(), systemImage: "paperplane")
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.small)
            .disabled(sendDisabled)
            discardButton
            Divider().frame(height: 16)
            Button {
                chooseAttachments()
            } label: {
                Label(L10n.action_attach(), systemImage: "paperclip")
            }
            .buttonStyle(.bordered)
            .controlSize(.small)
            if drafts != nil {
                // Never disabled. A draft is unfinished by definition, so there is no state a
                // composer can be in that this refuses, and pressing it on an unchanged message
                // reaches no server.
                Button {
                    saveDraftNow()
                } label: {
                    Label(L10n.action_save_draft(), systemImage: "tray.and.arrow.down")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
            }
            if showsSignaturePicker {
                signatureMenu
            }
            Spacer()
        }
        #else
        // A phone has room for neither four labelled buttons nor one row of them, so the bar takes
        // the guidelines' shape for a narrow toolbar: symbols without borders for what is used
        // while writing, and the rest in a More menu at the trailing end.
        HStack(spacing: 4) {
            Button { chooseAttachments() } label: {
                Label(L10n.action_attach(), systemImage: "paperclip").composerBarItem()
            }
            if showsSignaturePicker {
                signatureMenu
            }
            Spacer()
            if drafts != nil {
                Menu {
                    // Never disabled: a draft is unfinished by definition, and saving an unchanged
                    // one reaches no server.
                    Button(L10n.action_save_draft(), systemImage: "tray.and.arrow.down") {
                        saveDraftNow()
                    }
                    Button(L10n.action_discard(), systemImage: "trash", role: .destructive) {
                        discardTapped()
                    }
                } label: {
                    Label(L10n.a11y_more_actions(), systemImage: "ellipsis").composerBarItem()
                }
            }
        }
        .labelStyle(.iconOnly)
        .buttonStyle(.borderless)
        #endif
    }
}
