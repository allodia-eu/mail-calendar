// Settings → Videoconferencing's one field: the person's Jitsi server (docs/conferencing.md, rule 13).
// The field sends what it holds each time it is submitted or left; the core stores it in its tidied
// form or refuses it, and `VideoconferencingPage` says which. Once one is stored, every calendar
// whose provider writes a meeting offers Jitsi in the editor.

import MailcalBindings
import SwiftUI

/// Whether the field says its address was refused: while it still holds the text the core refused.
func jitsiRefusalShown(text: String, page: VideoconferencingPage) -> Bool {
    page.jitsiRefused == text
}

/// What the field holds after sending `sent`: the stored form, unless the core refused it, when the
/// text stays for the person to correct.
func jitsiFieldText(afterSending sent: String, page: VideoconferencingPage) -> String {
    jitsiRefusalShown(text: sent, page: page) ? sent : page.jitsiServer ?? ""
}

struct JitsiServerField: View {
    var model: MailboxModel

    @State private var text = ""
    @FocusState private var focused: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            TextField(L10n.settings_jitsi_server(), text: $text, prompt: Text(verbatim: "https://meet.example.org"))
                .labelsHidden()
                .textFieldStyle(.roundedBorder)
                .autocorrectionDisabled()
                #if os(iOS)
                    .textInputAutocapitalization(.never)
                    .keyboardType(.URL)
                #endif
                .focused($focused)
                .onSubmit(send)
                .onChange(of: focused) { _, isFocused in
                    if !isFocused { send() }
                }
            if jitsiRefusalShown(text: text, page: model.videoconferencing) {
                Text(L10n.settings_jitsi_server_invalid())
                    .font(.footnote)
                    .foregroundStyle(.red)
            }
        }
        .onAppear { text = model.videoconferencing.jitsiServer ?? "" }
        // A stored server changed while the field is not being typed in.
        .onChange(of: model.videoconferencing.jitsiServer) { _, stored in
            if !focused { text = stored ?? "" }
        }
    }

    private func send() {
        let sent = text
        model.setJitsiServer(sent)
        text = jitsiFieldText(afterSending: sent, page: model.videoconferencing)
    }
}
