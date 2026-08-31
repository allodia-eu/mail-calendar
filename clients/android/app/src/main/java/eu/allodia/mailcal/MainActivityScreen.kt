// The Compose tree onCreate hands to setContent, split out of MainActivity.kt: which top-level
// screen is showing (welcome, reading, add-account, diagnostics, settings, or the running app's
// mail/calendar/contacts tabs) and the two standing prompts (an unfiled copy, an invitation reply)
// that can be raised from any of them.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.TimeFormat

// [showcase] is whether this is a screenshot (documentation) launch, decided once in
// MainActivityBoot.kt's prepareBoot() before setContent runs.
@Composable
internal fun MainActivity.MainScreen(showcase: Boolean) {
            AppTheme(appearance = appearance) {
              // Mail and calendar render times through the same ambient setting, so the app can
              // never disagree with itself about whether it is 14:05 or 2:05 PM.
              androidx.compose.runtime.CompositionLocalProvider(
                  LocalUse24Hour provides (displaySettings.timeFormat == TimeFormat.TWENTY_FOUR_HOUR),
              ) {
                // Ask for notification permission (Android 13+) only once at least one account
                // exists, a returning user is asked on this launch, a first-time user right after
                // their first account connects (needsSetup → false). Prompting on the empty setup
                // screen is premature. Idempotent + re-checks at post time, so a denial never crashes.
                //
                // It also waits for the usage-statistics question to be settled, so the two asks
                // never stack. That is not just about a first run: a *returning* user upgrading
                // into this version already has accounts (needsSetup is false at launch), so
                // without this guard the system dialog would open straight on top of the welcome
                // screen.
                //
                // A showcase run never asks: it reports needsSetup = false (it seeds its own
                // accounts), and on a device that hasn't granted the permission yet the system
                // dialog would land right on top of the screenshot being taken.
                LaunchedEffect(needsSetup, analyticsConsent?.asked) {
                    if (!needsSetup && !showcase && analyticsConsent?.asked == true) {
                        maybeRequestNotificationPermission()
                    }
                }
                Surface(modifier = Modifier.fillMaxSize()) {
                    val ctx = LocalContext.current
                    val instance = app
                    val opened = openedMessage
                    val error = connectError
                    when {
                        // First boot: welcome the user and ask the one question, before setup and
                        // before anything else. The gate is the core's, not the UI's, `asked`
                        // also covers a returning user upgrading into this version, who has
                        // accounts already but has never been asked. The showcase and the demo
                        // report it settled (they have no store to record an answer in), so no
                        // screenshot run ever sees this.
                        instance != null && analyticsConsent?.asked == false -> WelcomeScreen(
                            payloadPreview = { instance.analyticsPayloadPreview() },
                            onGetStarted = { share ->
                                instance.setAnalyticsConsent(share)
                                analyticsConsent = instance.analyticsConsent()
                            },
                        )
                        // A message is open: show its reading view over the list.
                        instance != null && opened != null -> ReadingTabContent(instance, opened)
                        // First run (no account yet) or adding another: collect + validate the
                        // config in the form, then connect it as a new account via the shared
                        // addAccount path. A build error returns inline; a failed connect comes
                        // back via `addError` shown on the form.
                        instance != null && (needsSetup || addingAccount) ->
                            AccountSetupTabContent(instance, ctx)
                        // The Diagnostics screen, checked BEFORE Settings so it wins while both
                        // flags are true. The debug toggle raises/lowers the LIVE core's log
                        // ceiling; the persisted choice is applied at the next boot (connect /
                        // showcase / background worker) via DiagnosticsPrefs.bootLogLevel.
                        instance != null && showingDiagnostics -> DiagnosticsScreen(
                            onSetLogLevel = { instance.setLogLevel(it) },
                            onBack = { showingDiagnostics = false },
                        )
                        // The unified Settings screen, swapped in over the mailbox.
                        instance != null && showingSettings -> SettingsTabContent(instance)
                        instance != null -> MailboxTabContent(instance)
                        error != null -> ConnectionStatus(L10n.status_connect_failed(ctx, error), isError = true)
                        else -> ConnectionStatus(L10n.status_connecting(ctx), isError = false)
                    }
                    // The message went out; its copy did not reach Sent. Rendered here for the
                    // same reason as the question below: the send may have been started from
                    // anywhere, and the question outlives the screen that raised it.
                    UnfiledCopyPrompt(
                        unfiledCopy,
                        onRetry = { instance?.dispatch(Intent.RetryUnfiledCopy) },
                        onDismiss = { instance?.dispatch(Intent.DismissUnfiledCopy) },
                    )
                    // The calendar server stored the answer and then reported it could not tell
                    // the organiser. Rendered outside the screen switch above because an
                    // invitation can be answered from the reading view *or* from the list, and
                    // the question outlives whichever one raised it, the user may well have
                    // navigated on before the server's verdict came back.
                    //
                    // The reply's subject is composed here, not in the core, for the same reason
                    // the RSVP's is: it is copy a stranger reads in their inbox, and the core
                    // carries no locale.
                    InvitationReplyPrompt(replyPrompt) { send, remember ->
                        instance?.dispatch(
                            Intent.AnswerReplyPrompt(
                                send,
                                remember,
                                replyPrompt?.let {
                                    invitationReplySubject(ctx, it.response, it.summary)
                                },
                            ),
                        )
                    }
                }
              }
            }
        }
