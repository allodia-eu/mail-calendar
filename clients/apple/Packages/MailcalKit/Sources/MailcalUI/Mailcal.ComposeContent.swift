import MailcalBindings
import SwiftUI

extension ContentView {
    /// The composer, where the **shell** hosts it: in the detail column on macOS (in place of the
    /// reading pane) and as a full-screen cover on iOS.
    ///
    /// The composer itself is `ComposeHost`, shared with the detached composer window on the
    /// desktop (`docs/reading-window.md`); all this adds is the shell's own draft probe and what
    /// closing means here, which is emptying `compose`.
    ///
    /// Not `private`: macOSLayout's `detailColumn` mounts it, and that lives in
    /// Mailcal.Layout.swift (Swift's `private` is file-scoped).
    func composeContent(_ context: ComposeContext) -> some View {
        ComposeHost(model: model, context: context, probe: draftProbe) { compose = nil }
    }
}
