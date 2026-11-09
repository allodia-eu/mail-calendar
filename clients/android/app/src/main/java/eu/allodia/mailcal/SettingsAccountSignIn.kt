// An account's Sign-in section (docs/accounts.md rule 17), drawn from the core's record: the line
// saying how it signs in or what is wrong, the ways in through a browser, the browser sign-in out
// with its question and its Cancel, and the field a password is typed into. Which of them it offers,
// and what each is doing, is the core's; this words and draws them.
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
import uniffi.mailcal_bindings.BrowserSignInStage
import uniffi.mailcal_bindings.ControlState
import uniffi.mailcal_bindings.TypedPassword

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

/** What the section says while the browser is open: where to go on, and as whom. */
internal fun signInWaiting(ctx: Context, stage: BrowserSignInStage.InBrowser, entry: AccountEntry): String =
    if (stage.redirectUri == null) {
        L10n.nextcloud_sign_in_waiting(ctx)
    } else {
        L10n.settings_account_signin_waiting(ctx, entry.address)
    }

/** The section the core gave [entry]. */
@Composable
internal fun AccountSignInGroup(entry: AccountEntry, section: AccountSignIn, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    val browser = section.browser
    SettingsGroupCard(L10n.settings_account_signin_heading(ctx), signInLine(ctx, section.line, entry)) {
        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            // A route pressed while the browser waits starts the sign-in again.
            section.routes.forEach { route ->
                ControlButton(
                    label = signInRouteLabel(ctx, route),
                    control = section.control,
                    tag = routeTag(route),
                ) { ui.onSignIn(entry.id, emptyList()) }
            }
            section.password?.let { PasswordEntry(entry, it, section.control, ui) }
            val waiting = browser?.stage as? BrowserSignInStage.InBrowser
            if (browser != null && waiting != null) {
                Text(
                    signInWaiting(ctx, waiting, entry),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                ControlTextButton(L10n.action_cancel(ctx), browser.cancel, "account-sign-in-cancel") {
                    ui.onChange(AccountPageIntent.CancelSignIn(entry.id, browser.attempt))
                }
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
    val asking = browser?.stage as? BrowserSignInStage.Asking
    val set = asking?.let { UnreachedRules.setOf(it.unreached) }
    if (browser != null && set != null) {
        SignInUnreachedDialog(set) { choice ->
            ui.onChange(AccountPageIntent.AnswerUnreached(entry.id, browser.attempt, UnreachedRules.answer(choice)))
        }
    }
}

/**
 * The password field. One that switches from the provider's sign-in waits behind "Use a password
 * instead" until it runs; one that replaces a refused password is drawn at once. It never shows
 * the stored credential, and empties as it is sent. The button that reveals it opens a field and
 * starts nothing, so it is held as the [routes] are, without their spinner.
 */
@Composable
private fun PasswordEntry(entry: AccountEntry, field: AccountPasswordField, routes: ControlState, ui: AccountsSettingsUi) {
    val ctx = LocalContext.current
    var revealed by remember(entry.id) { mutableStateOf(false) }
    var password by remember(entry.id) { mutableStateOf("") }
    if (!field.replaces && !revealed && !field.save.running) {
        ControlButton(
            label = L10n.settings_account_switch_to_password(ctx),
            control = routes.copy(running = false),
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
