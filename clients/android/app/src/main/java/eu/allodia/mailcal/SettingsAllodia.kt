// Settings → Allodia account: the Allodia-account card, drawn from the core's `allodiaCard()`. Its
// Apple, Windows and Linux twins are AllodiaAccountSettings.swift, SettingsDialog.Allodia.cs and
// settings/allodia.rs: keep the states and the wording in step.
//
// It sits apart from the account list rather than in it, because an Allodia account is not a mail
// account: no mailbox, no switcher entry, and a token issued for it cannot touch anyone's mail.
// The setup wizard never offers it.
package eu.allodia.mailcal

import android.content.Context
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
import uniffi.mailcal_bindings.AccountChangeRefusal
import uniffi.mailcal_bindings.AllodiaAccount
import uniffi.mailcal_bindings.AllodiaCard
import uniffi.mailcal_bindings.AllodiaCardState
import uniffi.mailcal_bindings.AllodiaFailure
import uniffi.mailcal_bindings.AllodiaIntent

/**
 * What the card draws: [available] is what this build carries, false for a build from source, and
 * the card is then not mounted at all; [card] is the core's.
 */
internal data class AllodiaSettings(
    val available: Boolean = false,
    val card: AllodiaCard = ALLODIA_SIGNED_OUT,
) {
    /** Who is signed in on a sign-in the service has not refused, or null. */
    val signedIn: AllodiaAccount?
        get() = (card.state as? AllodiaCardState.SignedIn)?.account
}

/**
 * Signed out → sign in or create; a sign-in out → a spinner and Cancel; signed in → who, and what
 * to do about it; a sign-in the service ended → which one, and the two ways on from it.
 *
 * Both routes are offered when signed out because someone who has no account and someone returning
 * to one need different pages, and guessing wrong costs a round trip through a form they did not
 * want.
 */
@Composable
internal fun AllodiaAccountCard(
    state: AllodiaSettings,
    onChange: (AllodiaIntent) -> Unit,
    onManage: () -> Unit,
) {
    val ctx = LocalContext.current
    val card = state.card
    SettingsGroupCard(
        L10n.settings_allodia_heading(ctx),
        L10n.settings_allodia_description(ctx),
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            val browser = card.browser
            val account = card.state
            when {
                browser != null -> {
                    CircularProgressIndicator()
                    Text(
                        L10n.settings_allodia_signing_in(ctx),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        modifier = Modifier.padding(top = 8.dp),
                    )
                    ControlTextButton(L10n.action_cancel(ctx), browser.cancel, "allodia-cancel") {
                        onChange(AllodiaIntent.CancelSignIn(browser.attempt))
                    }
                }
                account is AllodiaCardState.Ended -> {
                    Text(
                        L10n.settings_allodia_session_ended(ctx, account.account.email),
                        style = MaterialTheme.typography.bodyMedium,
                    )
                    Text(
                        L10n.settings_allodia_session_ended_hint(ctx),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    // The ordinary sign-in: it replaces the stored grant once it completes.
                    ControlTextButton(L10n.settings_allodia_reauth_action(ctx), card.signIn, "allodia-sign-in") {
                        onChange(allodiaSignIn())
                    }
                    SignOutButton(card, onChange)
                }
                account is AllodiaCardState.SignedIn -> {
                    // The name is what the person recognises, but the address is what identifies
                    // the account, so the address is always shown, and the name only when the
                    // service holds one.
                    account.account.name?.takeIf { it.isNotBlank() }?.let { name ->
                        Text(name, style = MaterialTheme.typography.bodyMedium)
                    }
                    Text(
                        L10n.settings_allodia_signed_in(ctx, account.account.email),
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
                    SignOutButton(card, onChange)
                }
                else -> {
                    ControlTextButton(L10n.settings_allodia_sign_in(ctx), card.signIn, "allodia-sign-in") {
                        onChange(allodiaSignIn())
                    }
                    ControlTextButton(L10n.settings_allodia_create(ctx), card.create, "allodia-create") {
                        onChange(allodiaSignIn(create = true))
                    }
                }
            }
            card.failure?.let { failure ->
                Spacer(modifier = Modifier.height(4.dp))
                Text(
                    allodiaFailureSaid(ctx, failure),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                )
            }
        }
    }
}

@Composable
private fun SignOutButton(card: AllodiaCard, onChange: (AllodiaIntent) -> Unit) {
    ControlTextButton(L10n.settings_allodia_sign_out(LocalContext.current), card.signOut, "allodia-sign-out") {
        onChange(AllodiaIntent.SignOut)
    }
}

/**
 * Why the last sign-in or sign-out did not work, in the Settings card's words and the first-run
 * card's. A browser returning from another sign-in is remedied by signing in again
 * (docs/provider-oauth.md rule 3); any other refusal is worded as an account's page words it.
 */
internal fun allodiaFailureSaid(ctx: Context, failure: AllodiaFailure): String = when (failure) {
    is AllodiaFailure.SignIn -> when (val refusal = failure.refusal) {
        AccountChangeRefusal.StaleSignIn -> L10n.signin_stale(ctx)
        AccountChangeRefusal.BrowserNotOpened -> L10n.settings_allodia_browser_failed(ctx)
        AccountChangeRefusal.NotFinished -> L10n.settings_allodia_timeout(ctx)
        else -> L10n.settings_allodia_failed(ctx, refusalSaid(ctx, refusal))
    }
    is AllodiaFailure.SignOut -> L10n.settings_allodia_failed(ctx, failure.detail)
}
