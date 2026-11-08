import MailcalBindings
import SwiftUI

extension ContentView {
    /// The composer, where the **shell** hosts it: in the detail column on macOS (in place of the
    /// reading pane) and through `taskPresentation` on iOS.
    ///
    /// The composer itself is `ComposeHost`, shared with the detached composer window on the
    /// desktop (`docs/reading-window.md`); all this adds is what closing means here, which is
    /// emptying `compose`, and on the Mac where popping it out moves it to.
    ///
    /// Not `private`: macOSLayout's `detailColumn` mounts it, and that lives in
    /// Mailcal.Layout.swift (Swift's `private` is file-scoped).
    func composeContent(_ context: ComposeContext) -> some View {
        #if os(macOS)
        ComposeHost(model: model, context: context, dismiss: { compose = nil }) { state in
            await popOut(context, state)
        }
        #else
        ComposeHost(model: model, context: context) { compose = nil }
        #endif
    }

    #if os(macOS)
    /// Moves the pane's composer into a window of its own, on the composition it already has, and
    /// gives the pane back to the message list (`docs/reading-window.md`, "Popping a composer
    /// out"). Answers whether it moved; one that did not stays where it was.
    private func popOut(_ context: ComposeContext, _ state: ComposerState) async -> Bool {
        guard let moved = await model.openComposer(
            .moved(state: state), composition: context.opening.composition
        ) else {
            return false
        }
        openWindow(id: DesktopWindow.composer, value: desktopDrafts.open(moved))
        if compose?.id == context.id { compose = nil }
        return true
    }
    #endif
}
