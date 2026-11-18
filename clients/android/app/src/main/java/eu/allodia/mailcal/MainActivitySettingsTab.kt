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
import uniffi.mailcal_bindings.standardPort

/**
 * Changes one preference. It takes effect in the core before this returns and raises SETTINGS,
 * whose pull redraws every screen that shows it, so no caller keeps a copy of the value.
 */
internal fun MailcalApp.setPreference(intent: PreferenceIntent) = dispatch(Intent.Preferences(intent))

// The unified Settings screen, swapped in over the mailbox.
@Composable
internal fun MainActivity.SettingsTabContent(instance: MailcalApp) {
    // All three are pulled in the same step that sets the core, so none is null here.
    val display = displaySettings ?: return
    val quotes = quoteSettings ?: return
    val swipes = swipeSettings ?: return
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
                            // a slot drawn on the grid wears. Read straight from the core (an
                            // in-memory cache read) rather than held in state: the signal below
                            // recomposes Settings, and the rows carry the resolved `isDefault`.
                            calendars = remember(calendarVersion) { instance.calendars() },
                            onSetDefaultCalendar = { account, calendar ->
                                instance.setPreference(PreferenceIntent.SetDefaultCalendar(account, calendar))
                            },
                            // Videoconferencing; once a server is stored, the calendar signal
                            // redraws the editor's choices.
                            jitsiServer = { instance.jitsiServer() },
                            onSetJitsiServer = { instance.setJitsiServer(it) },
                            meetingDefaults = remember(calendarVersion) { instance.meetingDefaults() },
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
                            accounts = accounts,
                            defaultSendAccount = defaultSendAccount,
                            onSetDefaultSendAccount = { instance.setPreference(PreferenceIntent.SetDefaultSendAccount(it)) },
                            swipe = swipes,
                            onSetSwipeLeft = { instance.setPreference(PreferenceIntent.SetSwipeAction(SwipeDirection.LEFT, it)) },
                            onSetSwipeRight = { instance.setPreference(PreferenceIntent.SetSwipeAction(SwipeDirection.RIGHT, it)) },
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
                            // Advanced, reset the local cache.
                            onReset = { instance.reset() },
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
