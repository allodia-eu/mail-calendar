// The found card: the route detection took for an address, with what it offers the account for
// (docs/account-autodetect.md rule 8). A Microsoft or Google route asks the provider for the uses
// chosen alone; an IMAP route sets the account up with the servers the toggles keep. Split from
// AccountSetupDetect.kt, which holds the flow around it.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountSetup
import uniffi.mailcal_bindings.JmapSetup
import uniffi.mailcal_bindings.SetupRecommendation

// The routed card for a JMAP, IMAP, Microsoft or Google hit; [offer] is what it offers the account
// for, empty for JMAP, whose session says what it offers once signed in.
@Composable
internal fun FoundView(
    recommendation: SetupRecommendation,
    offer: UseOffer,
    connecting: Boolean,
    signingIn: Boolean,
    signingInGoogle: Boolean,
    externalError: String?,
    externalFailure: ConnectFailure? = null,
    onSignInMicrosoft: (String?, List<AccountCapability>?) -> Unit,
    onSignInGoogle: (String?, List<AccountCapability>?) -> Unit,
    onConnect: (AccountSetup) -> ConnectFailure?,
    onConnectJmap: (JmapSetup) -> ConnectFailure?,
    signInOffered: Boolean,
    onSignInJmap: (String, String) -> Unit,
    signingInJmap: Boolean,
    imapAuth: ImapAuthState,
    onSignInImap: ((uniffi.mailcal_bindings.ImapLoginRequest) -> Unit)?,
    signingInImap: Boolean,
    onManual: () -> Unit,
) {
    val ctx = LocalContext.current
    val form = remember(recommendation) { DetectedConnectForm(recommendation) }
    var password by remember(recommendation) { mutableStateOf("") }
    var approved by remember(recommendation) { mutableStateOf(false) }
    // Gates the Google sign-in button here just as it does in the manual picker.
    var googleConfirmed by remember(recommendation) { mutableStateOf(false) }
    // Mail, calendar and contacts, each starting where the core said.
    val toggles = remember(recommendation, offer) { UseToggles(offer) }
    var ownFailure by remember(recommendation) { mutableStateOf<ConnectFailure?>(null) }
    // Whichever connect answered last. `addAccount` runs on a thread of its own, so its answer
    // arrives as `externalFailure` rather than as the return of `onConnect`.
    val failure = ownFailure ?: externalFailure
    // A different certificate is a different decision, so an acceptance never carries over to
    // one nobody has been shown.
    var certificateAccepted by remember(failure?.certificate) { mutableStateOf(false) }
    // The transport's own words, but not while the certificate panel is up: that panel says the
    // same thing in the reader's language and with the certificate beside it. A refusal on a
    // route that cannot carry an exception keeps its message, because nothing else would say it.
    // Read off `failure` rather than the form: the form's fields are assigned below, so they
    // still hold the previous composition's answer here.
    val error = failure
        ?.takeIf { it.certificate == null || recommendation !is SetupRecommendation.Imap }
        ?.message
    form.password = password
    form.approved = approved
    form.uses = toggles.chosen().takeIf { toggles.choosing }
    form.rejectedCertificate = failure?.certificate
    form.certificateAccepted = certificateAccepted

    Column(
        modifier = Modifier.fillMaxSize().systemBarsPadding().verticalScroll(rememberScrollState()).padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(L10n.setup_detect_found_title(ctx), style = MaterialTheme.typography.headlineSmall)

        when (recommendation) {
            is SetupRecommendation.Microsoft -> {
                Text(L10n.setup_detect_microsoft_hint(ctx), style = MaterialTheme.typography.bodyMedium)
                UsesChoices(toggles)
                // A failed/declined sign-in comes back as externalError; show it so the user
                // isn't left on a silent dead-end and can retry or set up manually.
                InlineError(externalError)
                Button(
                    onClick = { onSignInMicrosoft(recommendation.email, form.uses?.uses) },
                    enabled = !signingIn,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    if (signingIn) CircularProgressIndicator(modifier = Modifier.size(18.dp)) else Text(L10n.setup_microsoft_signin(ctx))
                }
            }
            is SetupRecommendation.Google -> {
                Text(L10n.setup_detect_google_hint(ctx), style = MaterialTheme.typography.bodyMedium)
                UsesChoices(toggles)
                // Early Access gate: same as the manual picker, the sign-in button stays disabled
                // until the user confirms they've signed up, so we never open a flow Google blocks.
                GoogleEarlyAccessInfo(googleConfirmed) { googleConfirmed = it }
                // A failed/declined sign-in comes back as externalError; show it so the user isn't
                // left on a silent dead-end and can retry or set up manually.
                InlineError(externalError)
                GoogleSignInButton(
                    enabled = googleConfirmed && !signingInGoogle,
                    signingIn = signingInGoogle,
                    onClick = { onSignInGoogle(recommendation.email, form.uses?.uses) },
                )
            }
            is SetupRecommendation.Jmap -> {
                // Only when there IS a secret field to add something to. With sign-in on offer
                // the field is hidden, and "Just add your password or an API token" would be
                // describing a box that isn't on screen.
                if (!signInOffered) {
                    Text(L10n.setup_detect_found_jmap_note(ctx), style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                }
                recommendation.serverUrl.takeIf { it.isNotBlank() }?.let {
                    Text("JMAP · ${urlHost(it)}", style = MaterialTheme.typography.bodyMedium)
                }
                UntrustedApproval(form.needsApproval, approved) { approved = it }
                if (signInOffered) {
                    Text(
                        L10n.setup_jmap_signin_note(ctx),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    ConnectButton(!signingInJmap, signingInJmap, L10n.setup_jmap_signin_button(ctx)) {
                        onSignInJmap(recommendation.email, recommendation.serverUrl)
                    }
                }
                // When sign-in is on offer the secret field is NOT shown beside it: presenting
                // both at once asks the user to choose between two things that do the same job,
                // and the better one is already a single tap. It appears the moment sign-in
                // actually fails, and "Set up manually" is the other way to reach it, so the
                // manual path is never locked behind a flow this server might not honour.
                val signInFailed = (error ?: externalError) != null
                val showManualSecret = !signInOffered || signInFailed
                if (showManualSecret) {
                    PasswordField(password, { password = it }, L10n.setup_jmap_secret_placeholder(ctx))
                }
                InlineError(error ?: externalError)
                if (showManualSecret) {
                    ConnectButton(form.canConnect && !connecting, connecting, L10n.action_connect(ctx)) {
                        ownFailure = onConnectJmap(form.jmapSetup())
                    }
                }
            }
            is SetupRecommendation.Imap -> {
                // Mail and its servers first, which is what the trust question below is about; the
                // servers follow the mail toggle.
                if (toggles.choosing) {
                    UsesChoices(toggles) { it == AccountCapability.MAIL }
                } else {
                    SectionHeader("✉", L10n.setup_detect_section_email(ctx))
                }
                if (toggles.wantsMail) {
                    ServerRow(recommendation.incoming)
                    recommendation.outgoing?.let { ServerRow(it) }
                }
                UntrustedApproval(form.needsApproval, approved) { approved = it }
                UsesChoices(toggles) { it != AccountCapability.MAIL }
                ImapAuthExplanation(imapAuth)
                if (imapAuth.offersSignIn && onSignInImap != null) {
                    SignInButton(
                        enabled = !signingInImap && (!form.needsApproval || approved),
                        signingIn = signingInImap,
                        label = L10n.setup_imap_signin_button(ctx),
                    ) {
                        if (!toggles.flagMissing()) onSignInImap(imapLoginRequest(recommendation, form.uses))
                    }
                }
                if (imapAuth.showsPassword) {
                    Text(L10n.setup_detect_app_password_hint(ctx), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    PasswordField(password, { password = it }, L10n.setup_field_password(ctx))
                }
                InlineError(error ?: externalError)
                if (imapAuth.showsPassword) {
                    form.refusedCertificate?.let {
                        CertificateExceptionPanel(it, certificateAccepted) { on -> certificateAccepted = on }
                    }
                    ConnectButton(form.canConnect && !connecting, connecting, L10n.action_connect(ctx)) {
                        if (!toggles.flagMissing()) ownFailure = onConnect(form.imapSetup())
                    }
                }
            }
            is SetupRecommendation.Manual -> Unit // never routed here
        }

        TextButton(onClick = onManual, modifier = Modifier.fillMaxWidth()) { Text(L10n.setup_detect_manual(ctx)) }
    }
}


@Composable
private fun ServerRow(row: uniffi.mailcal_bindings.DetectedServerRow) {
    Text(
        text = "${row.protocol} · ${row.hostname}:${row.port} · ${row.security}",
        style = MaterialTheme.typography.bodyMedium,
    )
}

@Composable
private fun UntrustedApproval(needed: Boolean, approved: Boolean, onApprove: (Boolean) -> Unit) {
    if (!needed) return
    val ctx = LocalContext.current
    Text(L10n.setup_detect_untrusted_warning(ctx), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
    Row(verticalAlignment = Alignment.CenterVertically) {
        Checkbox(checked = approved, onCheckedChange = onApprove)
        Text(L10n.setup_detect_trust_confirm(ctx), style = MaterialTheme.typography.bodyMedium)
    }
}

@Composable
private fun InlineError(message: String?) {
    if (message != null) {
        Text(message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
    }
}
