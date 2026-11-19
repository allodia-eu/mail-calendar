// The Settings branch of MainScreen, split out of MainActivity.kt: the unified Settings screen's
// wiring for every category (general, calendar, reading, composing, signatures, privacy, accounts,
// the Allodia account and cross-device sync, diagnostics, advanced).
package eu.allodia.mailcal

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import uniffi.mailcal_bindings.AboutPlatform
import uniffi.mailcal_bindings.Intent
import uniffi.mailcal_bindings.MailcalApp
import uniffi.mailcal_bindings.PreferenceIntent
import uniffi.mailcal_bindings.SwipeDirection
import uniffi.mailcal_bindings.aboutInfo
import uniffi.mailcal_bindings.signatureImageRefusal
import uniffi.mailcal_bindings.signatureNameRefusal
import uniffi.mailcal_bindings.standardPort

/**
 * Changes one preference. It takes effect in the core before this returns and raises SETTINGS,
 * whose pull redraws every screen that shows it, so no caller keeps a copy of the value.
 */
internal fun MailcalApp.setPreference(intent: PreferenceIntent) = dispatch(Intent.Preferences(intent))

// The unified Settings screen, swapped in over the mailbox.
@Composable
internal fun MainActivity.SettingsTabContent(instance: MailcalApp) {
    // All five are pulled in the same step that sets the core, so none is null here.
    val display = displaySettings ?: return
    val quotes = quoteSettings ?: return
    val swipes = swipeSettings ?: return
    val sendAccount = sendAccountSetting ?: return
    val advanced = advancedPage ?: return
    SettingsScreen(
                            about = aboutInfo(AboutPlatform.ANDROID),
                            // General, time zone.
                            timeZone = timeZone,
                            onSetTimeZone = { instance.dispatch(Intent.SetTimeZone(it)) },
                            // General + Calendar, the display preferences the core owns. The
                            // clock and the week start signal CALENDAR besides, so the grid
                            // re-pulls rather than keeping the old week on screen.
                            display = display,
                            onSetTimeFormat = { instance.setPreference(PreferenceIntent.SetTimeFormat(it)) },
                            onSetAppearance = { instance.setPreference(PreferenceIntent.SetAppearance(it)) },
                            onSetWeekStart = { instance.setPreference(PreferenceIntent.SetWeekStart(it)) },
                            onSetVisibleHours = { instance.setPreference(PreferenceIntent.SetCalendarVisibleHours(it.toUByte())) },
                            // Calendar, which calendar a new event is filed on, and so the colour
                            // a slot drawn on the grid wears. Read from the core's memory again on
                            // CALENDAR and SETTINGS; the rows carry the resolved `isDefault`.
                            newEventCalendars = remember(calendarVersion, settingsVersion) {
                                instance.newEventCalendars()
                            },
                            onSetDefaultCalendar = { account, calendar ->
                                instance.setPreference(PreferenceIntent.SetDefaultCalendar(account, calendar))
                            },
                            // Videoconferencing, the core's page, read again on the same two
                            // signals. The Jitsi field is applied before dispatch returns, so the
                            // page read after it is what the core made of the text.
                            videoconferencing = remember(calendarVersion, settingsVersion) {
                                instance.videoconferencingPage()
                            },
                            onSetJitsiServer = {
                                instance.setPreference(PreferenceIntent.SetJitsiServer(it))
                                instance.videoconferencingPage()
                            },
                            onSetDefaultMeeting = { account, choice ->
                                instance.setPreference(PreferenceIntent.SetDefaultMeeting(account, choice))
                            },
                            // Reading, conversation grouping.
                            mode = mode,
                            onSetMode = { instance.dispatch(Intent.SetViewMode(it)) },
                            // Reading, the message background in a dark app; the same preference
                            // as the reading screen's sun/moon button.
                            onSetDarkMessages = { instance.setPreference(PreferenceIntent.SetDarkMessageBackground(it)) },
                            // Composing, quote style (+ the per-message opt-in), default send
                            // account, swipe actions.
                            quoteSettings = quotes,
                            onSetQuoteStyle = { instance.setPreference(PreferenceIntent.SetQuoteStyle(it)) },
                            onSetQuoteStylePerMessage = { instance.setPreference(PreferenceIntent.SetQuoteStylePerMessage(it)) },
                            sendAccount = sendAccount,
                            onSetDefaultSendAccount = { instance.setPreference(PreferenceIntent.SetDefaultSendAccount(it)) },
                            swipe = swipes,
                            onSetSwipeLeft = { instance.setPreference(PreferenceIntent.SetSwipeAction(SwipeDirection.LEFT, it)) },
                            onSetSwipeRight = { instance.setPreference(PreferenceIntent.SetSwipeAction(SwipeDirection.RIGHT, it)) },
                            // Signatures, the core's page. A change returns at once and the
                            // SIGNATURES signal redraws the page; nothing is echoed locally.
                            signatures = SignaturesUi(
                                page = signatures,
                                bodyHtml = { instance.signatureHtml(it) },
                                onChange = { instance.dispatch(Intent.Signature(it)) },
                                nameRefusal = ::signatureNameRefusal,
                                imageRefusal = { type, size -> signatureImageRefusal(type, size.toULong()) },
                            ),
                            // Privacy, withdraw (or belatedly give) the usage-statistics consent.
                            analyticsEnabled = analyticsConsent?.enabled == true,
                            onSetAnalytics = { instance.setPreference(PreferenceIntent.SetAnalyticsConsent(it)) },
                            analyticsPayloadPreview = { instance.analyticsPayloadPreview() },
                            // Accounts, the Allodia account, then per-account fetch depth + sync
                            // behaviour. The Allodia card is the core's (MainActivityAllodia.kt).
                            allodia = AllodiaSettings(available = allodiaSignInOffered(), card = allodiaCard),
                            onAllodia = { changeAllodia(it) },
                            onAllodiaManage = { manageAllodiaAccount() },
                            // The subscription on that account, as the core keeps it, and what
                            // buying from this build's shop keeps beside it
                            // (MainActivityAllodiaPurchases.kt).
                            allodiaSubscription = AllodiaSubscriptionUi(
                                card = allodiaSubscription,
                                store = allodiaStore,
                                actions = allodiaSubscriptionActions { changeAllodia(allodiaSignIn()) },
                            ),
                            // Accounts, what the person's other devices have to say, drawn above
                            // their own accounts because an offer becomes one of them.
                            allodiaDevices = allodiaDevices,
                            onAllodiaSetUp = { offer ->
                                settingsCategory = SettingsCategory.ACCOUNTS
                                setupStartEmail = offer.email
                                setupStartOffer = offer
                                addingAccount = true
                            },
                            // Diagnostics, the log viewer/share + debug-detail screen.
                            onOpenDiagnostics = { showingDiagnostics = true },
                            // Advanced, the core's page, read again on SETTINGS, which the
                            // reset raises as it starts and as it ends.
                            advanced = advanced,
                            onReset = { instance.dispatch(Intent.ResetDatabase) },
                            onBack = {
                                showingSettings = false
                                settingsCategory = null
                                accountSettings.page = null
                                accountSettings.notice = null
                            },
                            initialCategory = settingsCategory,
                            accountsUi = accountsSettingsUi(),
                        )
}

