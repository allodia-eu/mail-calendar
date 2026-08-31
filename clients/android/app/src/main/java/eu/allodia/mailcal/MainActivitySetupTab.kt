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

// First run (no account yet) or adding another: collect + validate the
// config in the form, then connect it as a new account via the shared
// addAccount path. A build error returns inline; a failed connect comes
// back via `addError` shown on the form.
@Composable
internal fun MainActivity.AccountSetupTabContent(instance: MailcalApp, ctx: Context) {
    AccountSetupFlow(
                            externalError = addError?.let { L10n.status_connect_failed(ctx, it) },
                            // The first account can't be cancelled (nothing to return to);
                            // adding another can back out to the running app.
                            onCancel = if (needsSetup) {
                                null
                            } else {
                                {
                                    addingAccount = false
                                    addError = null
                                    setupStartEmail = ""
                                    setupStartOffer = null
                                }
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
                                firstRun = needsSetup,
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
                            onSignInMicrosoft = { hint -> signInWithMicrosoft(hint) },
                            onSignInGoogle = { hint -> signInWithGoogle(hint) },
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
                            onConnect = { setup ->
                                try {
                                    val configToml = accountConfigToml(setup)
                                    addAccount(configToml)
                                    null
                                } catch (e: MailcalException) {
                                    e.message ?: L10n.error_invalid_config(ctx)
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
                                    e.message ?: L10n.error_invalid_config(ctx)
                                }
                            },
                            // Documentation screenshots only; null on every real launch.
                            showcaseSeed = ShowcaseMode.setupSeed(this@AccountSetupTabContent),
                        )
}
