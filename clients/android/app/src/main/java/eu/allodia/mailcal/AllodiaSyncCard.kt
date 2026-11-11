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
import uniffi.mailcal_bindings.AccountPageIntent
import uniffi.mailcal_bindings.AllodiaAccountOffer
import uniffi.mailcal_bindings.AllodiaAccountSyncMode
import uniffi.mailcal_bindings.AllodiaDevices
import uniffi.mailcal_bindings.AllodiaElsewhere
import uniffi.mailcal_bindings.AllodiaSyncFailure

/**
 * Offers to set up, accounts changed elsewhere, and accounts removed elsewhere, as the core keeps
 * them ([devices]).
 *
 * Draws nothing at all when there is nothing to say: before the first pass has answered (null)
 * and after one that found nothing ([AllodiaDevices.nothingFound]). A heading over an empty list
 * would claim the other devices hold no accounts.
 */
@Composable
internal fun AllodiaSyncCard(
    devices: AllodiaDevices?,
    onSetUp: (AllodiaAccountOffer) -> Unit,
    onChange: (AccountPageIntent) -> Unit,
    onSignInAgain: () -> Unit,
) {
    if (devices == null || devices.nothingFound) return
    val ctx = LocalContext.current
    SettingsGroupCard(
        L10n.settings_allodia_sync_heading(ctx),
        L10n.settings_allodia_sync_description(ctx),
    ) {
        Column(modifier = Modifier.fillMaxWidth()) {
            if (devices.checking) {
                CircularProgressIndicator()
                Text(
                    L10n.settings_allodia_sync_checking(ctx),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 8.dp),
                )
            }
            devices.offers.forEach { offer ->
                Text(offer.email, style = MaterialTheme.typography.bodyMedium)
                TextButton(onClick = { onSetUp(offer) }) {
                    Text(L10n.settings_allodia_sync_set_up(ctx))
                }
            }
            // Questions rather than statements, and the only answer this device can act on is
            // "keep what I have": Paused, so the other devices keep the account and this one stops
            // exchanging changes about it. A change on the account's page, so the question and
            // the page's control run and hold as one.
            devices.questions.forEach { question ->
                Text(
                    when (question.elsewhere) {
                        AllodiaElsewhere.CHANGED ->
                            L10n.settings_allodia_changed_elsewhere(ctx, question.email)
                        AllodiaElsewhere.REMOVED ->
                            L10n.settings_allodia_removed_elsewhere(ctx, question.email)
                    },
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                ControlTextButton(
                    label = L10n.settings_allodia_keep_local(ctx),
                    control = question.keep,
                    tag = "allodia-keep-${question.accountId}",
                ) { onChange(AccountPageIntent.SetSharing(question.accountId, AllodiaAccountSyncMode.PAUSED)) }
                question.failed?.let { SharingFailedLine(it) }
            }
            devices.failure?.let { PassFailedRows(it, onSignInAgain) }
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
private fun PassFailedRows(failure: AllodiaSyncFailure, onSignInAgain: () -> Unit) {
    val ctx = LocalContext.current
    when (failure) {
        // An offer, not an error: they are signed in and one feature is asleep.
        AllodiaSyncFailure.NEEDS_REAUTH -> Column(modifier = Modifier.padding(top = 4.dp)) {
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
        AllodiaSyncFailure.SIGNED_OUT -> Column(modifier = Modifier.padding(top = 4.dp)) {
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
        AllodiaSyncFailure.UNAVAILABLE -> Text(
            L10n.settings_allodia_sync_unavailable(ctx),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.error,
            modifier = Modifier.padding(top = 4.dp),
        )
    }
}
