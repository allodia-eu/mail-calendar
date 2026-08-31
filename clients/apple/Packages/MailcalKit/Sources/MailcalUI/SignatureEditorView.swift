// The signature body editor: the shared `clients/composer/dist/editor.html` bundle hosted body-only,
// with the same hardened WKWebView configuration as the composer (docs/composer-security.md):
// local assets, JS for this document only, every remote load and navigation blocked. Authoring a
// signature is authoring mail content, so it gets the composer's gates, not a lighter set.
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
final class SignatureEditor: NSObject, WKNavigationDelegate {
    let webView: WKWebView
    private var expectingInitialLoad = true
    /// The body to load once the bundle has finished loading, set for an existing signature,
    /// `nil` for a new one. Applied in `didFinish` because the bundle loads asynchronously.
    var pendingBody: String?

    override init() {
        let configuration = WKWebViewConfiguration()
        let preferences = WKWebpagePreferences()
        preferences.allowsContentJavaScript = true
        configuration.defaultWebpagePreferences = preferences
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = false
        configuration.websiteDataStore = .nonPersistent()
        webView = WKWebView(frame: .zero, configuration: configuration)
        super.init()
        webView.navigationDelegate = self
        webView.allowsBackForwardNavigationGestures = false
        installRemoteBlockThenLoad()
    }

    /// Compiles the same block-every-remote-subresource rule list the composer installs, the
    /// native barrier behind the bundle's CSP. If compilation fails the CSP still blocks remote
    /// loads, so the editor is loaded anyway rather than left blank.
    private func installRemoteBlockThenLoad() {
        WKContentRuleListStore.default().compileContentRuleList(
            forIdentifier: "composer-block-remote",
            encodedContentRuleList: Self.blockRemoteRuleList
        ) { [weak self] ruleList, _ in
            guard let self else { return }
            if let ruleList {
                self.webView.configuration.userContentController.add(ruleList)
            }
            self.loadEditor()
        }
    }

    private static let blockRemoteRuleList = """
        [{"trigger":{"url-filter":"^https?://"},"action":{"type":"block"}}]
        """

    // Always call `setSignatureBody`, even for a brand-new signature with no body: it also carries
    // the placeholder, and the bundle's default ("Write your message") is the composer's wording,
    // which is wrong here.
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        // The toolbar's strings first, then the body, `setSignatureBody` carries this surface's own
        // placeholder and must win over the composer wording `setComposerLabels` sends.
        webView.evaluateJavaScript(ComposerLabels.script())
        let body = Self.jsString(pendingBody ?? "")
        let placeholder = Self.jsString(L10n.settings_signatures_placeholder())
        webView.evaluateJavaScript("window.setSignatureBody(\(body), \(placeholder))")
        // Writing the signature is the only thing this screen is for, so the caret opens in it.
        // Asked for rather than assumed: the shared bundle focuses nothing of its own accord,
        // because in the composer the caret belongs in To (docs/contacts.md §4).
        webView.evaluateJavaScript("window.focusComposerBody()")
    }

    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping @MainActor (WKNavigationActionPolicy) -> Void
    ) {
        if expectingInitialLoad {
            expectingInitialLoad = false
            decisionHandler(.allow)
        } else {
            decisionHandler(.cancel)
        }
    }

    /// Reads back the authored signature, the HTML to store and its plain-text rendering.
    /// `nil` if the editor could not be read (the bundle is still loading).
    func body(_ completion: @escaping ((html: String, plain: String)?) -> Void) {
        webView.evaluateJavaScript("window.signatureBody()") { value, _ in
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
        webView.evaluateJavaScript("window.insertSignatureImage(\(Self.jsString(json)))")
    }

    private func loadEditor() {
        let asset = SignatureEditorAsset.load()
        webView.loadHTMLString(asset.html, baseURL: asset.baseURL)
    }

    /// Encodes `value` as a JavaScript string literal so it can be passed into an
    /// `evaluateJavaScript` call without breaking out of the argument.
    private static func jsString(_ value: String) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: value, options: .fragmentsAllowed),
              let literal = String(data: data, encoding: .utf8)
        else {
            return "\"\""
        }
        return literal
    }
}

/// Loads the shared editor bundle, by the same three routes the composer uses (SPM resource, app
/// bundle, then the source tree for a `swift run` from the checkout).
private struct SignatureEditorAsset {
    let html: String
    let baseURL: URL?

    static func load() -> SignatureEditorAsset {
        for bundle in [Bundle.module, Bundle.main] {
            if let bundleURL = bundle.url(
                forResource: "editor",
                withExtension: "html",
                subdirectory: "composer"
            ), let html = try? String(contentsOf: bundleURL, encoding: .utf8) {
                return SignatureEditorAsset(html: html, baseURL: bundleURL.deletingLastPathComponent())
            }
        }
        let sourceURL = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("composer/editor.html")
        if let html = try? String(contentsOf: sourceURL, encoding: .utf8) {
            return SignatureEditorAsset(html: html, baseURL: sourceURL.deletingLastPathComponent())
        }
        return SignatureEditorAsset(
            html: "<!doctype html><html><body><script>window.signatureBody=function(){return JSON.stringify({body_html:\"\",body_plain:\"\"});};</script></body></html>",
            baseURL: nil
        )
    }
}

private struct SignatureEditorWebView: PlatformViewRepresentable {
    let editor: SignatureEditor

    #if os(macOS)
    func makeNSView(context: Context) -> WKWebView { editor.webView }
    func updateNSView(_ nsView: WKWebView, context: Context) {}
    #else
    func makeUIView(context: Context) -> WKWebView { editor.webView }
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
