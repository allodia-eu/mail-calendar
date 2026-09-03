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
            .dropDestination(for: URL.self) { urls, _ in
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
