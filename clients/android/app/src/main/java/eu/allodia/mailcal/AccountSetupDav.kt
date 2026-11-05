// The calendar-and-contacts route: a server that holds a calendar or address book and no mail
// (docs/accounts.md, docs/account-autodetect.md rule 8). Detection reaches it for a domain with a
// calendar or address-book server and no mail server, signed in with the typed address; the manual
// form reaches it as its own account type, with the servers typed. Either way it is a standards
// account without a mailbox, connected through the same password route as IMAP.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.delay
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountSetup
import uniffi.mailcal_bindings.RejectedCertificate

/** An account without mail: no mail server, and the core stores none (`AccountSetup::uses`). */
internal fun davSetup(
    login: String,
    password: String,
    caldav: String,
    carddav: String,
    uses: List<AccountCapability>?,
    accepted: RejectedCertificate?,
) = AccountSetup(
    imapHost = "",
    username = login.trim(),
    password = password,
    smtpHost = null,
    caldavBaseUrl = caldav.trim().ifEmpty { null },
    acceptedCertificate = accepted,
    carddavBaseUrl = carddav.trim().ifEmpty { null },
    uses = uses,
)

/** The manual form's calendar-and-contacts type: a login, either server or both, a password. */
@Composable
internal fun DavManualFields(
    login: String,
    onLogin: (String) -> Unit,
    caldav: String,
    onCaldav: (String) -> Unit,
    carddav: String,
    onCarddav: (String) -> Unit,
    password: String,
    onPassword: (String) -> Unit,
    caldavMissing: Boolean,
    nextcloud: NextcloudSetupUi = NextcloudSetupUi(),
    connecting: Boolean = false,
) {
    val ctx = LocalContext.current
    Text(
        L10n.setup_dav_note(ctx),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    // Asked once the typing settles, and drawn above the fields, which stay as they are.
    val server = caldav.trim().ifEmpty { carddav.trim() }
    LaunchedEffect(server) {
        if (server.isEmpty()) return@LaunchedEffect
        delay(NEXTCLOUD_ASK_AFTER_MS)
        nextcloud.onAsk(server)
    }
    if (server.isNotEmpty() && nextcloud.offers(server)) {
        NextcloudOffer(nextcloud.signingIn, enabled = !connecting) {
            SetupUses.davUses(caldav, carddav)?.let { nextcloud.onSignIn(server, it) }
        }
    }
    SetupField(login, onLogin, L10n.setup_field_username(ctx), keyboardType = KeyboardType.Email)
    SetupField(
        caldav,
        onCaldav,
        L10n.setup_field_caldav_optional(ctx),
        L10n.setup_hint_caldav(ctx),
        KeyboardType.Uri,
        missing = caldavMissing,
    )
    SetupField(carddav, onCarddav, L10n.setup_field_carddav_optional(ctx), L10n.setup_hint_carddav(ctx), KeyboardType.Uri)
    PasswordField(password, onPassword, L10n.setup_field_password(ctx))
}

/** How long the manual form waits after the last keystroke before asking the server typed. */
private const val NEXTCLOUD_ASK_AFTER_MS = 800L

/** Nextcloud's own sign-in, under the line that explains it (docs/nextcloud-sign-in.md rule 2). */
@Composable
private fun NextcloudOffer(signingIn: Boolean, enabled: Boolean, onSignIn: () -> Unit) {
    val ctx = LocalContext.current
    Text(L10n.nextcloud_sign_in_note(ctx), style = MaterialTheme.typography.bodyMedium)
    Box(modifier = Modifier.testTag("nextcloud-sign-in")) {
        SignInButton(enabled = enabled && !signingIn, signingIn = signingIn, label = L10n.nextcloud_sign_in_button(ctx), onClick = onSignIn)
    }
    if (signingIn) {
        Text(
            L10n.nextcloud_sign_in_waiting(ctx),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

/**
 * The found card for a domain with a calendar or address book and no mail server: the uses the
 * core offers, the typed address as the login, and a password. At a Nextcloud server ([nextcloud]
 * set) it leads with Nextcloud's own sign-in, and the password waits behind "Use a password
 * instead".
 */
@Composable
internal fun DavFoundCard(
    email: String,
    offer: UseOffer,
    nextcloud: NextcloudSetupUi? = null,
    connecting: Boolean,
    externalError: String?,
    externalFailure: ConnectFailure?,
    onConnect: (AccountSetup) -> ConnectFailure?,
    onManual: () -> Unit,
) {
    val ctx = LocalContext.current
    val toggles = remember(offer) { UseToggles(offer) }
    var password by remember(offer) { mutableStateOf("") }
    var ownFailure by remember(offer) { mutableStateOf<ConnectFailure?>(null) }
    val failure = ownFailure ?: externalFailure
    var certificateAccepted by remember(failure?.certificate) { mutableStateOf(false) }
    val refused = failure?.certificate
    var passwordShown by remember(offer) { mutableStateOf(nextcloud == null) }
    Column(
        modifier = Modifier.fillMaxSize().systemBarsPadding().imePadding().verticalScroll(rememberScrollState()).padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(L10n.setup_detect_found_title(ctx), style = MaterialTheme.typography.headlineSmall)
        Text(L10n.setup_detect_dav_note(ctx), style = MaterialTheme.typography.bodyMedium)
        UsesChoices(toggles)
        nextcloud?.let { ui ->
            NextcloudOffer(ui.signingIn, enabled = !connecting) {
                if (toggles.flagMissing()) return@NextcloudOffer
                val chosen = toggles.chosen()
                val server = chosen.caldavUrl.ifEmpty { chosen.carddavUrl }
                val uses = chosen.uses ?: SetupUses.davUses(chosen.caldavUrl, chosen.carddavUrl)
                if (server.isNotEmpty() && uses != null) ui.onSignIn(server, uses)
            }
            if (!passwordShown) {
                TextButton(onClick = { passwordShown = true }, modifier = Modifier.testTag("nextcloud-password-instead")) {
                    Text(L10n.setup_imap_signin_password_instead(ctx))
                }
            }
        }
        if (passwordShown) PasswordField(password, { password = it }, L10n.setup_field_password(ctx))
        (failure?.takeIf { it.certificate == null }?.message ?: externalError)?.let {
            Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
        }
        refused?.let { CertificateExceptionPanel(it, certificateAccepted) { on -> certificateAccepted = on } }
        if (passwordShown) ConnectButton(
            enabled = password.isNotBlank() && !connecting && (refused == null || certificateAccepted),
            connecting = connecting,
            label = L10n.action_connect(ctx),
        ) {
            if (toggles.flagMissing()) return@ConnectButton
            val chosen = toggles.chosen()
            ownFailure = onConnect(
                davSetup(
                    login = email,
                    password = password,
                    caldav = chosen.caldavUrl,
                    carddav = chosen.carddavUrl,
                    uses = chosen.uses,
                    accepted = if (certificateAccepted) refused else null,
                ),
            )
        }
        TextButton(onClick = onManual, modifier = Modifier.fillMaxWidth()) { Text(L10n.setup_detect_manual(ctx)) }
    }
}
