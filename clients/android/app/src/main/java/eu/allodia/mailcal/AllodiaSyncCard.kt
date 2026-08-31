// Settings → Accounts: what the person's other devices have to say, above their own mail accounts.
// Its Apple, Windows and Linux twins draw the same three things in the same order, keep the states
// and the wording in step.
//
// It sits in Accounts rather than in the Allodia-account category because what it is about is mail
// accounts: one arriving is an account to set up, and that is where somebody looks for it.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AllodiaAccountOffer
import uniffi.mailcal_bindings.AllodiaGrantHealth

/**
 * Offers to set up, accounts that moved elsewhere, and accounts removed elsewhere.
 *
 * Draws nothing at all when there is nothing to say, including before the first pass has run,
 * which is not the same as a pass that found nothing and must not look like one.
 */
@Composable
internal fun AllodiaSyncCard(
    state: AllodiaSyncState,
    onSetUp: (AllodiaAccountOffer) -> Unit,
    onKeepLocal: (String) -> Unit,
    onSignInAgain: () -> Unit,
) {
    val ctx = LocalContext.current
    val report = state.report
    val hasSomethingToSay = report != null &&
        (report.offers.isNotEmpty() ||
            report.changedElsewhere.isNotEmpty() ||
            report.removedElsewhere.isNotEmpty())
    if (!state.checking && state.failure == null && !hasSomethingToSay) return

    SettingsGroupCard(
        L10n.settings_allodia_sync_heading(ctx),
        L10n.settings_allodia_sync_description(ctx),
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            if (state.checking) {
                CircularProgressIndicator()
                Text(
                    L10n.settings_allodia_sync_checking(ctx),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 8.dp),
                )
            }
            report?.offers?.forEach { offer ->
                Text(offer.email, style = MaterialTheme.typography.bodyMedium)
                TextButton(onClick = { onSetUp(offer) }) {
                    Text(L10n.settings_allodia_sync_set_up(ctx))
                }
            }
            // Both of these are questions rather than statements, and the only answer this device
            // can act on today is "keep what I have". Applying the other side's settings needs a
            // path for editing a connected account's server details, which does not exist yet.
            report?.changedElsewhere?.forEach { change ->
                Text(
                    L10n.settings_allodia_changed_elsewhere(ctx, change.email),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                TextButton(onClick = { onKeepLocal(change.accountId) }) {
                    Text(L10n.settings_allodia_keep_local(ctx))
                }
            }
            report?.removedElsewhere?.forEach { change ->
                Text(
                    L10n.settings_allodia_removed_elsewhere(ctx, change.email),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                TextButton(onClick = { onKeepLocal(change.accountId) }) {
                    Text(L10n.settings_allodia_keep_local(ctx))
                }
            }
            if (state.failure != null) {
                AllodiaSyncFailure(state.health, onSignInAgain)
            }
        }
    }
}

/**
 * What a failed pass is allowed to put on screen.
 *
 * The core's typed answer decides, never the failure's text. A grant that predates a permission and
 * one the service revoked are different sentences with different remedies, and everything else says
 * nothing about the sign-in at all, so it gets one plain line and the detail stays in the
 * diagnostic log. There is no longer a path from an exception's message to a screen, which is what
 * put `invalid_scope, unable to issue scope mailcal:accounts:read` in front of somebody.
 */
@Composable
private fun AllodiaSyncFailure(health: AllodiaGrantHealth, onSignInAgain: () -> Unit) {
    val ctx = LocalContext.current
    when (health) {
        // An offer, not an error: they are signed in and one feature is asleep.
        AllodiaGrantHealth.NEEDS_REAUTH -> Column(modifier = Modifier.padding(top = 4.dp)) {
            Text(
                L10n.settings_allodia_reauth(ctx),
                style = MaterialTheme.typography.bodyMedium,
            )
            Text(
                L10n.settings_allodia_reauth_hint(ctx),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            TextButton(onClick = onSignInAgain) {
                Text(L10n.settings_allodia_reauth_action(ctx))
            }
        }
        AllodiaGrantHealth.SIGNED_OUT -> Column(modifier = Modifier.padding(top = 4.dp)) {
            Text(
                L10n.settings_allodia_signed_out_elsewhere(ctx),
                style = MaterialTheme.typography.bodyMedium,
            )
            Text(
                L10n.settings_allodia_signed_out_elsewhere_hint(ctx),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            TextButton(onClick = onSignInAgain) {
                Text(L10n.settings_allodia_sign_in(ctx))
            }
        }
        AllodiaGrantHealth.OK -> Text(
            L10n.settings_allodia_sync_unavailable(ctx),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.error,
            modifier = Modifier.padding(top = 4.dp),
        )
    }
}
