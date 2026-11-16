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
                    - geometry.contentInsets.bottom,
                content: geometry.contentSize.height
            )
        } action: { _, window in
            visible = window
        }
        .onChange(of: editor.caret) { _, caret in
            unreachedCaret = caret
            followCaret(editorTop: editorTop)
        }
        // A layout pass: the editor has taken the height it was given, so a caret that was out of
        // reach may now be reached.
        .onChange(of: visible.content) { _, _ in followCaret(editorTop: editorTop) }
        // Where the column shows the page from, which the page keeps its toolbar at (rule 7).
        .onChange(of: visible.offset - editorTop, initial: true) { _, top in
            editor.setVisibleTop(top)
        }
    }

    /// Scrolls the column by what the core answers for the caret last reported, and leaves it
    /// where it is when the caret is already in view.
    ///
    /// The page reports its new height and its caret in the same frame, so a line typed past the
    /// end arrives before the column has grown to hold it. A target beyond the column's end is
    /// applied as far as it goes and kept, and tried again after the next layout pass. It is always
    /// reached in the end: the editor is at least as tall as its content, and the room under the
    /// caret (the editor's padding, its border and the column's bottom padding) is more than the
    /// core's margin.
    private func followCaret(editorTop: CGFloat) {
        guard let caret = unreachedCaret else { return }
        if let target = composerCaretScroll(
            offsetPx: visible.offset,
            visiblePx: visible.height,
            caretTopPx: editorTop + caret.top,
            caretBottomPx: editorTop + caret.bottom
        ) {
            scrollPosition.scrollTo(y: target)
            // Half a point either way is the editor's height rounded to whole pixels.
            if target > visible.content - visible.height + 0.5 { return }
        }
        unreachedCaret = nil
    }
}

/// The column's scroll offset, the height of its visible part (keyboard and bars excluded), and
/// the height of everything in it.
struct ScrollWindow: Equatable {
    let offset: CGFloat
    let height: CGFloat
    let content: CGFloat
}