// Settings → Accounts' state and what each change on it asks of the core
// (MainActivityAccountSettings.kt).
private fun MainActivity.accountsSettingsUi() = AccountsSettingsUi(
    state = accountSettings,
    // Its notices open its page (docs/notices.md rule 6), and Settings opens with the first that
    // stands everywhere (rule 2).
    notices = notices.notices,
    noticeHandlers = noticeHandlers(),
    standardPort = { kind, security -> standardPort(kind, security).toInt() },
    endpointsGap = { id, endpoints -> app?.accountEndpointsGap(id, endpoints) },
    // Add account opens setup in place of Settings, and cancelling or finishing returns to Accounts.
    onAddAccount = {
        addError = null
        settingsCategory = SettingsCategory.ACCOUNTS
        addingAccount = true
    },
    onOpenPage = { id ->
        accountSettings.notice = null
        accountSettings.page = id
        // Its server is asked again which mailboxes the account's sign-in opens.
        id?.let { app?.accountPageOpened(it) }
        refreshAccountPage()
    },
    onChange = { intent -> changeAccount(intent) },
    onSignIn = { id, adding -> signInInBrowser(id, adding) },
    onSeparateSignIn = { id -> openPageSeparate(id) },
    onConnectSeparate = { username, password -> connectSeparate(accountSettings.separate, username, password) },
    onSeparateProviderSignIn = { signInSeparate(accountSettings.separate) },
    onReviewServerChange = { id -> reviewServerChange(id) },
)
