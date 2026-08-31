// Settings → Accounts: the Allodia-account card, above the mail accounts. Its Apple, Windows and
// Linux twins are AllodiaAccountSettings.swift, SettingsDialog.Allodia.cs and settings/allodia.rs:
// keep the states and the wording in step.
//
// It sits BESIDE the account list rather than in it, because an Allodia account is not a mail
// account: no mailbox, no switcher entry, and a token issued for it cannot touch anyone's mail.
// The setup wizard never offers it.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AllodiaAccount

/**
 * What the card needs to draw itself. Held by the activity rather than the composable so a sign-in
 * survives the browser hop: the redirect re-enters through `onNewIntent`, not through this screen.
 *
 * [available] is what this build carries, false for a build from source, and the card is then not
 * mounted at all.
 */
internal data class AllodiaSettings(
    val available: Boolean = false,
    val account: AllodiaAccount? = null,
    val signingIn: Boolean = false,
    val failure: String? = null,
)

/**
 * Signed out → sign in or create; signing in → a spinner; signed in → who, and what to do about it.
 *
 * Both routes are offered when signed out because someone who has no account and someone returning
 * to one need different pages, and guessing wrong costs a round trip through a form they did not
 * want.
 */
@Composable
internal fun AllodiaAccountCard(
    state: AllodiaSettings,
    onSignIn: () -> Unit,
    onCreate: () -> Unit,
    onManage: () -> Unit,
    onSignOut: () -> Unit,
) {
    val ctx = LocalContext.current
    SettingsGroupCard(
        L10n.settings_allodia_heading(ctx),
        L10n.settings_allodia_description(ctx),
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            when {
                state.signingIn -> {
                    CircularProgressIndicator()
                    Text(
                        L10n.settings_allodia_signing_in(ctx),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                }
                state.account != null -> {
                    // The name is what the person recognises, but the address is what identifies
                    // the account, so the address is always shown, and the name only when the
                    // service holds one.
                    state.account.name?.takeIf { it.isNotBlank() }?.let { name ->
                        Text(name, style = MaterialTheme.typography.bodyMedium)
                    }
                    Text(
                        L10n.settings_allodia_signed_in(ctx, state.account.email),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    // Managing and deleting are the same page, named twice on purpose: an
                    // account someone can create has to offer deletion somewhere findable, and
                    // "Manage account" is not the word anybody looks for when they want out.
                    TextButton(onClick = onManage) {
                        Text(L10n.settings_allodia_manage(ctx))
                    }
                    Text(
                        L10n.settings_allodia_manage_hint(ctx),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    TextButton(onClick = onManage) {
                        Text(L10n.settings_allodia_delete(ctx))
                    }
                    TextButton(onClick = onSignOut) {
                        Text(L10n.settings_allodia_sign_out(ctx))
                    }
                }
                else -> {
                    TextButton(onClick = onSignIn) {
                        Text(L10n.settings_allodia_sign_in(ctx))
                    }
                    TextButton(onClick = onCreate) {
                        Text(L10n.settings_allodia_create(ctx))
                    }
                }
            }
            state.failure?.let { failure ->
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    L10n.settings_allodia_failed(ctx, failure),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
            }
        }
    }
}
