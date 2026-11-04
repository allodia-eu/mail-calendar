// Settings → Videoconferencing's one field: the person's Jitsi server (docs/conferencing.md, rule 13).
// Empty until they fill it, and checked by the core before it is stored, so an address it refuses
// is said here and nothing changes. Once one is stored, every calendar whose provider writes a
// meeting offers Jitsi in the editor.

import MailcalBindings
import SwiftUI

struct JitsiServerField: View {
    var model: MailboxModel

    @State private var text = ""
    @State private var refused = false
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
                .onSubmit(store)
                .onChange(of: focused) { _, isFocused in
                    if !isFocused { store() }
                }
                .onChange(of: text) { _, _ in refused = false }
            if refused {
                Text(L10n.settings_jitsi_server_invalid())
                    .font(.footnote)
                    .foregroundStyle(.red)
            }
        }
        .onAppear { text = model.app?.jitsiServer() ?? "" }
    }

    /// Hands the address to the core, which stores it or says it will not; the stored form (its
    /// trailing `/` dropped) is what the field shows afterwards.
    private func store() {
        guard let app = model.app else { return }
        if app.setJitsiServer(server: text) {
            text = app.jitsiServer() ?? ""
        } else {
            refused = true
        }
    }
}
