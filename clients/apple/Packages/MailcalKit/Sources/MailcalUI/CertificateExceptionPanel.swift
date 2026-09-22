// The certificate a server offered that could not be verified, and the decision it puts to the
// person in front of it. Shared by the detected card and the manual form so both ask the same
// question in the same words (`docs/certificate-exceptions.md`).

import SwiftUI
import MailcalBindings

/// The warning, what the certificate claims about itself, and the confirmation that unlocks
/// Connect. Drawn only after a connect was refused for a certificate; until `accepted` is on,
/// Connect stays inert, the same shape the untrusted-settings gate next door uses.
struct CertificateExceptionPanel: View {
    let certificate: RejectedCertificate
    @Binding var accepted: Bool

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            Text(L10n.setup_certificate_warning(server: certificate.serverName))
                .font(.callout)
                .foregroundStyle(.red)
                .fixedSize(horizontal: false, vertical: true)
            VStack(alignment: .leading, spacing: 4) {
                if let subject = Self.party(certificate.subjectCommonName, certificate.subjectOrganisation) {
                    detail(L10n.setup_certificate_issued_to(), subject)
                }
                if let issuer = Self.party(certificate.issuerCommonName, certificate.issuerOrganisation) {
                    detail(L10n.setup_certificate_issued_by(), issuer)
                }
                if let validity { detail(L10n.setup_certificate_valid(), validity) }
                detail(L10n.setup_certificate_fingerprint(), certificate.sha256)
            }
            Toggle(L10n.setup_certificate_confirm(), isOn: $accepted)
        }
    }

    /// One claim, label first, so the values line up down the panel.
    private func detail(_ label: String, _ value: String) -> some View {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
            Text(label)
                .font(.caption.weight(.semibold))
                .foregroundStyle(.secondary)
                .frame(width: 84, alignment: .leading)
            Text(value)
                .font(.caption)
                .textSelection(.enabled)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: 0)
        }
    }

    /// "name (organisation)", or whichever of the two the certificate carries. `nil` when it
    /// names neither, which is when there is nothing to show rather than an empty row.
    private static func party(_ commonName: String?, _ organisation: String?) -> String? {
        switch (commonName, organisation) {
        case let (name?, organisation?): return "\(name) (\(organisation))"
        case let (name?, nil): return name
        case let (nil, organisation?): return organisation
        case (nil, nil): return nil
        }
    }

    /// The window the certificate claims, as a date range in the reader's own locale. The core
    /// sends epoch seconds and formats nothing (`docs/timestamps.md`).
    private var validity: String? {
        guard let from = certificate.notBefore, let until = certificate.notAfter else { return nil }
        return "\(Self.day(from)) – \(Self.day(until))"
    }

    private static func day(_ epochSeconds: Int64) -> String {
        certificateDateFormatter.string(from: Date(timeIntervalSince1970: TimeInterval(epochSeconds)))
    }
}

/// Built once: `DateFormatter` loads ICU locale data on construction, the cost the timestamp
/// helpers elsewhere in this package carry the same warning about.
@MainActor private let certificateDateFormatter: DateFormatter = {
    let output = DateFormatter()
    output.dateStyle = .long
    output.timeStyle = .none
    return output
}()
