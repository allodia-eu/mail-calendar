// The controls a page draws from the core's `ControlState` (docs/waiting.md): a switch row, a
// button, a picker and a confirmation. Each is held while it cannot be used and shows a spinner
// beside it while work it started runs, so every page waits the same way.

import MailcalBindings
import SwiftUI

extension ControlState {
    /// A control nothing on the core's side holds or runs: setup's own steps, which keep their
    /// waiting in the model.
    static func free(_ enabled: Bool = true) -> ControlState {
        ControlState(enabled: enabled, running: false)
    }

    /// The same control, held as well while `held`.
    func held(_ held: Bool) -> ControlState {
        ControlState(enabled: enabled && !held, running: running)
    }
}

/// The spinner beside a control whose work is running.
private struct Running: View {
    let control: ControlState

    var body: some View {
        if control.running {
            ProgressView().controlSize(.small)
        }
    }
}

/// A switch, as the copy calls it ("Switch off Contacts?"), with its note beneath. `change` is
/// told what the person switched it to.
struct ControlToggle: View {
    let title: String
    let note: String?
    let on: Bool
    let control: ControlState
    let change: (Bool) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                Toggle(title, isOn: Binding(get: { on }, set: { change($0) }))
                    .toggleStyle(.switch)
                    .disabled(!control.enabled)
                Running(control: control)
            }
            if let note {
                Text(note)
                    .font(.caption)
                    .foregroundStyle(.secondary)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// A button. Its style comes from where it is drawn, as any button's does.
struct ControlButton: View {
    let title: String
    var systemImage: String?
    var role: ButtonRole?
    let control: ControlState
    let action: () -> Void

    var body: some View {
        HStack(spacing: 8) {
            Button(role: role, action: action) {
                if let systemImage {
                    Label(title, systemImage: systemImage)
                } else {
                    Text(title)
                }
            }
            .disabled(!control.enabled)
            Running(control: control)
        }
    }
}

/// A menu picker under its title: `labels` listed, `selected` shown. `pick` is told the position
/// the person picked, and only when it differs from `selected`.
struct ControlPicker: View {
    let title: String
    let labels: [String]
    let selected: Int
    let control: ControlState
    let pick: (Int) -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            // Drawn here rather than left to the Picker: outside a Form, a `.menu` Picker renders
            // its label on macOS but drops it on iOS.
            Text(title).font(.subheadline).bold()
            HStack(spacing: 8) {
                Picker(
                    title,
                    selection: Binding(
                        get: { selected },
                        set: { if $0 != selected { pick($0) } }
                    )
                ) {
                    ForEach(labels.indices, id: \.self) { index in
                        Text(labels[index]).tag(index)
                    }
                }
                .pickerStyle(.menu)
                .labelsHidden()
                .fixedSize()
                .disabled(!control.enabled)
                Running(control: control)
            }
        }
    }
}

/// Asks before a change that deletes something, while `item` holds what it would change: `action`
/// does it, Cancel does not. An `alert`, not a `confirmationDialog`: iPadOS presents the latter as
/// a popover, which drops the `.cancel` button.
struct ConfirmAlert<Item>: ViewModifier {
    @Binding var item: Item?
    let title: (Item) -> String
    let message: (Item) -> String
    let action: String
    let confirm: (Item) -> Void

    func body(content: Content) -> some View {
        content.alert(
            item.map(title) ?? "",
            isPresented: Binding(get: { item != nil }, set: { if !$0 { item = nil } }),
            presenting: item
        ) { item in
            Button(action, role: .destructive) { confirm(item) }
            Button(L10n.action_cancel(), role: .cancel) {}
        } message: { item in
            Text(message(item))
        }
    }
}

extension View {
    /// Asks before a change that deletes something; see `ConfirmAlert`.
    func confirmAlert<Item>(
        _ item: Binding<Item?>, action: String, title: @escaping (Item) -> String,
        message: @escaping (Item) -> String, confirm: @escaping (Item) -> Void
    ) -> some View {
        modifier(ConfirmAlert(
            item: item, title: title, message: message, action: action, confirm: confirm))
    }
}
