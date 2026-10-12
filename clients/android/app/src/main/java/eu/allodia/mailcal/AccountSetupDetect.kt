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
import uniffi.mailcal_bindings.ConnectionSecurity
import uniffi.mailcal_bindings.JmapSetup
import uniffi.mailcal_bindings.MailServerKind
import uniffi.mailcal_bindings.MissReason
import uniffi.mailcal_bindings.AllodiaAccountOffer
import uniffi.mailcal_bindings.SetupChoice
import uniffi.mailcal_bindings.SetupRecommendation
import uniffi.mailcal_bindings.offeredSetup
import uniffi.mailcal_bindings.AccountCapability
import uniffi.mailcal_bindings.AccountKind as CoreAccountKind
import uniffi.mailcal_bindings.DetectedSetup


// The email-first flow. [detect] runs the (blocking) core lookup off-main. The IMAP/JMAP/
// Microsoft callbacks match AccountSetupScreen's; [onCancel] backs out when adding another
// account.
@Composable
internal fun AccountSetupFlow(
    externalError: String?,
    externalFailure: ConnectFailure? = null,
    onCancel: (() -> Unit)?,
    signingIn: Boolean,
    signingInGoogle: Boolean = false,
    connecting: Boolean,
    detect: suspend (String) -> DetectedSetup,
    onSignInMicrosoft: (String?, List<AccountCapability>?) -> Unit,
    onSignInGoogle: (String?, List<AccountCapability>?) -> Unit = { _, _ -> },
    onConnect: (AccountSetup) -> ConnectFailure?,
    onConnectJmap: (JmapSetup) -> ConnectFailure?,
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
    // The core's standard port for a server kind and security, threaded through to the manual
    // form. Null suggests no port, which is what a preview and a JVM test get.
    standardPort: ((MailServerKind, ConnectionSecurity) -> Int)? = null,
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
    // The uses a provider sign-in offers for an address, for the manual form; null in a test.
    providerChoices: ((CoreAccountKind, String) -> List<SetupChoice>)? = null,
    // Told which route a detection took, for the address it was run for: the servers found beside
    // a JMAP server are offered once its account is added.
    onRoute: (String, DetectedSetup) -> Unit = { _, _ -> },
    // A route to open on rather than the address: the servers found beside an account's own.
    startOn: Pair<String, DetectedSetup>? = null,
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
        phase = route(detect(seed.email), seed.email, signInOffered = false)
    }

    LaunchedEffect(startOn) {
        val (address, setup) = startOn ?: return@LaunchedEffect
        email = address
        phase = route(setup, address, signInOffered = false)
    }

    // An offer opened from elsewhere, the Settings list, lands on its own route, the same as one
    // pressed on this screen.
    LaunchedEffect(startOffer) {
        val offer = startOffer ?: return@LaunchedEffect
        val setup = offeredSetup(offer)
        onRoute(offer.email, setup)
        phase = route(setup, offer.email, signInOffered = false)
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
                        val setup = offeredSetup(offer)
                        onRoute(offer.email, setup)
                        phase = route(setup, offer.email, signInOffered = false)
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
                        val detected = detect(email)
                        val recommendation = detected.recommendation
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
                        onRoute(email.trim(), detected)
                        phase = route(detected, email, signInOffered, imapAuth)
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
            offer = current.offer,
            signInOffered = current.signInOffered,
            imapAuth = current.imapAuth,
            onSignInImap = onSignInImap,
            signingInImap = signingInImap,
            connecting = connecting,
            signingIn = signingIn,
            signingInGoogle = signingInGoogle,
            externalError = externalError,
            externalFailure = externalFailure,
            onSignInMicrosoft = onSignInMicrosoft,
            onSignInGoogle = onSignInGoogle,
            onConnect = onConnect,
            onConnectJmap = onConnectJmap,
            onSignInJmap = onSignInJmap,
            signingInJmap = signingInJmap,
            onManual = { goManual(current.recommendation) },
        )

        is DetectPhase.Dav -> DavFoundCard(
            email = current.email,
            offer = current.offer,
            connecting = connecting,
            externalError = externalError,
            externalFailure = externalFailure,
            onConnect = onConnect,
            onManual = { phase = DetectPhase.Manual(reason = null, edit = null, dav = current.offer) },
        )

        is DetectPhase.Manual -> {
            val prefill = manualPrefill(current.edit, current.dav)
            AccountSetupScreen(
                externalError = externalError,
                externalFailure = externalFailure,
                onCancel = onCancel,
                signingIn = signingIn,
                signingInGoogle = signingInGoogle,
                connecting = connecting,
                onSignInMicrosoft = onSignInMicrosoft,
                onSignInGoogle = onSignInGoogle,
                onConnect = onConnect,
                onConnectJmap = onConnectJmap,
                standardPort = standardPort,
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
                prefillCaldav = prefill.caldavUrl,
                prefillCarddav = prefill.carddavUrl,
                note = current.reason?.let { reasonNote(it, ctx) },
                providerChoices = providerChoices,
            )
        }
    }
}
