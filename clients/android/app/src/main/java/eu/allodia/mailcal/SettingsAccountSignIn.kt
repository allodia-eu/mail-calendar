// An IMAP or JMAP account's Sign-in section (docs/accounts.md rule 17): how it signs in now, and,
// once its server has answered, signing in again or switching to the other way in. What it offers
// is decided in AccountSignInRules; this draws it.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.SignInMethod

/** The section, with a pasted JMAP secret's replacement first while the server refuses it. */
@Composable
internal fun AccountSignInGroup(entry: AccountEntry, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    val signIn = ui.state.signIn
    // Blocking and up to seconds long, so asked off the main thread as the page opens.
    LaunchedEffect(entry.id) { ui.onAskSignInOptions(entry.id) }
    val section = AccountSignInRules.section(entry, signIn.options[entry.id])
    val status = when (section.method) {
        SignInMethod.PASSWORD -> L10n.settings_account_signs_in_password(ctx)
        SignInMethod.PROVIDER -> L10n.settings_account_signs_in_provider(ctx)
    }
    val run = signIn.running?.takeIf { it.accountId == entry.id }
    val idle = signIn.idle
    SettingsGroupCard(L10n.settings_account_signin_heading(ctx), status) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            if (entry.id in ui.pastedSecret) SecretFields(entry, ui)
            if (section.signInAgain) {
                BrowserButton(
                    L10n.signin_expired_action(ctx),
                    busy = run?.switching == false,
                    enabled = idle,
                    tag = "account-sign-in-provider-again",
                ) { ui.onAccountSignIn(entry.id, false) }
            }
            if (section.toProvider) {
                val label = section.providerLabel?.let {
                    L10n.settings_account_switch_to_provider_named(ctx, it)
                } ?: L10n.settings_account_switch_to_provider(ctx)
                BrowserButton(
                    label,
                    busy = run?.switching == true,
                    enabled = idle,
                    tag = "account-switch-to-provider",
                ) { ui.onAccountSignIn(entry.id, true) }
            }
            if (section.toPassword) PasswordSwitch(entry, ui, run, idle)
            if (run?.step == SignInStep.BROWSER) {
                Text(
                    L10n.settings_account_signin_waiting(ctx, entry.address),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (section.offersSwitch) {
                Text(
                    L10n.settings_account_switch_description(ctx),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
    // Before the browser opens: what becomes of what the new sign-in will not reach (rule 18).
    signIn.question?.takeIf { it.accountId == entry.id }?.let { asked ->
        SignInUnreachedDialog(asked.set, ui.onUnreachedChoice)
    }
}

/** A button that opens the browser, with a spinner while its flow runs ([busy]). */
@Composable
private fun BrowserButton(
    label: String,
    busy: Boolean,
    enabled: Boolean,
    tag: String,
    onClick: () -> Unit,
) {
    OutlinedButton(onClick = onClick, enabled = enabled, modifier = Modifier.testTag(tag)) {
        if (busy) {
            CircularProgressIndicator(modifier = Modifier.size(18.dp))
            Spacer(modifier = Modifier.width(8.dp))
        }
        Text(label)
    }
}

/**
 * "Use a password instead": reveals an empty password field, never the stored credential, and the
 * button that tries it. The field empties after each switch.
 */
@Composable
private fun PasswordSwitch(entry: AccountEntry, ui: AccountsSettingsUi, run: SignInRun?, idle: Boolean) {
    val ctx = LocalContext.current
    val saved = ui.state.signIn.passwordsSaved
    var revealed by remember(entry.id, saved) { mutableStateOf(false) }
    var password by remember(entry.id, saved) { mutableStateOf("") }
    if (!revealed) {
        OutlinedButton(
            onClick = { revealed = true },
            enabled = idle,
            modifier = Modifier.testTag("account-switch-to-password"),
        ) { Text(L10n.settings_account_switch_to_password(ctx)) }
        return
    }
    PasswordField(password, { password = it }, L10n.setup_field_password(ctx))
    Button(
        onClick = { ui.onSwitchToPassword(entry.id, password) },
        enabled = idle && password.isNotEmpty(),
        modifier = Modifier.testTag("account-switch-password-save"),
    ) {
        if (run?.step == SignInStep.PASSWORD) {
            CircularProgressIndicator(modifier = Modifier.size(18.dp))
            Spacer(modifier = Modifier.width(8.dp))
        }
        Text(L10n.settings_account_switch_password_save(ctx))
    }
}
