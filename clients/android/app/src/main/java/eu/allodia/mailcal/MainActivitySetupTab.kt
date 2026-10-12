// The add-account branch of MainScreen, split out of MainActivity.kt: first-run setup and adding
// another account both land on the same AccountSetupFlow, wired to the email-first detection, the
// three OAuth sign-in routes and the manual IMAP/JMAP forms.
package eu.allodia.mailcal

import android.content.Context
import androidx.compose.runtime.Composable
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.MailcalException
import uniffi.mailcal_bindings.accountConfigToml
import uniffi.mailcal_bindings.jmapAccountConfigToml
import uniffi.mailcal_bindings.oauthRoutes
import uniffi.mailcal_bindings.providerSetupChoices

// What the form should say about a connect that did not succeed. A refused certificate comes
// back whole, because the form offers to accept it; everything else is a message
// (docs/certificate-exceptions.md). The generated message of a refusal dumps the certificate
// into one line, so it is deliberately not used for that case.
internal fun connectFailure(error: MailcalException, ctx: Context): ConnectFailure =
    when (error) {
        is MailcalException.CertificateRejected ->
            ConnectFailure(error.reason, error.certificate)
        else -> ConnectFailure(coreErrorText(error) ?: L10n.error_invalid_config(ctx))
    }

// First run (no account yet) or adding another: collect + validate the
// config in the form, then connect it as a new account via the shared
// addAccount path. A build error returns inline; a failed connect comes
// back via `addError` shown on the form.
@Composable
internal fun MainActivity.AccountSetupTabContent(instance: MailcalApp, ctx: Context) {
    // A connect ends on the uses its server offers, then on the link step, ahead of the form
    // (MainActivitySetupFlow.kt).
    setupFlow.uses?.let { step ->
        SetupUsesStepScreen(step) { chooseSetupUses(it) }
        return
    }
    val links = setupFlow.links
    if (setupFlow.showsLinks && links != null) {
        SetupLinkStepScreen(
            step = links,
            onPick = { picker, option -> setupFlow.pick(picker, option) },
            onAddAnother = { addLinkedAccount() },
            onBeside = { setUpBeside() },
            onDone = { link -> finishLinkStep(link) },
        )
        return
    }
    // A new form for each account the flow sets up, from the address.
    androidx.compose.runtime.key(setupFlow.form) { SetupForm(instance, ctx) }
}

@Composable
private fun MainActivity.SetupForm(instance: MailcalApp, ctx: Context) {
    AccountSetupFlow(
                            externalError = addError?.let { L10n.status_connect_failed(ctx, it) },
                            externalFailure = addFailure,
                            // The first account can't be cancelled (nothing to return to);
                            // adding another can back out to the running app, or to the link
                            // step it was added from.
                            onCancel = if (needsSetup && setupFlow.returningTo == null) {
                                null
                            } else {
                                { cancelSetup() }
                            },
                            // The running app answers from the core, so the manual form's port
                            // box and the address the core dials are the same number.
                            standardPort = { kind, security ->
                                uniffi.mailcal_bindings.standardPort(kind, security).toInt()
                            },
                            startEmail = setupStartEmail,
                            startOffer = setupStartOffer,
                            // The recommendation is first-run only; the offers are not, because
                            // somebody who set up one of three linked accounts still has two to
                            // go (docs/onboarding.md).
                            allodia = OnboardingAllodia(
                                offered = allodiaSignInOffered(),
                                signingIn = signingInAllodia,
                                escapable = allodiaSignInSlow,
                                signedIn = allodiaAccount != null,
                                checking = allodiaSync.checking,
                                offers = allodiaSync.report?.offers,
                                onCreate = { registerAllodiaAccount() },
                                onSignIn = { signInToAllodia() },
                                onCancelSignIn = { cancelAllodiaSignIn() },
                                // Another account added from the link step is not the first.
                                firstRun = needsSetup && !setupFlow.addedAny,
                            ),
                            signingIn = signingInMicrosoft,
                            signingInGoogle = signingInGoogle,
                            // Busy while the browser is open OR while the returned code is
                            // being exchanged, the user should never see an idle button during
                            // either.
                            signingInJmap = signingInJmap || completingJmap,
                            connecting = isConnecting,
                            // The email-first lookup, run off-main against the shared core.
                            detect = { email -> detectAccount(email) },
                            onSignInMicrosoft = { hint, uses -> signInWithMicrosoft(hint, uses) },
                            onSignInGoogle = { hint, uses -> signInWithGoogle(hint, uses) },
                            // What a provider sign-in offers for the address typed on the form.
                            providerChoices = { kind, email -> providerSetupChoices(kind, email) },
                            // The servers found beside a JMAP server, offered once it is added.
                            onRoute = { email, setup -> setupFlow.found = FoundBeside.of(email, setup) },
                            startOn = setupFlow.startOn,
                            // Only the browser sign-ins this build carries a registration for.
                            // Asked here rather than in the form, which has to render without the
                            // core for the JVM suite.
                            offeredKinds = AccountKind.offered(oauthRoutes()),
                            // The blocking availability probe, hopped off the main thread by the
                            // form's debounced effect; and the browser sign-in it gates.
                            onCheckJmapSignIn = { email, server ->
                                withContext(Dispatchers.IO) { jmapSignInAvailable(email, server) }
                            },
                            onSignInJmap = { email, server -> signInWithJmap(email, server) },
                            // The same pair for a mail account: what the server accepts, hopped
                            // off the main thread by the caller's coroutine, and the browser
                            // sign-in it gates.
                            onCheckImapAuth = { request ->
                                withContext(Dispatchers.IO) { imapAuthOptions(request) }
                            },
                            onSignInImap = { request -> signInWithImap(request) },
                            signingInImap = signingInImap,
                            onConnect = { setup ->
                                try {
                                    // Accepted once, carried for the rest of this setup: a retry
                                    // that then fails on the password must not ask again.
                                    setup.acceptedCertificate?.let { setupAcceptedCertificate = it }
                                    val carried = setup.copy(
                                        acceptedCertificate = setup.acceptedCertificate
                                            ?: setupAcceptedCertificate,
                                    )
                                    val configToml = accountConfigToml(carried)
                                    addAccount(configToml)
                                    null
                                } catch (e: MailcalException) {
                                    connectFailure(e, ctx)
                                }
                            },
                            // JMAP mirrors the IMAP path, a different config builder, the same
                            // shared addAccount (off-main connect + persist-after-connect). No
                            // browser flow: JMAP is HTTP Basic/bearer, unlike Microsoft.
                            onConnectJmap = { setup ->
                                try {
                                    val configToml = jmapAccountConfigToml(setup)
                                    addAccount(configToml)
                                    null
                                } catch (e: MailcalException) {
                                    // A JMAP account's stored config carries no exception, so a
                                    // refusal here is reported and not offered (rule 8).
                                    connectFailure(e, ctx).copy(certificate = null)
                                }
                            },
                            // Documentation screenshots only; null on every real launch.
                            showcaseSeed = ShowcaseMode.setupSeed(this@SetupForm),
                        )
}
