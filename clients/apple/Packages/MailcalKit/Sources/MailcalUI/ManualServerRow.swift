import MailcalBindings
import SwiftUI

/// A mail server's row: the name, the port and the security picker together, on the manual setup
/// form and on an account's page in Settings.
///
/// The port is a field of its own rather than a colon inside the server name, because the person
/// who needs it is the one least likely to know that a colon is where it goes.
struct ManualServerRow: View {
    let placeholder: String
    @Binding var host: String
    @Binding var field: ManualServerField

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            TextField(placeholder, text: $host)
                .setupField(.host)
            HStack(spacing: 8) {
                TextField(L10n.setup_field_port(), text: portBinding)
                    .setupField(.host)
                    .frame(maxWidth: 88)
                Picker(L10n.setup_field_security(), selection: securityBinding) {
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
    private var portBinding: Binding<String> {
        Binding(get: { field.port }, set: { field.typePort($0) })
    }

    /// Picking a security goes through `choose`, which moves the port only while it is still ours.
    private var securityBinding: Binding<ConnectionSecurity> {
        Binding(get: { field.security }, set: { field.choose($0) })
    }
}

extension AccountSetupView {
    func serverRow(
        placeholder: String, host: Binding<String>, field: Binding<ManualServerField>
    ) -> some View {
        ManualServerRow(placeholder: placeholder, host: host, field: field)
    }
}
