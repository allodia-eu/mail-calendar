import MailcalBindings
import SwiftUI

/// The manual form's server row: the name, the port and the security picker together.
///
/// The port is a field of its own rather than a colon inside the server name, because the person
/// who needs it is the one least likely to know that a colon is where it goes.
extension AccountSetupView {
    @ViewBuilder
    func serverRow(
        placeholder: String, host: Binding<String>, field: Binding<ManualServerField>
    ) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            TextField(placeholder, text: host)
                .setupField(.host)
            HStack(spacing: 8) {
                TextField(L10n.setup_field_port(), text: portBinding(field))
                    .setupField(.host)
                    .frame(maxWidth: 88)
                Picker(L10n.setup_field_security(), selection: securityBinding(field)) {
                    Text(L10n.setup_security_implicit_tls()).tag(ConnectionSecurity.implicitTls)
                    Text(L10n.setup_security_starttls()).tag(ConnectionSecurity.startTls)
                }
                .labelsHidden()
                .pickerStyle(.menu)
            }
        }
    }

    /// Typing in the port field goes through `typePort`, so the field records that the port is now
    /// the user's and stops following the picker.
    private func portBinding(_ field: Binding<ManualServerField>) -> Binding<String> {
        Binding(
            get: { field.wrappedValue.port },
            set: { field.wrappedValue.typePort($0) })
    }

    /// Picking a security goes through `choose`, which moves the port only while it is still ours.
    private func securityBinding(
        _ field: Binding<ManualServerField>
    ) -> Binding<ConnectionSecurity> {
        Binding(
            get: { field.wrappedValue.security },
            set: { field.wrappedValue.choose($0) })
    }
}
