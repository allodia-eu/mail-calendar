// Files dragged onto the composer, and the question a picture raises.
//
// A drop is handled NATIVELY, not by the page. The editor bundle refuses `drop`, because web code
// only ever sees a `File` with no path: it could neither hand the bytes to Rust for a streamed send
// nor put a removable row in the attachment list. The host resolves the drop to a file URL, so both
// work, and the page is handed a picture only when the user asks for one.
//
// A picture raises the question the other formats do not: it can be shown where the user is typing
// (an inline `cid:` part, what Outlook does) or sent as a file to download. Everything else is
// simply attached. The question is asked once for the whole drop.

import Foundation
import MailcalBindings
import SwiftUI
import UniformTypeIdentifiers

/// What to call a dropped item that had to be copied to disk before the host could use it, which
/// on iPhone and iPad is all of them (`EditorWebViewTouch.swift`: a drop there carries an
/// `NSItemProvider`, never a path).
///
/// The extension is the part that matters, not the stem: what a drop becomes is decided from the
/// file's type, by `ComposerDropModifier.isPicture` below and by the core's byte sniff behind it,
/// so an extensionless copy of a screenshot would be attached rather than offered as a picture.
///
/// Platform-free, and here rather than beside its one caller, so it is covered by a test: `swift
/// test` runs on macOS, where anything inside `#if os(iOS)` compiles to nothing and reports a pass
/// over the code that broke.
enum DroppedFileName {
    static func resolve(suggested: String?, type: UTType) -> String {
        let candidate = suggested ?? "attachment"
        guard (candidate as NSString).pathExtension.isEmpty,
              let ext = type.preferredFilenameExtension
        else {
            return candidate
        }
        return (candidate as NSString).appendingPathExtension(ext) ?? candidate
    }
}

/// Makes the composer accept dropped files, wherever it is mounted: the macOS detail column and
/// the iPad's full-screen cover apply the same modifier, so the two cannot come to behave
/// differently.
struct ComposerDropModifier: ViewModifier {
    @Binding var attachments: [PickedAttachment]
    /// Pictures waiting on the question below; non-empty only between a drop and its answer.
    @Binding var droppedPictures: [URL]
    /// The composer's shared error line, which this writes the picture-specific message into.
    @Binding var composerError: String?
    let editor: RichComposerEditor

    func body(content: Content) -> some View {
        content
            .dropDestination(for: URL.self) { urls, _ in accept(urls) }
            // The editor is a representable, so `dropDestination` above never sees a drop that lands
            // on it: that rectangle is a hole in the SwiftUI tree it hit-tests. The web view takes
            // those itself and hands them straight back here, so a file dropped on the message and
            // one dropped on the chrome go through the same code on every Apple host.
            .onAppear {
                (editor.webView as? EditorWebView)?.acceptDroppedFiles = { accept($0) }
            }
            .confirmationDialog(
                L10n.compose_image_drop_title(),
                isPresented: questionPresented,
                titleVisibility: .visible
            ) {
                Button(L10n.compose_image_drop_inline()) { showInMessage() }
                Button(L10n.compose_image_drop_attach()) {
                    attachments.append(contentsOf: droppedPictures.map { PickedAttachment(url: $0) })
                    droppedPictures = []
                }
                Button(L10n.action_cancel(), role: .cancel) { droppedPictures = [] }
            } message: {
                Text(L10n.compose_image_drop_body())
            }
    }

    /// Sorts one drop: everything that is not a picture attaches straight away, and the pictures
    /// go to the question below. Answers whether the drop carried anything at all.
    @discardableResult
    private func accept(_ urls: [URL]) -> Bool {
        let files = urls.filter(\.isFileURL)
        guard !files.isEmpty else {
            return false
        }
        attachments.append(
            contentsOf: files.filter { !Self.isPicture($0) }.map { PickedAttachment(url: $0) }
        )
        droppedPictures = files.filter(Self.isPicture)
        return true
    }

    /// Whether a dropped file is worth asking about. The system's guess from the file's type,
    /// which is enough to choose a question; the core sniffs the bytes before anything is shown
    /// (`composerImageDataUrl`), so a mislabelled file still cannot become an `<img>`.
    private static func isPicture(_ url: URL) -> Bool {
        if let type = try? url.resourceValues(forKeys: [.contentTypeKey]).contentType {
            return type.conforms(to: .image)
        }
        return UTType(filenameExtension: url.pathExtension)?.conforms(to: .image) ?? false
    }

    private var questionPresented: Binding<Bool> {
        Binding(get: { !droppedPictures.isEmpty }, set: { if !$0 { droppedPictures = [] } })
    }

    /// Reads each picture through the core and hands it to the shared editor.
    ///
    /// A picture the core cannot read as one is attached instead of being dropped on the floor:
    /// the user asked for it to be in the message, and losing it silently is the worse answer.
    private func showInMessage() {
        let pictures = droppedPictures
        droppedPictures = []
        var unreadable: [URL] = []
        for picture in pictures {
            do {
                let dataUrl = try composerImageDataUrl(path: picture.path)
                editor.insertImage(dataUrl: dataUrl, fileName: picture.lastPathComponent)
            } catch {
                unreadable.append(picture)
            }
        }
        if !unreadable.isEmpty {
            composerError = L10n.compose_image_failed()
            attachments.append(contentsOf: unreadable.map { PickedAttachment(url: $0) })
        }
    }
}
