// An account's Sign-in section (docs/accounts.md rule 17), drawn from the core's record: the line
// saying how it signs in or what is wrong, the ways in through a browser, and the field a password
// is typed into. Which of them it offers is the core's; this words and draws them. A browser
// sign-in is still the activity's to run, so its button spins from the activity's own state.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountEntry
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.AccountPasswordField
import uniffi.mailcal_bindings.AccountSecretKind
import uniffi.mailcal_bindings.AccountSignIn
import uniffi.mailcal_bindings.AccountSignInLine
import uniffi.mailcal_bindings.AccountSignInRoute
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.TypedPassword
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind

/** The line under the heading, for [entry]. */
internal fun signInLine(ctx: Context, line: AccountSignInLine, entry: AccountEntry): String = when (line) {
    AccountSignInLine.PASSWORD -> L10n.settings_account_signs_in_password(ctx)
    AccountSignInLine.PROVIDER -> L10n.settings_account_signs_in_provider(ctx)
    AccountSignInLine.NEXTCLOUD -> L10n.settings_account_signs_in_nextcloud(ctx)
    AccountSignInLine.AT_PROVIDER -> L10n.settings_account_signin_description(ctx)
    AccountSignInLine.NEEDS_PERMISSION -> L10n.settings_account_needs_permission(ctx)
    AccountSignInLine.REFUSED -> L10n.signin_expired_prompt(ctx, accountName(ctx, entry))
}

/** The words on [route]'s button. */
internal fun signInRouteLabel(ctx: Context, route: AccountSignInRoute): String = when (route) {
    is AccountSignInRoute.SignInAgain -> L10n.signin_expired_action(ctx)
    is AccountSignInRoute.ToProvider -> route.label?.let {
        L10n.settings_account_switch_to_provider_named(ctx, it)
    } ?: L10n.settings_account_switch_to_provider(ctx)
    is AccountSignInRoute.ToNextcloud -> L10n.settings_account_switch_to_provider_named(ctx, "Nextcloud")
}

private fun routeTag(route: AccountSignInRoute): String = when (route) {
    is AccountSignInRoute.SignInAgain -> "account-sign-in-again"
    is AccountSignInRoute.ToProvider -> "account-switch-to-provider"
    is AccountSignInRoute.ToNextcloud -> "account-switch-to-nextcloud"
}

/** The section the core gave [entry]. */
@Composable
internal fun AccountSignInGroup(entry: AccountEntry, section: AccountSignIn, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    // Microsoft and Google sign in again through their own consent; every other account through
    // its own sign-in, which may ask a question before its browser opens.
    val consent = entry.kind == CoreAccountKind.MICROSOFT || entry.kind == CoreAccountKind.GOOGLE
    val signIn = ui.state.signIn
    val run = signIn.running?.takeIf { it.accountId == entry.id }
    val idle = if (consent) ui.state.signingIn == null else signIn.idle
    val waiting = if (consent) ui.state.signingIn == entry.id else run?.step == SignInStep.BROWSER
    // Held, too, while the core works on a browser sign-in the activity runs.
    val free = section.control.enabled && idle
    SettingsGroupCard(L10n.settings_account_signin_heading(ctx), signInLine(ctx, section.line, entry)) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            section.routes.forEach { route ->
                val again = route is AccountSignInRoute.SignInAgain
                val busy = if (consent) waiting else run != null && run.switching != again
                ControlButton(
                    label = signInRouteLabel(ctx, route),
                    control = ControlState(enabled = free, running = busy),
                    tag = routeTag(route),
                ) {
                    if (consent) ui.onSignInAgain(entry.id, emptyList()) else ui.onAccountSignIn(entry.id, !again)
                }
            }
            section.password?.let { PasswordEntry(entry, it, free, ui) }
            if (waiting) {
                Text(
                    L10n.settings_account_signin_waiting(ctx, entry.address),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            if (section.switches) {
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

/**
 * The password field. One that switches from the provider's sign-in waits behind "Use a password
 * instead" until it runs; one that replaces a refused password is drawn at once. It never shows
 * the stored credential, and empties as it is sent.
 */
@Composable
private fun PasswordEntry(entry: AccountEntry, field: AccountPasswordField, enabled: Boolean, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    var revealed by remember(entry.id) { mutableStateOf(false) }
    var password by remember(entry.id) { mutableStateOf("") }
    if (!field.replaces && !revealed && !field.save.running) {
        ControlButton(
            label = L10n.settings_account_switch_to_password(ctx),
            control = ControlState(enabled = enabled, running = false),
            tag = "account-switch-to-password",
        ) { revealed = true }
        return
    }
    val title = when (field.kind) {
        AccountSecretKind.PASSWORD -> L10n.setup_field_password(ctx)
        AccountSecretKind.PASSWORD_OR_TOKEN -> L10n.setup_jmap_secret_placeholder(ctx)
    }
    HeldFields(field.save) { open -> PasswordField(password, { password = it }, title, enabled = open) }
    ControlButton(
        label = if (field.replaces) L10n.action_save(ctx) else L10n.settings_account_switch_password_save(ctx),
        control = field.save,
        tag = "account-password-save",
        ready = password.isNotEmpty(),
    ) {
        ui.onChange(AccountPageIntent.UsePassword(entry.id, TypedPassword(password)))
        password = ""
    }
}
