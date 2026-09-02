// The controls the account-setup surfaces are built from, shared by the manual picker
// (AccountSetupScreen) and the detected-account card (AccountSetupDetect) so the two cannot
// drift.
//
// Split from AccountSetupScreen, which had reached the size limit, along the seam it already had:
// that file is one form, these are the pieces every setup surface draws.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalUriHandler
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountSetup
import uniffi.mailcal_bindings.JmapSetup
import uniffi.mailcal_bindings.OAuthRoutes

// The Early Access notice + mandatory confirmation that gates Google sign-in. Google is in Early
// Access while it reviews the app and hard-blocks anyone not on the app's OAuth test-user
// allow-list, so the user must sign up first (the link) and confirm they've done so (the
// checkbox) before we open the browser, or they'd hit Google's block screen instead of ours. The
// caller keeps [confirmed] and disables its "Sign in with Google" button until it is true. Shared
// by the manual picker (AccountSetupScreen) and the detected-account card (AccountSetupDetect).
@androidx.compose.runtime.Composable
internal fun GoogleEarlyAccessInfo(
    confirmed: Boolean,
    onConfirmedChange: (Boolean) -> Unit,
) {
    val ctx = LocalContext.current
    val uriHandler = LocalUriHandler.current
    Text(
        text = L10n.setup_google_early_access_title(ctx),
        style = MaterialTheme.typography.titleSmall,
    )
    Text(
        text = L10n.setup_google_early_access_body(ctx),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    TextButton(onClick = { uriHandler.openUri(L10n.setup_google_early_access_url(ctx)) }) {
        Text(L10n.setup_google_early_access_link(ctx))
    }
    Row(verticalAlignment = Alignment.CenterVertically) {
        Checkbox(checked = confirmed, onCheckedChange = onConfirmedChange)
        Text(
            text = L10n.setup_google_early_access_confirm(ctx),
            style = MaterialTheme.typography.bodyMedium,
        )
    }
}

// The "Sign in with Google" button, shared by the manual picker and the detected-account card.
// [enabled] must already fold in the Early Access confirmation (the caller gates it), so this
// button can never start a sign-in Google would hard-block. Shows a spinner + "Signing in…" while
// the browser sign-in is in flight.
@androidx.compose.runtime.Composable
internal fun GoogleSignInButton(
    enabled: Boolean,
    signingIn: Boolean,
    onClick: () -> Unit,
) {
    val ctx = LocalContext.current
    Button(onClick = onClick, enabled = enabled, modifier = Modifier.fillMaxWidth()) {
        if (signingIn) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                CircularProgressIndicator(modifier = Modifier.size(18.dp))
                Text(L10n.setup_google_signing_in(ctx))
            }
        } else {
            Text(L10n.setup_google_signin(ctx))
        }
    }
}

// The "Sign in with your provider" offer for a JMAP account: a note plus the button, shown only
// when the core confirmed this server advertises discoverable OAuth. Hidden entirely otherwise:
// an always-visible button that fails for most servers is worse than no button.
@androidx.compose.runtime.Composable
internal fun JmapSignInOffer(offered: Boolean, signingIn: Boolean, onClick: () -> Unit) {
    if (!offered) return
    val ctx = LocalContext.current
    Text(
        text = L10n.setup_jmap_signin_note(ctx),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    ConnectButton(
        enabled = !signingIn,
        connecting = signingIn,
        label = L10n.setup_jmap_signin_button(ctx),
        onClick = onClick,
    )
}

// The line that says what a mail server answered, when it says something.
//
// Silent in the ordinary case, a provider that takes a password and always did: there is nothing
// to explain, and a line saying so would be noise on every setup.
@androidx.compose.runtime.Composable
internal fun ImapAuthExplanation(state: ImapAuthState) {
    val ctx = LocalContext.current
    val (text, colour) = when (state) {
        is ImapAuthState.SignIn ->
            L10n.setup_imap_signin_note(ctx) to MaterialTheme.colorScheme.onSurfaceVariant
        ImapAuthState.RegistrationNeeded ->
            L10n.setup_imap_signin_registration_needed(ctx) to
                MaterialTheme.colorScheme.onSurfaceVariant
        ImapAuthState.Failed ->
            L10n.setup_imap_signin_failed(ctx) to MaterialTheme.colorScheme.error
        ImapAuthState.Password -> return
    }
    Text(text = text, style = MaterialTheme.typography.bodySmall, color = colour)
}

// The "Sign in with your provider" button for a mail account. The same shape as the Connect
// button beside it, because the two are alternatives on one card rather than an action and a
// decoration.
@androidx.compose.runtime.Composable
internal fun SignInButton(
    enabled: Boolean,
    signingIn: Boolean,
    label: String,
    onClick: () -> Unit,
) {
    ConnectButton(enabled = enabled, connecting = signingIn, label = label, onClick = onClick)
}

// The Connect button shared by the IMAP and JMAP branches: a spinner while the (blocking)
// connect + first sync runs so an impatient user can't fire it twice and sees it's working.
@androidx.compose.runtime.Composable
internal fun ConnectButton(
    enabled: Boolean,
    connecting: Boolean,
    label: String,
    onClick: () -> Unit,
) {
    Button(onClick = onClick, enabled = enabled, modifier = Modifier.fillMaxWidth()) {
        if (connecting) {
            CircularProgressIndicator(modifier = Modifier.size(18.dp))
        } else {
            Text(label)
        }
    }
}

// A single-line text field for the setup form; [placeholder] is shown when empty and
// [keyboardType] tailors the soft keyboard (email/password vs. plain text).
@androidx.compose.runtime.Composable
internal fun SetupField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
    placeholder: String? = null,
    keyboardType: KeyboardType = KeyboardType.Text,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        modifier = Modifier.fillMaxWidth(),
        singleLine = true,
        label = { Text(label) },
        placeholder = placeholder?.let { text -> { Text(text) } },
        keyboardOptions = KeyboardOptions(keyboardType = keyboardType),
    )
}

// A single-line, masked field for a password or API token in the setup form.
@androidx.compose.runtime.Composable
internal fun PasswordField(
    value: String,
    onValueChange: (String) -> Unit,
    label: String,
) {
    OutlinedTextField(
        value = value,
        onValueChange = onValueChange,
        modifier = Modifier.fillMaxWidth(),
        singleLine = true,
        label = { Text(label) },
        visualTransformation = PasswordVisualTransformation(),
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
    )
}
