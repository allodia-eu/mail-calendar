// The Settings branch of MainScreen, split out of MainActivity.kt: the unified Settings screen's
// wiring for every category (general, calendar, reading, composing, signatures, privacy, accounts,
// the Allodia account and cross-device sync, diagnostics, advanced).
package eu.allodia.mailcal

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import uniffi.mailcal_bindings.AboutPlatform
import uniffi.mailcal_bindings.AllodiaAccountSyncMode
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.aboutInfo

// The unified Settings screen, swapped in over the mailbox.
@Composable
internal fun MainActivity.SettingsTabContent(instance: MailcalApp) {
    SettingsScreen(
                            about = aboutInfo(AboutPlatform.ANDROID),
                            // General, time zone.
                            timeZone = timeZone,
                            onSetTimeZone = { instance.dispatch(Intent.SetTimeZone(it)) },
                            // General + Calendar, the display preferences the core owns. Each
                            // setter persists in the core and signals both SETTINGS and CALENDAR,
                            // so the grid re-pulls rather than keeping the old week on screen.
                            display = displaySettings,
                            onSetTimeFormat = { instance.setTimeFormat(it) },
                            // The one display setting the core signals SETTINGS alone for, it
                            // computes nothing from the appearance, so repainting is ours.
                            onSetAppearance = {
                                instance.setAppearance(it)
                                appearance = it
                            },
                            onSetWeekStart = { instance.setWeekStart(it) },
                            onSetVisibleHours = { instance.setCalendarVisibleHours(it.toUByte()) },
                            // Calendar, which calendar a new event is filed on, and so the colour
                            // a slot drawn on the grid wears. Read straight from the core (an
                            // in-memory cache read) rather than held in state: the signal below
                            // recomposes Settings, and the rows carry the resolved `isDefault`.
                            calendars = remember(calendarVersion) { instance.calendars() },
                            onSetDefaultCalendar = { account, calendar ->
                                instance.setDefaultCalendar(account, calendar)
                            },
                            // Reading, conversation grouping.
                            mode = mode,
                            onSetMode = { instance.dispatch(Intent.SetViewMode(it)) },
                            // Composing, quote style (+ the per-message opt-in), default send
                            // account, swipe actions.
                            quoteSettings = quoteSettings,
                            onSetQuoteStyle = { style ->
                                instance.setQuoteStyle(style)
                                quoteSettings = quoteSettings.copy(style = style)
                            },
                            onSetQuoteStylePerMessage = { perMessage ->
                                instance.setQuoteStylePerMessage(perMessage)
                                quoteSettings = quoteSettings.copy(perMessage = perMessage)
                            },
                            accounts = accounts,
                            defaultSendAccount = defaultSendAccount,
                            onSetDefaultSendAccount = { account ->
                                instance.setDefaultSendAccount(account)
                                defaultSendAccount = account
                            },
                            swipe = swipeSettings,
                            onSetSwipeLeft = { action ->
                                instance.setSwipeAction(uniffi.mailcal_bindings.SwipeDirection.LEFT, action)
                                swipeSettings = swipeSettings.copy(left = action)
                            },
                            onSetSwipeRight = { action ->
                                instance.setSwipeAction(uniffi.mailcal_bindings.SwipeDirection.RIGHT, action)
                                swipeSettings = swipeSettings.copy(right = action)
                            },
                            // Signatures, the library and the per-account slots. Every setter
                            // persists in the core and re-signals SETTINGS, which is what refreshes
                            // the snapshot above; nothing is echoed locally.
                            signatures = signatures,
                            signatureHtml = { instance.signatureHtml(it) },
                            onCreateSignature = { name, html, plain ->
                                instance.createSignature(name, html, plain)
                            },
                            onUpdateSignature = { id, name, html, plain ->
                                instance.updateSignature(id, name, html, plain)
                            },
                            onDeleteSignature = { instance.deleteSignature(it) },
                            onSetAccountSignature = { account, slot, signature ->
                                instance.setAccountSignature(account, slot, signature)
                            },
                            // Privacy, withdraw (or belatedly give) the usage-statistics consent.
                            // Echoed locally on write: the core raises no Settings surface for it,
                            // so the switch would otherwise snap back until the next launch.
                            analyticsEnabled = analyticsConsent?.enabled == true,
                            onSetAnalytics = { share ->
                                instance.setAnalyticsConsent(share)
                                analyticsConsent = instance.analyticsConsent()
                            },
                            analyticsPayloadPreview = { instance.analyticsPayloadPreview() },
                            // Accounts, the Allodia account, then per-account fetch depth + sync
                            // behaviour. The Allodia state is the activity's: a sign-in leaves for
                            // the browser and returns through onNewIntent, so nothing this screen
                            // held would still be there.
                            allodia = AllodiaSettings(
                                available = allodiaSignInOffered(),
                                account = allodiaAccount,
                                signingIn = signingInAllodia,
                                failure = allodiaFailure,
                            ),
                            onAllodiaSignIn = { signInToAllodia() },
                            onAllodiaCreate = { registerAllodiaAccount() },
                            onAllodiaManage = { manageAllodiaAccount() },
                            onAllodiaSignOut = { signOutOfAllodia() },
                            // Accounts, what the person's other devices have to say, drawn above
                            // their own accounts because an offer becomes one of them.
                            allodiaSync = allodiaSync,
                            onAllodiaSetUp = { offer ->
                                setupStartEmail = offer.email
                                setupStartOffer = offer
                                addingAccount = true
                            },
                            // "Keep what I have" is Paused: the other devices keep the account,
                            // and this one stops exchanging changes about it, which is exactly
                            // what the question asked.
                            onAllodiaKeepLocal = { accountId ->
                                setAllodiaAccountSyncMode(
                                    accountId,
                                    AllodiaAccountSyncMode.PAUSED,
                                )
                            },
                            // How each account is shared, and the control that changes it. Absent
                            // in a build with no Allodia sign-in, which draws nothing at all.
                            accountsSyncMode =
                                if (allodiaSignInOffered()) accountsSyncMode else emptyMap(),
                            onSetAccountSyncMode = { accountId, mode ->
                                setAllodiaAccountSyncMode(accountId, mode)
                            },
                            // Accounts, per-account fetch depth + sync behaviour.
                            settings = syncSettings,
                            onSetSyncDepth = { account, months ->
                                instance.setAccountSyncDepth(account, months)
                            },
                            onSetMessageSize = { account, megabytes ->
                                instance.setAccountMessageSizeLimit(account, megabytes)
                            },
                            onSetStrategy = { account, strategy ->
                                instance.setSyncStrategy(account, strategy)
                            },
                            onSetPollInterval = { account, minutes ->
                                instance.setPollInterval(account, minutes)
                            },
                            onSetPushFolder = { account, folder, subscribed ->
                                instance.setPushFolder(account, folder, subscribed)
                            },
                            // Diagnostics, the log viewer/share + debug-detail screen.
                            onOpenDiagnostics = { showingDiagnostics = true },
                            // Advanced, reset the local cache.
                            onReset = { instance.reset() },
                            onBack = {
                                showingSettings = false
                                settingsCategory = null
                            },
                            initialCategory = settingsCategory,
                        )
}
