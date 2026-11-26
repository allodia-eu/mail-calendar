// The manual form's Nextcloud pane (docs/nextcloud-sign-in.md rule 2): the server's address alone,
// for a person whose email address leads detection nowhere near their Nextcloud server. The core
// asks the address whether it is Nextcloud before a browser opens; when it cannot tell, the pane
// says so and offers the sign-in anyway, for as long as the field holds the address it asked.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.KeyboardType

@Composable
internal fun NextcloudManualFields(
    address: String,
    onAddress: (String) -> Unit,
    nextcloud: NextcloudSetupUi,
    connecting: Boolean,
) {
    val ctx = LocalContext.current
    val busy = nextcloud.signingIn || connecting
    val canSignIn = address.isNotBlank() && !busy
    Text(
        L10n.setup_nextcloud_note(ctx),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    SetupField(
        address,
        onAddress,
        L10n.setup_field_nextcloud_server(ctx),
        keyboardType = KeyboardType.Uri,
        onGo = { if (canSignIn) nextcloud.onSignInAt(address, false) },
        enabled = !busy,
    )
    nextcloud.unsureFor(address)?.let { host ->
        Text(L10n.setup_nextcloud_unsure(ctx, host = host), style = MaterialTheme.typography.bodyMedium)
        OutlinedButton(
            onClick = { nextcloud.onSignInAt(address, true) },
            enabled = !busy,
            modifier = Modifier.fillMaxWidth().testTag("nextcloud-try-anyway"),
        ) {
            Text(L10n.setup_nextcloud_try_anyway(ctx))
        }
    }
    Box(modifier = Modifier.testTag("nextcloud-pane-sign-in")) {
        SignInButton(
            enabled = canSignIn,
            signingIn = nextcloud.signingIn,
            label = L10n.nextcloud_sign_in_button(ctx),
            onClick = { nextcloud.onSignInAt(address, false) },
        )
    }
    if (nextcloud.signingIn) {
        Text(
            if (nextcloud.checking) L10n.setup_nextcloud_checking(ctx) else L10n.nextcloud_sign_in_waiting(ctx),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}
