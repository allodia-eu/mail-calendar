// The composer's editor takes dropped files itself, and takes only those.
//
// Worth a test because the failure is invisible: SwiftUI's `dropDestination` hit-tests its own view
// tree and the editor is an `NSViewRepresentable`, so that rectangle is a hole in it. With no
// dragging destination of its own the editor ignored every file dropped on the message while the
// From/To/Subject chrome above it accepted them, which reads as "drag and drop works" in any
// screenshot and in any test that does not ask this exact question.
//
// iPhone and iPad have the same hole and their own fix (`EditorWebViewTouch.swift`), which nothing
// here asserts on: `swift test` runs on macOS, so an `#if os(iOS)` suite would compile to nothing
// and report a pass over exactly the code in question. What is testable without a platform is the
// naming rule the touch hosts need, and that is below; the interaction itself is verified by hand
// on an iPad, dragging a picture out of Photos in Split View.

import Testing
import UniformTypeIdentifiers

@testable import MailcalUI

#if os(macOS)
import AppKit
import WebKit

@MainActor
@Suite struct ComposerDropTargetTests {

    /// Puts the view in a window, which is what `viewDidMoveToWindow` waits for.
    private func mountedEditor() -> EditorWebView {
        let frame = NSRect(x: 0, y: 0, width: 400, height: 300)
        let view = EditorWebView(frame: frame, configuration: WKWebViewConfiguration())
        let window = NSWindow(contentRect: frame, styleMask: [], backing: .buffered, defer: true)
        window.contentView?.addSubview(view)
        return view
    }

    @Test func theEditorAcceptsAFileDrag() {
        #expect(mountedEditor().registeredDraggedTypes.contains(.fileURL))
    }

    @Test func theEditorAcceptsNothingElse() {
        // WebKit registers its own types while building the view, and a drag it handles reaches the
        // page, where a `File` carries no path: it could neither be streamed from disk on send nor
        // listed for removal (docs/composer-security.md, Gate 13). The file URL is the only thing
        // this view may be offered.
        #expect(mountedEditor().registeredDraggedTypes == [.fileURL])
    }
}
#endif

// The naming rule for a staged drop, which the touch hosts depend on and macOS never exercises.
// Platform-free on purpose, so this runs.
@Suite struct DroppedFileNameTests {

    @Test func aNamelessItemStillGetsTheExtensionForItsType() {
        // A picture dragged out of Photos arrives with no usable name. Without the extension the
        // composer would offer to attach a screenshot rather than show it, because both
        // `isPicture` and the core's sniff start from the file's type.
        #expect(DroppedFileName.resolve(suggested: nil, type: .png).hasSuffix(".png"))
        #expect(DroppedFileName.resolve(suggested: "IMG_0001", type: .jpeg).hasSuffix(".jpeg"))
    }

    @Test func anExtensionTheItemAlreadyHasIsLeftAlone() {
        // Including one that disagrees with the declared type: the bytes decide what a picture is,
        // and a second guess here would only disagree with the core.
        #expect(DroppedFileName.resolve(suggested: "notes.txt", type: .plainText) == "notes.txt")
        #expect(DroppedFileName.resolve(suggested: "sneaky.png", type: .svg) == "sneaky.png")
    }
}
