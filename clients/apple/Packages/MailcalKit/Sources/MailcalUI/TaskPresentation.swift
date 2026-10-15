// How a task that takes over the app (the composer, Settings) is presented on iPhone and iPad.

#if os(iOS)
import SwiftUI
import UIKit

extension View {
    /// A page sheet on iPad, as Apple's Human Interface Guidelines ask ("Sheets": prefer the page
    /// or form sheet presentation styles in an iPadOS app), and a full-screen cover on iPhone,
    /// where a sheet would fill the width anyway.
    ///
    /// Decided by the device, not by the size class: a size class changes while an iPad window is
    /// resized, and moving an open composer from one presentation to the other tears it down. In a
    /// narrow iPad window the page sheet adapts on its own.
    ///
    /// `swipeCloses: false` keeps the sheet's own close control the only way out, for the
    /// composer, whose Close keeps the draft (`docs/drafts.md`).
    @ViewBuilder
    func taskPresentation<Item: Identifiable, Content: View>(
        item: Binding<Item?>,
        onDismiss: (() -> Void)? = nil,
        swipeCloses: Bool,
        @ViewBuilder content: @escaping (Item) -> Content
    ) -> some View {
        if UIDevice.current.userInterfaceIdiom == .pad {
            sheet(item: item, onDismiss: onDismiss) { presented in
                content(presented)
                    .presentationSizing(.page)
                    .interactiveDismissDisabled(!swipeCloses)
            }
        } else {
            fullScreenCover(item: item, onDismiss: onDismiss, content: content)
        }
    }
}
#endif
