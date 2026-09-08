// The composer's editor web view on iPhone and iPad: taking a dropped file for the host.
//
// The same hole as macOS, in the UIKit spelling. SwiftUI's `dropDestination` hit-tests its own view
// tree, so the rectangle a representable occupies is not in it: a file dragged onto the message was
// ignored while the same file dropped on the From/To/Subject chrome was taken.
//
// One thing is genuinely different here, and it is the reason this is not the same six lines. A
// drop on macOS already carries a path. This one does not: Photos, Files and everything else hand
// over an `NSItemProvider`, whose file representation exists only for the length of a single
// callback. So each item is copied to a file of our own before the shared handler sees it, which is
// what `ComposerDropModifier` and the core both need (a path to stream from on send, and a row that
// can be removed). Android stages to its app cache for the same reason.

#if os(iOS)
import UIKit
import UniformTypeIdentifiers
import WebKit

/// A `WKWebView` that takes files dropped on the message itself.
final class EditorWebView: WKWebView {
    /// Files dropped on the editor, handed to the host. Set by `ComposerDropModifier`, which owns
    /// what a dropped file becomes; this type only gets the drop as far as it.
    var acceptDroppedFiles: (([URL]) -> Void)? {
        get { dropTarget.acceptDroppedFiles }
        set { dropTarget.acceptDroppedFiles = newValue }
    }

    private let dropTarget = EditorDropTarget()
    private var installed: UIDropInteraction?

    /// ⚠️ WebKit adds a drop interaction of its own to the content view it creates lazily, and
    /// UIKit offers a drop to the deepest view that will take it, so ours has to displace that one
    /// or it never runs. A drop WebKit handles reaches the page, where a `File` carries no path
    /// (docs/composer-security.md, Gate 13). Done here rather than in the initialiser because that
    /// content view does not exist yet at init, and again on every re-parenting.
    override func didMoveToWindow() {
        super.didMoveToWindow()
        removeDropInteractions(below: self)
        if installed == nil {
            let interaction = UIDropInteraction(delegate: dropTarget)
            addInteraction(interaction)
            installed = interaction
        }
    }

    private func removeDropInteractions(below view: UIView) {
        for subview in view.subviews {
            for interaction in subview.interactions where interaction is UIDropInteraction {
                subview.removeInteraction(interaction)
                subview.pasteConfiguration = nil
            }
            removeDropInteractions(below: subview)
        }
    }
}

/// Accepts the drop and turns each item into a file on disk.
@MainActor
final class EditorDropTarget: NSObject, UIDropInteractionDelegate {
    var acceptDroppedFiles: (([URL]) -> Void)?

    func dropInteraction(_ interaction: UIDropInteraction, canHandle session: any UIDropSession) -> Bool {
        !session.items.isEmpty
    }

    func dropInteraction(
        _ interaction: UIDropInteraction,
        sessionDidUpdate session: any UIDropSession
    ) -> UIDropProposal {
        UIDropProposal(operation: .copy)
    }

    func dropInteraction(_ interaction: UIDropInteraction, performDrop session: any UIDropSession) {
        let providers = session.items.map(\.itemProvider)
        Task { @MainActor in
            var staged: [URL] = []
            for provider in providers {
                if let url = await Self.stage(provider) {
                    staged.append(url)
                }
            }
            if !staged.isEmpty {
                acceptDroppedFiles?(staged)
            }
        }
    }

    /// Copies one dropped item into a file the host can keep.
    @MainActor
    private static func stage(_ provider: NSItemProvider) async -> URL? {
        guard let type = provider.registeredContentTypes.first else {
            return nil
        }
        let name = DroppedFileName.resolve(suggested: provider.suggestedName, type: type)
        return await withCheckedContinuation { continuation in
            _ = provider.loadFileRepresentation(for: type, openInPlace: false) { url, _, _ in
                continuation.resume(returning: copyIntoCache(url, named: name))
            }
        }
    }

    /// The copy itself. Separate and `nonisolated` because the load lands on a background queue,
    /// and because it is the part that must finish before that callback returns: the URL it is
    /// given is deleted the moment it does.
    private nonisolated static func copyIntoCache(_ source: URL?, named name: String) -> URL? {
        guard let source else {
            return nil
        }
        let folder = FileManager.default.temporaryDirectory
            .appendingPathComponent("composer-drop-\(UUID().uuidString)", isDirectory: true)
        let destination = folder.appendingPathComponent(name)
        do {
            try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
            try FileManager.default.copyItem(at: source, to: destination)
            return destination
        } catch {
            return nil
        }
    }
}
#endif
