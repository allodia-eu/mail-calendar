// The composer's importance picker (docs/importance.md), in the action bar beside the signature's.
//
// The importance is the core's, held with the composition: the menu reads it there, a pick writes it
// there, and every save and the send take it from there, so the composer sends nothing of its own. A
// composer moved into a window of its own reads the same composition and so opens with it.

import MailcalBindings
import SwiftUI

/// The core verbs a composer's importance goes through. Closures over the model, so a preview or a
/// test can stand in for it. Each answers `nil` when there is no core to ask.
struct ComposerImportanceVerbs {
    let read: (String) -> ComposerImportance?
    let set: (String, MessageImportance) -> ComposerImportance?
}

extension MailboxModel {
    /// The importance picker `composition`'s composer draws. A lock and a lookup in the core, with
    /// nothing that waits, so it is answered on the caller's thread.
    func composerImportance(_ composition: String) -> ComposerImportance? {
        try? app?.composerImportance(composition: composition)
    }

    /// Sets what `composition`'s message goes out with, and answers with the picker as it stands.
    func setComposerImportance(_ composition: String, _ importance: MessageImportance) -> ComposerImportance? {
        try? app?.setComposerImportance(composition: composition, importance: importance)
    }
}

extension RichComposeView {
    /// The menu, with what the core offers inline and the current choice checked. Drawn only
    /// when there is a core to ask.
    @ViewBuilder var importanceMenu: some View {
        if let picker = importance {
            Menu {
                // Inline for the reason `signatureMenu` gives: one click, not a submenu.
                Picker(L10n.compose_importance_label(), selection: importanceBinding) {
                    ForEach(picker.options, id: \.self) { option in
                        Text(importanceLabel(option)).tag(option)
                    }
                }
                .pickerStyle(.inline)
            } label: {
                Label(L10n.compose_importance_label(), systemImage: composerImportanceSymbol(picker.selected))
                .composerBarItem()
            }
            .accessibilityValue(importanceLabel(picker.selected))
            #if os(macOS)
            // Styled as a button beside Attach and Signature, for the reason `signatureMenu` gives.
            .menuStyle(.button)
            .buttonStyle(.bordered)
            .controlSize(.small)
            .fixedSize()
            #endif
        }
    }

    /// The list row's glyph for a raised or lowered message, so the bar shows what is in force;
    /// at normal, which a row marks with nothing, a neutral one that still reads as the control
    /// where the bar draws icons only.
    private func composerImportanceSymbol(_ importance: MessageImportance) -> String {
        switch importance {
        case .high: "exclamationmark"
        case .low: "arrow.down"
        case .normal: "arrow.up.arrow.down"
        }
    }

    private var importanceBinding: Binding<MessageImportance> {
        Binding(
            get: { importance?.selected ?? .normal },
            set: { picked in
                if let answered = importanceVerbs?.set(composition, picked) {
                    importance = answered
                }
            }
        )
    }
}
