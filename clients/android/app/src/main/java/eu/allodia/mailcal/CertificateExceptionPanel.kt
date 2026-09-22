// The certificate a server offered that could not be verified, and the decision it puts to the
// person in front of it. Shared by the detected card and the manual form so both ask the same
// question in the same words (docs/certificate-exceptions.md).
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import java.text.DateFormat
import java.util.Date
import uniffi.mailcal_bindings.RejectedCertificate

// The warning, what the certificate claims about itself, and the confirmation that unlocks
// Connect. Drawn only after a connect was refused for a certificate; until [accepted] is on,
// Connect stays inert, the same shape the untrusted-settings gate next door uses.
@Composable
internal fun CertificateExceptionPanel(
    certificate: RejectedCertificate,
    accepted: Boolean,
    onAcceptedChange: (Boolean) -> Unit,
) {
    val ctx = LocalContext.current
    Column(verticalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.fillMaxWidth()) {
        Text(
            L10n.setup_certificate_warning(ctx, certificate.serverName),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.error,
        )
        party(certificate.subjectCommonName, certificate.subjectOrganisation)?.let {
            CertificateDetail(L10n.setup_certificate_issued_to(ctx), it)
        }
        party(certificate.issuerCommonName, certificate.issuerOrganisation)?.let {
            CertificateDetail(L10n.setup_certificate_issued_by(ctx), it)
        }
        validity(certificate)?.let { CertificateDetail(L10n.setup_certificate_valid(ctx), it) }
        CertificateDetail(L10n.setup_certificate_fingerprint(ctx), certificate.sha256)
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Switch(checked = accepted, onCheckedChange = onAcceptedChange)
            Text(L10n.setup_certificate_confirm(ctx), style = MaterialTheme.typography.bodyMedium)
        }
    }
}

// One claim, label first, so the values line up down the panel.
@Composable
private fun CertificateDetail(label: String, value: String) {
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), modifier = Modifier.fillMaxWidth()) {
        Text(
            label,
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            modifier = Modifier.width(88.dp),
        )
        Text(value, style = MaterialTheme.typography.bodySmall)
    }
}

// "name (organisation)", or whichever of the two the certificate carries. Null when it names
// neither, which is when there is nothing to show rather than an empty row.
internal fun party(commonName: String?, organisation: String?): String? = when {
    commonName != null && organisation != null -> "$commonName ($organisation)"
    commonName != null -> commonName
    else -> organisation
}

// The window the certificate claims, as a date range in the reader's own locale. The core sends
// epoch seconds and formats nothing (docs/timestamps.md).
internal fun validity(certificate: RejectedCertificate): String? {
    val from = certificate.notBefore ?: return null
    val until = certificate.notAfter ?: return null
    val format = DateFormat.getDateInstance(DateFormat.LONG)
    return "${format.format(Date(from * 1000))} – ${format.format(Date(until * 1000))}"
}
