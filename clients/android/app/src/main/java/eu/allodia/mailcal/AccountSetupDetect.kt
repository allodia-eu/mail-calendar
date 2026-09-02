// The email-first account-setup flow: the user types only their email, the shared core
// detects their provider's settings, and we route them to a prefilled JMAP / IMAP /
// Microsoft path, falling back to the manual 3-tab form (AccountSetupScreen) with a reason
// when nothing usable is found. The connect-gating logic (including the untrusted-settings
// approval) lives in DetectedConnectForm, a plain class the JVM suite drives without Compose.
package eu.allodia.mailcal

import androidx.activity.compose.BackHandler
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import uniffi.mailcal_bindings.AccountSetup
import uniffi.mailcal_bindings.JmapSetup
import uniffi.mailcal_bindings.MissReason
import uniffi.mailcal_bindings.AllodiaAccountOffer
import uniffi.mailcal_bindings.SetupRecommendation
import uniffi.mailcal_bindings.setupFromOffer


// The email-first flow. [detect] runs the (blocking) core lookup off-main. The IMAP/JMAP/
// Microsoft callbacks match AccountSetupScreen's; [onCancel] backs out when adding another
// account.
@Composable
internal fun AccountSetupFlow(
    externalError: String?,
    onCancel: (() -> Unit)?,
    signingIn: Boolean,
    signingInGoogle: Boolean = false,
    connecting: Boolean,
    detect: suspend (String) -> SetupRecommendation,
    onSignInMicrosoft: (String?) -> Unit,
    onSignInGoogle: (String?) -> Unit = {},
    onConnect: (AccountSetup) -> String?,
    onConnectJmap: (JmapSetup) -> String?,
    // Whether this JMAP server offers discoverable OAuth sign-in, and how to start it. Both are
    // threaded straight through to the detected card and the manual form.
    onCheckJmapSignIn: (suspend (String, String) -> Boolean)? = null,
    // Asks the mail server what it accepts, before any credential field is drawn. Null where
    // there is no core to ask, which is a preview or a test: the card then shows the password
    // form, which is what works everywhere.
    onCheckImapAuth: (suspend (uniffi.mailcal_bindings.ImapLoginRequest) -> uniffi.mailcal_bindings.ImapAuthOffer)? = null,
    // Runs the IMAP browser sign-in.
    onSignInImap: ((uniffi.mailcal_bindings.ImapLoginRequest) -> Unit)? = null,
    signingInImap: Boolean = false,
    onSignInJmap: (String, String) -> Unit = { _, _ -> },
    signingInJmap: Boolean = false,
    // Which account types the manual form's picker shows; threaded straight through.
    offeredKinds: List<AccountKind> = AccountKind.entries,
    // Documentation screenshots only (docs/user-docs.md); null on every real launch.
    showcaseSeed: ShowcaseSetupSeed? = null,
    // The address an account offered by one of the person's other devices is for, filling the
    // field.
    startEmail: String = "",
    // The whole record behind that address, when the flow was opened from an offer. Its route is
    // taken from what the other device wrote down rather than re-derived from the address, the
    // round trip account sync exists to save.
    startOffer: AllodiaAccountOffer? = null,
    // The Allodia onboarding block (docs/onboarding.md). Its card is first-run only; its offers
    // are not.
    allodia: OnboardingAllodia? = null,
) {
    var phase by remember { mutableStateOf<DetectPhase>(DetectPhase.Email) }
    var email by remember { mutableStateOf(startEmail) }
    val scope = rememberCoroutineScope()
    val ctx = LocalContext.current
    // The in-flight detection, held so backing out of it can cancel it (see the handler below).
    var detecting by remember { mutableStateOf<Job?>(null) }

    // Drive the flow to the moment a documentation screenshot pictures. This only types the
    // address and presses the button: which screen it lands on is decided by `detect`, which in a
    // showcase build answers from the core's domain-keyed script. So the capture can never show a
    // card the app would not really produce for that address, including the untrusted-approval
    // gate, whose whole point is that it is not cosmetic.
    //
    // `signInOffered = false`: the JMAP availability probe is a network call, and a showcase build
    // has no network. No showcase domain routes to JMAP, so it is never consulted anyway.
    LaunchedEffect(showcaseSeed) {
        val seed = showcaseSeed ?: return@LaunchedEffect
        email = seed.email
        if (!seed.runDetection) return@LaunchedEffect
        phase = DetectPhase.Detecting
        phase = route(detect(seed.email), signInOffered = false)
    }

    // An offer opened from elsewhere, the Settings list, lands on its own route, the same as one
    // pressed on this screen.
    LaunchedEffect(startOffer) {
        val offer = startOffer ?: return@LaunchedEffect
        phase = route(setupFromOffer(offer), signInOffered = false)
    }

    val goManual: (SetupRecommendation?) -> Unit = { edit ->
        phase = DetectPhase.Manual(reason = null, edit = edit)
    }

    // Back retraces the flow: the routed card, the manual form and the "Looking…" spinner all
    // return to the email question, which returns to the app the user was adding an account from.
    // On a FIRST run there is no such app, nothing is set up yet, so `onCancel` is null, and the
    // handler goes quiet so the press closes the app, as it should on a root screen.
    BackHandler(enabled = phase != DetectPhase.Email || onCancel != null) {
        if (phase == DetectPhase.Email) {
            onCancel?.invoke()
        } else {
            // A lookup left running would finish a second later and yank the user forward onto the
            // very card they just backed out of.
            detecting?.cancel()
            detecting = null
            phase = DetectPhase.Email
        }
    }

    when (val current = phase) {
        DetectPhase.Email -> Column(
            modifier = Modifier.fillMaxSize().systemBarsPadding().verticalScroll(rememberScrollState()).padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(L10n.setup_detect_title(ctx), style = MaterialTheme.typography.headlineSmall)
            // The recommendation, the way back for someone who already has an account, and the
            // divider that names what follows, above the address field, in that order
            // (docs/onboarding.md). Nothing at all when this build carries no registration. On a
            // later add the card is gone and the offers are not: `firstRun` tells the two apart.
            allodia?.let { state ->
                OnboardingAllodiaCard(
                    offered = state.offered,
                    signingIn = state.signingIn,
                    signedIn = state.signedIn,
                    offers = state.offers,
                    checking = state.checking,
                    onCreate = state.onCreate,
                    onSignIn = state.onSignIn,
                    // The record's own route, not one re-derived from the address: that round
                    // trip is exactly what syncing an account list exists to save, and for a
                    // domain publishing no autoconfig it would find less.
                    onSetUp = { offer ->
                        email = offer.email
                        phase = route(setupFromOffer(offer), signInOffered = false)
                    },
                    escapable = state.escapable,
                    onCancelSignIn = state.onCancelSignIn,
                    firstRun = state.firstRun,
                )
            }
            Text(
                L10n.setup_detect_description(ctx),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            SetupField(email, { email = it }, L10n.setup_field_email(ctx), L10n.setup_detect_email_placeholder(ctx), KeyboardType.Email)
            Button(
                onClick = {
                    detecting = scope.launch {
                        phase = DetectPhase.Detecting
                        val recommendation = detect(email)
                        // Ask whether this server offers sign-in *here*, while the "Looking…"
                        // spinner is already up, rather than after the card is on screen. Both
                        // calls are network round trips; running the second one under the same
                        // spinner costs a moment of waiting the user is already doing, and buys a
                        // card that never changes shape once it appears.
                        val signInOffered = onCheckJmapSignIn
                            ?.takeIf { recommendation is SetupRecommendation.Jmap }
                            ?.let { check ->
                                val jmap = recommendation as SetupRecommendation.Jmap
                                check(jmap.email, jmap.serverUrl)
                            } ?: false
                        // The IMAP half of the same question, asked under the same spinner and
                        // for the same reason: a card that decides what to ask for after it is
                        // on screen changes shape under the person reading it.
                        val imapAuth = onCheckImapAuth
                            ?.takeIf { recommendation is SetupRecommendation.Imap }
                            ?.let { check ->
                                val imap = recommendation as SetupRecommendation.Imap
                                ImapAuthState.of(check(imapLoginRequest(imap)))
                            } ?: ImapAuthState.Password
                        phase = route(recommendation, signInOffered, imapAuth)
                    }
                },
                enabled = email.isNotBlank(),
                modifier = Modifier.fillMaxWidth(),
            ) { Text(L10n.setup_detect_action(ctx)) }
            TextButton(onClick = { goManual(null) }, modifier = Modifier.fillMaxWidth()) {
                Text(L10n.setup_detect_manual(ctx))
            }
            if (onCancel != null) {
                TextButton(onClick = onCancel, modifier = Modifier.fillMaxWidth()) { Text(L10n.action_cancel(ctx)) }
            }
        }

        DetectPhase.Detecting -> Column(
            modifier = Modifier.fillMaxSize().systemBarsPadding().padding(24.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            CircularProgressIndicator()
            Text(L10n.setup_detect_looking(ctx), style = MaterialTheme.typography.bodyMedium)
        }

        is DetectPhase.Found -> FoundView(
            recommendation = current.recommendation,
            signInOffered = current.signInOffered,
            imapAuth = current.imapAuth,
            onSignInImap = onSignInImap,
            signingInImap = signingInImap,
            connecting = connecting,
            signingIn = signingIn,
            signingInGoogle = signingInGoogle,
            externalError = externalError,
            onSignInMicrosoft = onSignInMicrosoft,
            onSignInGoogle = onSignInGoogle,
            onConnect = onConnect,
            onConnectJmap = onConnectJmap,
            onSignInJmap = onSignInJmap,
            signingInJmap = signingInJmap,
            onManual = { goManual(current.recommendation) },
        )

        is DetectPhase.Manual -> {
            val prefill = manualPrefill(current.edit)
            AccountSetupScreen(
                externalError = externalError,
                onCancel = onCancel,
                signingIn = signingIn,
                signingInGoogle = signingInGoogle,
                connecting = connecting,
                onSignInMicrosoft = onSignInMicrosoft,
                onSignInGoogle = onSignInGoogle,
                onConnect = onConnect,
                onConnectJmap = onConnectJmap,
                onCheckJmapSignIn = onCheckJmapSignIn,
                onSignInJmap = onSignInJmap,
                signingInJmap = signingInJmap,
                onCheckImapAuth = onCheckImapAuth,
                onSignInImap = onSignInImap,
                signingInImap = signingInImap,
                initialKind = prefill.kind,
                offeredKinds = offeredKinds,
                prefillEmail = email.ifBlank { prefill.email },
                prefillImapHost = prefill.imapHost,
                prefillSmtpHost = prefill.smtpHost,
                prefillJmapServer = prefill.jmapServer,
                note = current.reason?.let { reasonNote(it, ctx) },
            )
        }
    }
}

// The routed card for a JMAP/IMAP/Microsoft hit.
@Composable
private fun FoundView(
    recommendation: SetupRecommendation,
    connecting: Boolean,
    signingIn: Boolean,
    signingInGoogle: Boolean,
    externalError: String?,
    onSignInMicrosoft: (String?) -> Unit,
    onSignInGoogle: (String?) -> Unit,
    onConnect: (AccountSetup) -> String?,
    onConnectJmap: (JmapSetup) -> String?,
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
    // Calendar defaults ON when a CalDAV endpoint was discovered (opt-out), OFF otherwise (opt-in).
    var calendarEnabled by remember(recommendation) {
        mutableStateOf((recommendation as? SetupRecommendation.Imap)?.caldavUrl != null)
    }
    var calendarUrl by remember(recommendation) { mutableStateOf("") }
    var error by remember(recommendation) { mutableStateOf<String?>(null) }
    form.password = password
    form.approved = approved
    form.calendarEnabled = calendarEnabled
    form.calendarUrlEntry = calendarUrl

    Column(
        modifier = Modifier.fillMaxSize().systemBarsPadding().verticalScroll(rememberScrollState()).padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(L10n.setup_detect_found_title(ctx), style = MaterialTheme.typography.headlineSmall)

        when (recommendation) {
            is SetupRecommendation.Microsoft -> {
                Text(L10n.setup_detect_microsoft_hint(ctx), style = MaterialTheme.typography.bodyMedium)
                // A failed/declined sign-in comes back as externalError; show it so the user
                // isn't left on a silent dead-end and can retry or set up manually.
                InlineError(externalError)
                Button(
                    onClick = { onSignInMicrosoft(recommendation.email) },
                    enabled = !signingIn,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    if (signingIn) CircularProgressIndicator(modifier = Modifier.size(18.dp)) else Text(L10n.setup_microsoft_signin(ctx))
                }
            }
            is SetupRecommendation.Google -> {
                Text(L10n.setup_detect_google_hint(ctx), style = MaterialTheme.typography.bodyMedium)
                // Early Access gate: same as the manual picker, the sign-in button stays disabled
                // until the user confirms they've signed up, so we never open a flow Google blocks.
                GoogleEarlyAccessInfo(googleConfirmed) { googleConfirmed = it }
                // A failed/declined sign-in comes back as externalError; show it so the user isn't
                // left on a silent dead-end and can retry or set up manually.
                InlineError(externalError)
                GoogleSignInButton(
                    enabled = googleConfirmed && !signingInGoogle,
                    signingIn = signingInGoogle,
                    onClick = { onSignInGoogle(recommendation.email) },
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
                        error = onConnectJmap(form.jmapSetup())
                    }
                }
            }
            is SetupRecommendation.Imap -> {
                SectionHeader("✉", L10n.setup_detect_section_email(ctx))
                ServerRow(recommendation.incoming)
                recommendation.outgoing?.let { ServerRow(it) }
                ImapAuthExplanation(imapAuth)
                UntrustedApproval(form.needsApproval, approved) { approved = it }
                if (imapAuth.offersSignIn && onSignInImap != null) {
                    SignInButton(
                        enabled = !signingInImap && (!form.needsApproval || approved),
                        signingIn = signingInImap,
                        label = L10n.setup_imap_signin_button(ctx),
                    ) { onSignInImap(imapLoginRequest(recommendation, form.effectiveCaldavUrl)) }
                }
                if (imapAuth.showsPassword) {
                    Text(L10n.setup_detect_app_password_hint(ctx), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
                    PasswordField(password, { password = it }, L10n.setup_field_password(ctx))
                }
                CalendarSection(
                    discovered = recommendation.caldavUrl,
                    enabled = calendarEnabled,
                    onEnabledChange = { calendarEnabled = it },
                    url = calendarUrl,
                    onUrlChange = { calendarUrl = it },
                )
                InlineError(error ?: externalError)
                if (imapAuth.showsPassword) {
                    ConnectButton(form.canConnect && !connecting, connecting, L10n.action_connect(ctx)) {
                        error = onConnect(form.imapSetup())
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

// A small section header, e.g. "✉  Email" / "📅  Calendar", grouping the found card.
@Composable
private fun SectionHeader(icon: String, label: String) {
    Text("$icon $label", style = MaterialTheme.typography.titleSmall)
}

// The Calendar section of the found card. When detection discovered a CalDAV endpoint the
// toggle is pre-checked (opt-out) and its host is shown; otherwise it's an opt-in toggle that
// reveals a manual CalDAV field. Calendar reuses the IMAP credentials at connect.
@Composable
private fun CalendarSection(
    discovered: String?,
    enabled: Boolean,
    onEnabledChange: (Boolean) -> Unit,
    url: String,
    onUrlChange: (String) -> Unit,
) {
    val ctx = LocalContext.current
    SectionHeader("📅", L10n.setup_detect_section_calendar(ctx))
    val label = if (discovered != null) L10n.setup_detect_calendar_enable(ctx) else L10n.setup_detect_calendar_add(ctx)
    Row(verticalAlignment = Alignment.CenterVertically) {
        Checkbox(checked = enabled, onCheckedChange = onEnabledChange)
        Text(label, style = MaterialTheme.typography.bodyMedium)
    }
    if (enabled) {
        if (discovered != null) {
            Text(urlHost(discovered), style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        } else {
            SetupField(url, onUrlChange, L10n.setup_field_caldav(ctx), L10n.setup_hint_caldav(ctx))
        }
    }
}

// The host of a discovered URL (CalDAV endpoint, JMAP base), for a compact confirmation line:
// so an untrusted result's "check the server names" has a name to check; the full URL is the
// fallback if it somehow doesn't parse.
private fun urlHost(url: String): String = runCatching { java.net.URI(url).host }.getOrNull() ?: url

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
