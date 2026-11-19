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
#if os(iOS)
import PhotosUI
#endif

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
    func makeNSView(context: Context) -> WKWebView { mount(context) }
    func updateNSView(_ nsView: WKWebView, context: Context) { followScheme(context) }
    #else
    func makeUIView(context: Context) -> WKWebView { mount(context) }
    func updateUIView(_ uiView: WKWebView, context: Context) { followScheme(context) }
    #endif

    private func mount(_ context: Context) -> WKWebView {
        let view = editor.mount()
        followScheme(context)
        return view
    }

    private func followScheme(_ context: Context) {
        editor.host.setDark(context.environment.colorScheme == .dark)
    }
}

/// The editor for one signature: its name, the rich body, and an "add image" button, drawn from
/// the core's page. Save stores it under the session's id, and the page's outcome closes the
/// editor or says why it was not saved.
struct SignatureEditorView: View {
    var model: MailboxModel
    let title: String
    let close: () -> Void

    @State private var editor: SignatureEditor
    @State private var session: SignatureEditorSession
    @State private var name: String
    /// Why the last save or picture did not take, in the catalog's words.
    @State private var problem: String?

    init(
        model: MailboxModel,
        session: SignatureEditorSession,
        title: String,
        initialName: String,
        initialBodyHTML: String?,
        close: @escaping () -> Void
    ) {
        self.model = model
        self.title = title
        self.close = close
        let editor = SignatureEditor()
        editor.pendingBody = initialBodyHTML
        _editor = State(initialValue: editor)
        _session = State(initialValue: session)
        _name = State(initialValue: initialName)
    }

    private var page: SignaturesPage { model.signatures }

    /// Cancel and Add image, held while a change runs on the page.
    private var held: ControlState { .free(!page.save.running) }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text(title).font(.headline)
            HeldFields(control: page.save) {
                LabeledContent(L10n.settings_signatures_name_label()) {
                    TextField(L10n.settings_signatures_name_placeholder(), text: $name)
                        .textFieldStyle(.roundedBorder)
                }
            }
            Text(L10n.settings_signatures_body_label())
                .font(.subheadline)
                .foregroundStyle(.secondary)
            SignatureEditorWebView(editor: editor)
                .frame(minHeight: 200)
                .border(.quaternary)
            HStack {
                HeldFields(control: held) {
                    insertImageControl
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                }
                Spacer()
            }
            if let problem {
                Text(problem).font(.caption).foregroundStyle(.red)
            }
            HStack {
                Spacer()
                ControlButton(
                    title: L10n.action_cancel(), role: .cancel, control: held, action: close)
                ControlButton(
                    title: L10n.settings_signatures_save(),
                    control: SignatureEditorSession.save(page, name: name), action: commit)
                    .buttonStyle(.borderedProminent)
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
        // A save that is running closes the editor or says why it did not take, so the sheet is
        // not swiped away under it.
        .interactiveDismissDisabled(page.save.running)
        .onChange(of: page.outcome?.serial) { _, _ in
            switch session.take(page.outcome) {
            case .close: close()
            case let .refused(reason): problem = SignatureLabels.refusal(reason)
            case nil: break
            }
        }
    }

    /// Sends the name as typed, which the core trims, and the body the bundle reads back.
    private func commit() {
        problem = nil
        editor.body { result in
            guard let result else { return }
            model.saveSignature(session.id, name: name, bodyHTML: result.html, bodyPlain: result.plain)
        }
    }

    /// Add image. On iPhone and iPad a menu, because a logo is as likely to be in the photo
    /// library as in Files, and the document picker cannot reach the library.
    @ViewBuilder private var insertImageControl: some View {
        #if os(macOS)
        Button {
            chooseImage()
        } label: {
            Label(L10n.settings_signatures_insert_image(), systemImage: "photo")
        }
        #else
        PhotosOrFilesMenu(photos: choosePhoto, files: chooseImage) {
            Label(L10n.settings_signatures_insert_image(), systemImage: "photo")
        }
        #endif
    }

    #if os(iOS)
    /// A photograph arrives as a JPEG, because `PlatformPhotoPicker` asks for the compatible form,
    /// and is asked of the core's rule as a file from Files is.
    private func choosePhoto() {
        problem = nil
        PlatformPhotoPicker.present(filter: .images, limit: 1) { pick in
            if let url = pick.files.first {
                insert(url)
            } else if !pick.failed.isEmpty {
                problem = L10n.settings_signatures_image_failed()
            }
        }
    }
    #endif

    private func chooseImage() {
        problem = nil
        #if os(macOS)
        let panel = NSOpenPanel()
        panel.canChooseFiles = true
        panel.canChooseDirectories = false
        panel.allowsMultipleSelection = false
        panel.allowedContentTypes = SignatureImage.types
        panel.begin { response in
            guard response == .OK, let url = panel.urls.first else { return }
            insert(url)
        }
        #else
        PlatformFilePicker.present(contentTypes: SignatureImage.types, multiple: false) { urls in
            guard let url = urls.first else { return }
            insert(url)
        }
        #endif
    }

    private func insert(_ url: URL) {
        switch SignatureImage.load(url) {
        case let .dataURL(dataURL):
            editor.insertImage(dataURL: dataURL, altText: url.deletingPathExtension().lastPathComponent)
        case let .refused(refusal):
            problem = SignatureLabels.image(refusal)
        case .failed:
            problem = L10n.settings_signatures_image_failed()
        }
    }
}
