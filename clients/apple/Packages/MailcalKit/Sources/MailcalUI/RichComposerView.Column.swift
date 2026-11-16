// The desktop composer's one column (`docs/composer-layout.md`): a scroll view holding the header,
// the notes, the files and the editor in its growing mode. Split out of RichComposerView.swift to
// keep it under 500 lines; the touch composer's is `TouchComposerChrome`.

import MailcalBindings
import SwiftUI

extension RichComposeView {
    /// The composer as one column that scrolls as a whole (`docs/composer-layout.md`): `top` and the
    /// header, the notes, the files, then the message, which grows with its text. Nothing inside it
    /// scrolls on its own, so every file and every line is reached the same way.
    ///
    /// The editor is as tall as `composerEditorHeight` says: its content, or the room the visible
    /// part leaves under everything above it, whichever is more. It never takes `maxHeight:
    /// .infinity`, which inside a `ScrollView` is an unbounded web view.
    func composerColumn<Top: View>(@ViewBuilder top: () -> Top) -> some View {
        let top = top()
        let editorTop = columnPadding + aboveEditorHeight + columnSpacing
        let room = visible.height - editorTop - columnPadding
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
                    .frame(height: composerEditorHeight(contentPx: editor.contentHeight, roomPx: room))
                    .border(.quaternary)
            }
            .padding(columnPadding)
        }
        .scrollPosition($scrollPosition)
        .onScrollGeometryChange(for: ScrollWindow.self) { geometry in
            ScrollWindow(
                offset: geometry.contentOffset.y + geometry.contentInsets.top,
                height: geometry.containerSize.height - geometry.contentInsets.top
                    - geometry.contentInsets.bottom
            )
        } action: { _, window in
            visible = window
        }
        .onChange(of: editor.caret) { _, caret in
            guard let caret,
                  let target = composerCaretScroll(
                      offsetPx: visible.offset,
                      visiblePx: visible.height,
                      caretTopPx: editorTop + caret.top,
                      caretBottomPx: editorTop + caret.bottom
                  )
            else { return }
            scrollPosition.scrollTo(y: target)
        }
    }
}

/// The column's scroll offset and the height of its visible part, keyboard and bars excluded.
struct ScrollWindow: Equatable {
    let offset: CGFloat
    let height: CGFloat
}
