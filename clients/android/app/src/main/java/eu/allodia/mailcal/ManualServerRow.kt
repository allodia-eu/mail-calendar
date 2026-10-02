// The manual form's server row: the name, the port and the security choice together.
//
// The port is a field of its own rather than a colon inside the server name, because the person
// who needs it is the one least likely to know that a colon is where it goes.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.width
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.ConnectionSecurity

/** One server's name, port and connection security, as one row of the manual form. */
@Composable
internal fun ServerRow(
    host: String,
    onHostChange: (String) -> Unit,
    label: String,
    placeholder: String?,
    field: ManualServerField,
    onFieldChange: (ManualServerField) -> Unit,
    ctx: Context,
) {
    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Column(modifier = Modifier.weight(1f)) {
                SetupField(host, onHostChange, label, placeholder)
            }
            Column(modifier = Modifier.width(112.dp)) {
                SetupField(
                    field.port,
                    { onFieldChange(field.typePort(it)) },
                    L10n.setup_field_port(ctx),
                    keyboardType = KeyboardType.Number,
                )
            }
        }
        SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
            SECURITIES.forEachIndexed { index, security ->
                SegmentedButton(
                    selected = field.security == security,
                    onClick = { onFieldChange(field.choose(security)) },
                    shape = SegmentedButtonDefaults.itemShape(
                        index = index,
                        count = SECURITIES.size,
                    ),
                ) { Text(security.label(ctx)) }
            }
        }
    }
}

/** The two securities a manual form offers, in the order every client lists them. */
private val SECURITIES = listOf(ConnectionSecurity.IMPLICIT_TLS, ConnectionSecurity.START_TLS)

private fun ConnectionSecurity.label(ctx: Context): String = when (this) {
    ConnectionSecurity.IMPLICIT_TLS -> L10n.setup_security_implicit_tls(ctx)
    ConnectionSecurity.START_TLS -> L10n.setup_security_starttls(ctx)
}
