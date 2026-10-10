// The signature body editor: the shared `clients/composer/dist/editor.html` bundle hosted body-only,
// in the composer's own `EditorHost` (docs/composer-security.md): local assets, JS for this
// document only, every remote load and navigation blocked. Authoring a signature is authoring mail
// content, so it gets the composer's gates, not a lighter set.
//
// The one thing it does that the composer does not is insert an image as a self-contained `data:`
// URI. That is what a signature stores (one file, no side-car blobs) and what the core rewrites
// to a `cid:` part on send.

import Foundation
import MailcalBindings
import SwiftUI
import UniformTypeIdentifiers
import WebKit

/// The cap on an embedded signature image, in bytes. A signature rides in **every** message the
/// account sends, so a 5 MB logo is 5 MB per mail, and base64 adds a third on top. 512 KB is
/// generous for a logo and small enough that nobody notices it on the wire.
private let signatureImageLimit = 512 * 1024

@MainActor
@Observable
final class SignatureEditor {
    /// The web view and its gates, the composer's own (`EditorHost`). Nothing is built until the
    /// editor is on screen.
    @ObservationIgnored let host = EditorHost(
        fallbackHTML: "<!doctype html><html><body><script>window.signatureBody=function(){return JSON.stringify({body_html:\"\",body_plain:\"\"});};</script></body></html>"
    )
    /// The body to load once the bundle has finished loading, set for an existing signature,
    /// `nil` for a new one. Applied once the bundle has loaded, because it loads asynchronously.
    var pendingBody: String?
    /// The link dialog the editor is waiting on, if any.
    var linkRequest: LinkDialogRequest?

    /// The editor's request channel.
    var hostChannel: ComposerHostChannel { host.channel }

    init() {
        host.channel.onLink = { [weak self] in self?.linkRequest = $0 }
        host.onLoad = { [weak self] in self?.seedOnLoad() }
    }

    /// The web view, for the representable: built on the first call, the same one after.
    func mount() -> WKWebView {
        host.mount()
    }

    // Always call `setSignatureBody`, even for a brand-new signature with no body: it also carries
    // the placeholder, and the bundle's default ("Write your message") is the composer's wording,
    // which is wrong here.
    private func seedOnLoad() {
        // The toolbar's strings first, then the body, `setSignatureBody` carries this surface's own
        // placeholder and must win over the composer wording `setComposerLabels` sends.
        host.evaluate(ComposerLabels.script())
        host.channel.announce()
        let body = EditorHost.jsString(pendingBody ?? "")
        let placeholder = EditorHost.jsString(L10n.settings_signatures_placeholder())
        host.evaluate("window.setSignatureBody(\(body), \(placeholder))")
        // Writing the signature is the only thing this screen is for, so the caret opens in it.
        // Asked for rather than assumed: the shared bundle focuses nothing of its own accord,
        // because in the composer the caret belongs in To (docs/contacts.md §4).
        host.evaluate("window.focusComposerBody()")
    }

    /// Reads back the authored signature, the HTML to store and its plain-text rendering.
    /// `nil` if the editor could not be read (the bundle is still loading).
    func body(_ completion: @escaping ((html: String, plain: String)?) -> Void) {
        host.evaluate("window.signatureBody()") { value, _ in
            guard let json = value as? String,
                  let data = json.data(using: .utf8),
                  let parsed = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
                  let html = parsed["body_html"] as? String
            else {
                completion(nil)
                return
            }
            completion((html: html, plain: parsed["body_plain"] as? String ?? ""))
        }
    }

    /// Inserts `url`'s image at the caret as a `data:` URI.
    func insertImage(dataURL: String, altText: String) {
        let payload: [String: Any] = ["data_url": dataURL, "alt_text": altText]
        guard let data = try? JSONSerialization.data(withJSONObject: payload),
              let json = String(data: data, encoding: .utf8)
        else { return }
        host.evaluate("window.insertSignatureImage(\(EditorHost.jsString(json)))")
    }
}

private struct SignatureEditorWebView: PlatformViewRepresentable {
    let editor: SignatureEditor

    #if os(macOS)
    func makeNSView(context: Context) -> WKWebView { editor.mount() }
    func updateNSView(_ nsView: WKWebView, context: Context) {}
    #else
    func makeUIView(context: Context) -> WKWebView { editor.mount() }
    func updateUIView(_ uiView: WKWebView, context: Context) {}
    #endif
}

