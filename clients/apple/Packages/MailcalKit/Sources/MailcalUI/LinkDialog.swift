// The link dialog: the words a link is shown as and where it points, the way Outlook asks. The
// address is completed and checked by the core as it is typed (`composerLinkAddress`), the same
// rule the answer is held to, so Apply is enabled exactly when the link can be made.

import MailcalBindings
import SwiftUI

struct LinkDialog: View {
    let request: LinkDialogRequest
    let finish: (ComposerLinkAnswer) -> Void

    @State private var text: String
    @State private var address: String
    @FocusState private var focused: Field?

    private enum Field { case text, address }

    init(request: LinkDialogRequest, finish: @escaping (ComposerLinkAnswer) -> Void) {
        self.request = request
        self.finish = finish
        _text = State(initialValue: request.text)
        _address = State(initialValue: request.address)
    }

    private var title: String {
        request.address.isEmpty ? L10n.editor_link_dialog_insert() : L10n.editor_link_dialog_edit()
    }

    private var canApply: Bool { composerLinkAddress(typed: address) != nil }

    /// Said only once something has been typed: an empty field is unfinished, not wrong.
    private var showsHint: Bool {
        !address.trimmingCharacters(in: .whitespaces).isEmpty && !canApply
    }

    private func apply() {
        guard canApply else { return }
        finish(.apply(text: text, address: address))
    }

    var body: some View {
        #if os(macOS)
        VStack(alignment: .leading, spacing: 16) {
            Text(title).font(.headline)
            Form {
                TextField(L10n.editor_link_text(), text: $text)
                    .focused($focused, equals: .text)
                TextField(L10n.editor_link_address(), text: $address)
                    .focused($focused, equals: .address)
                    .autocorrectionDisabled()
                if showsHint {
                    Text(L10n.editor_link_invalid())
                        .font(.caption)
                        .foregroundStyle(.red)
                }
            }
            .formStyle(.columns)
            .onSubmit(apply)
            HStack {
                if request.removable {
                    Button(L10n.editor_link_remove()) { finish(.remove) }
                }
                Spacer()
                Button(L10n.action_cancel(), role: .cancel) { finish(.cancel) }
                    .keyboardShortcut(.cancelAction)
                Button(L10n.editor_link_apply(), action: apply)
                    .keyboardShortcut(.defaultAction)
                    .disabled(!canApply)
            }
        }
        .padding(20)
        .frame(width: 460)
        .onAppear { focused = .address }
        #else
        NavigationStack {
            Form {
                Section {
                    TextField(L10n.editor_link_text(), text: $text)
                        .focused($focused, equals: .text)
                    TextField(L10n.editor_link_address(), text: $address)
                        .focused($focused, equals: .address)
                        .keyboardType(.URL)
                        .textContentType(.URL)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        .submitLabel(.done)
                } footer: {
                    if showsHint {
                        Text(L10n.editor_link_invalid()).foregroundStyle(.red)
                    }
                }
                if request.removable {
                    Section {
                        Button(L10n.editor_link_remove(), role: .destructive) { finish(.remove) }
                    }
                }
            }
            .onSubmit(apply)
            .navigationTitle(title)
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) {
                    Button(L10n.action_cancel(), role: .cancel) { finish(.cancel) }
                }
                ToolbarItem(placement: .confirmationAction) {
                    Button(L10n.editor_link_apply(), action: apply).disabled(!canApply)
                }
            }
        }
        .presentationDetents([.medium])
        .onAppear { focused = .address }
        #endif
    }
}
