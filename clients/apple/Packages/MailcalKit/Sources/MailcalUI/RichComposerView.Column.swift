// The desktop composer's column (`docs/composer-layout.md`): a scroll view holding the header,
// the notes, the files and the editor in its framed mode. Split out of RichComposerView.swift to
// keep it under 500 lines; the touch composer's is `TouchComposerChrome`.

import MailcalBindings
import SwiftUI

/// The shortest the message's box is drawn, before the column has measured its visible part.
private let minimumEditorHeight: CGFloat = 240

extension RichComposeView {
    /// The composer as a column whose header scrolls away before the message does
    /// (`docs/composer-layout.md`, rules 2 and 7): `top` and the header, the notes, the files, then
    /// the message in a box exactly as tall as the column's visible part. Down, the column carries
    /// the header away until the box fills the visible part, and the message then scrolls inside
    /// it under a toolbar that stays where it is; up, the message goes back to its top before the
    /// header returns. `EditorWebView` hands each step to whichever the core says.
    func composerColumn<Top: View>(@ViewBuilder top: () -> Top) -> some View {
        let top = top()
        let editorTop = columnPadding + aboveEditorHeight + columnSpacing
        return ScrollView {
            VStack(alignment: .leading, spacing: columnSpacing) {
                VStack(alignment: .leading, spacing: columnSpacing) {
                    top
                    composerHeader
                    composerNotes
                    fileTiles
                }
                .onGeometryChange(for: CGFloat.self) { $0.size.height } action: {
                    aboveEditorHeight = $0
                }
                RichComposerWebView(editor: editor)
                    .frame(height: max(visible.height, minimumEditorHeight))
            }
            // Nothing under the message: its box ends where the column's visible part does.
            .padding([.top, .horizontal], columnPadding)
        }
        // The message's box draws the one scroll bar: the column carries only the header away, and
        // a bar for that beside the message's would read as two scrolls. `never` rather than
        // `hidden`, which macOS overrides when the user has scroll bars always shown.
        .scrollIndicators(.never)
        .scrollPosition($scrollPosition)
        .onScrollGeometryChange(for: ScrollWindow.self) { geometry in
            ScrollWindow(
                offset: geometry.contentOffset.y + geometry.contentInsets.top,
                height: geometry.containerSize.height - geometry.contentInsets.top
                    - geometry.contentInsets.bottom,
                content: geometry.contentSize.height
            )
        } action: { _, window in
            visible = window
            editor.columnOffset = window.offset
            editor.columnEnd = max(0, window.content - window.height)
        }
        .onChange(of: editor.caret) { _, caret in
            // A target past the column's end stops there: the message's box then fills the visible
            // part, and the page keeps its caret in view inside it.
            guard let caret,
                  let target = composerCaretScroll(
                      offsetPx: visible.offset,
                      visiblePx: visible.height,
                      caretTopPx: editorTop + caret.top,
                      caretBottomPx: editorTop + caret.bottom
                  )
            else { return }
            let end = max(0, visible.content - visible.height)
            if abs(min(target, end) - visible.offset) >= 0.5 {
                scrollPosition.scrollTo(y: min(target, end))
            }
        }
    }
}

/// The column's scroll offset, the height of its visible part (keyboard and bars excluded), and
/// the height of everything in it.
struct ScrollWindow: Equatable {
    let offset: CGFloat
    let height: CGFloat
    let content: CGFloat
}