/// Reads an image file and turns it into a `data:` URI, or explains why it can't. Returns `nil`
/// for an unreadable file; the size check is separate so the user is told *which* problem it is.
enum SignatureImage {
    /// The outcome of picking an image for a signature.
    enum Outcome {
        /// A `data:image/…;base64,…` URI, ready to insert.
        case dataURL(String)
        /// The file is larger than the per-image cap; carries the cap for the message.
        case tooLarge(limit: String)
        /// The file could not be read, or is not an image type we can name.
        case failed
    }

    static func load(_ url: URL) -> Outcome {
        guard let data = try? Data(contentsOf: url) else { return .failed }
        guard data.count <= signatureImageLimit else {
            return .tooLarge(
                limit: ByteCountFormatter.string(
                    fromByteCount: Int64(signatureImageLimit),
                    countStyle: .file
                )
            )
        }
        guard let mediaType = mediaType(for: url) else { return .failed }
        return .dataURL("data:\(mediaType);base64,\(data.base64EncodedString())")
    }

    /// The file's `image/*` media type. Anything else is refused here rather than embedded:
    /// the editor would drop it anyway (it only accepts `data:image/`), and refusing at the
    /// picker is where the user can still be told.
    private static func mediaType(for url: URL) -> String? {
        let type = (try? url.resourceValues(forKeys: [.contentTypeKey]).contentType)
            ?? UTType(filenameExtension: url.pathExtension)
        guard let mime = type?.preferredMIMEType, mime.hasPrefix("image/") else { return nil }
        return mime
    }
}

/// The editor for one signature: its name, the rich body, and an "add image" button. `save`
/// receives the name and both body renderings; the caller decides whether that is a create or an
/// update (it knows which signature it opened).
struct SignatureEditorView: View {
    let title: String
    let initialName: String
    let initialBodyHTML: String?
    let save: (String, String, String) -> Void
    let cancel: () -> Void

    @State private var editor: SignatureEditor
    @State private var name: String
    @State private var imageError: String?

    init(
        title: String,
        initialName: String,
        initialBodyHTML: String?,
        save: @escaping (String, String, String) -> Void,
        cancel: @escaping () -> Void
    ) {
        self.title = title
        self.initialName = initialName
        self.initialBodyHTML = initialBodyHTML
        self.save = save
        self.cancel = cancel
        let editor = SignatureEditor()
        editor.pendingBody = initialBodyHTML
        _editor = State(initialValue: editor)
        _name = State(initialValue: initialName)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title).font(.headline)
            LabeledContent(L10n.settings_signatures_name_label()) {
                TextField(L10n.settings_signatures_name_placeholder(), text: $name)
                    .textFieldStyle(.roundedBorder)
            }
            Text(L10n.settings_signatures_body_label())
                .font(.subheadline)
                .foregroundStyle(.secondary)
            SignatureEditorWebView(editor: editor)
                .frame(minHeight: 200)
                .border(.quaternary)
            HStack {
                Button {
                    chooseImage()
                } label: {
                    Label(L10n.settings_signatures_insert_image(), systemImage: "photo")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                Spacer()
            }
            if let imageError {
                Text(imageError).font(.caption).foregroundStyle(.red)
            }
            HStack {
                Spacer()
                Button(L10n.action_cancel(), role: .cancel) { cancel() }
                Button(L10n.settings_signatures_save()) { commit() }
                    .buttonStyle(.borderedProminent)
                    .disabled(name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty)
            }
        }
        .padding(16)
        #if os(macOS)
        // A minimum only makes sense where the sheet can size itself. On iPhone the screen is
        // narrower than this (402pt), and a minWidth wider than the screen does not scroll, it
        // clips, cutting the field labels off the left edge and Save off the right.
        .frame(minWidth: 460, minHeight: 420)
        #else
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        #endif
        .modifier(ComposerLinkDialogModifier(request: $editor.linkRequest, channel: editor.hostChannel))
    }

    private func commit() {
        editor.body { result in
            guard let result else { return }
            save(name.trimmingCharacters(in: .whitespacesAndNewlines), result.html, result.plain)
        }
    }

    private func chooseImage() {
        imageError = nil
        #if os(macOS)
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.allowedContentTypes = [.image]
        panel.begin { response in
            guard response == .OK, let url = panel.urls.first else { return }
            insert(url)
        }
        #else
        PlatformFilePicker.present { urls in
            guard let url = urls.first else { return }
            insert(url)
        }
        #endif
    }

    private func insert(_ url: URL) {
        switch SignatureImage.load(url) {
        case let .dataURL(dataURL):
            editor.insertImage(dataURL: dataURL, altText: url.deletingPathExtension().lastPathComponent)
        case let .tooLarge(limit):
            imageError = L10n.settings_signatures_image_too_large(limit: limit)
        case .failed:
            imageError = L10n.settings_signatures_image_failed()
        }
    }
}
