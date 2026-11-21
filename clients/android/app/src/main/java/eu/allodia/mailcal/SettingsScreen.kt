// The unified Settings screen (Android): the mobile twin of the macOS SettingsView and the
// Windows SettingsDialog. The same categories, in the same order, under the same names
// (docs/settings.md), but where the desktops show a sidebar beside a detail panel, a phone gets
// a hub-and-spoke: a list of category rows (icon, name, one-line summary of what's inside), each
// opening its own screen. The back arrow and system back both step detail → hub → mailbox. State
// lives in the Rust core (its page records and persisted preferences); this renders it and
// dispatches the setters, which re-signal SETTINGS. Swapped in over the mailbox (like the reading
// view). The category taxonomy is SettingsCategory.kt and the Notifications category is
// SettingsNotifications.kt, so each file stays under the 500-line limit.
package eu.allodia.mailcal

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.systemBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AboutInfo
import uniffi.mailcal_bindings.AdvancedPage
import uniffi.mailcal_bindings.AllodiaAccountOffer
import uniffi.mailcal_bindings.AllodiaDevices
import uniffi.mailcal_bindings.AllodiaIntent
import uniffi.mailcal_bindings.Appearance
import uniffi.mailcal_bindings.DisplaySettings
import uniffi.mailcal_bindings.NewEventCalendars
import uniffi.mailcal_bindings.QuoteSettings
import uniffi.mailcal_bindings.QuoteStyleKind
import uniffi.mailcal_bindings.SendAccountSetting
import uniffi.mailcal_bindings.SwipeActionKind
import uniffi.mailcal_bindings.SwipeSettings
import uniffi.mailcal_bindings.TimeFormat
import uniffi.mailcal_bindings.TimeZoneSnapshot
import uniffi.mailcal_bindings.VideoconferencingPage
import uniffi.mailcal_bindings.ViewMode
import uniffi.mailcal_bindings.WeekStart

@Composable
internal fun SettingsScreen(
    // General
    timeZone: TimeZoneSnapshot?,
    onSetTimeZone: (id: String) -> Unit,
    // General + Calendar, the display preferences the core owns (week start, 12/24-hour clock,
    // default horizon). The clock spans mail AND calendar, so it sits under General.
    display: DisplaySettings,
    onSetTimeFormat: (TimeFormat) -> Unit,
    onSetAppearance: (Appearance) -> Unit,
    onSetWeekStart: (WeekStart) -> Unit,
    onSetVisibleHours: (Int) -> Unit,
    // Calendar, which calendar a new event is filed on: the core's `newEventCalendars`, whose rows
    // carry `isDefault` already resolved against what exists.
    newEventCalendars: NewEventCalendars,
    onSetDefaultCalendar: (account: String?, calendar: String?) -> Unit,
    // Videoconferencing, the core's page: the Jitsi server, the text it last refused and each
    // account's default meeting. Sending the field answers the page as the core left it.
    videoconferencing: VideoconferencingPage,
    onSetJitsiServer: (String) -> VideoconferencingPage,
    onSetDefaultMeeting: (account: String, choice: String?) -> Unit,
    // Reading
    mode: ViewMode,
    onSetMode: (ViewMode) -> Unit,
    onSetDarkMessages: (Boolean) -> Unit,
    // Composing
    quoteSettings: QuoteSettings,
    onSetQuoteStyle: (QuoteStyleKind) -> Unit,
    onSetQuoteStylePerMessage: (Boolean) -> Unit,
    sendAccount: SendAccountSetting,
    onSetDefaultSendAccount: (String?) -> Unit,
    swipe: SwipeSettings,
    onSetSwipeLeft: (SwipeActionKind) -> Unit,
    onSetSwipeRight: (SwipeActionKind) -> Unit,
    // Signatures, the library (write once, reuse on any account) and each account's two slots.
    signatures: SignaturesUi,
    // About, read once by the caller: it is a call into the cdylib (SettingsAbout.kt).
    about: AboutInfo,
    // Privacy
    analyticsEnabled: Boolean,
    onSetAnalytics: (Boolean) -> Unit,
    analyticsPayloadPreview: () -> String,
    // Accounts, the Allodia account first, then the per-account mail settings. The Allodia
    // card is the core's, and a change to it is sent as an AllodiaIntent.
    allodia: AllodiaSettings,
    onAllodia: (AllodiaIntent) -> Unit,
    onAllodiaManage: () -> Unit,
    // The subscription, drawn under the account card while the core draws a section. Held by the
    // activity: buying leaves for Play's sheet or a browser.
    allodiaSubscription: AllodiaSubscriptionUi,
    allodiaDevices: AllodiaDevices?,
    onAllodiaSetUp: (AllodiaAccountOffer) -> Unit,
    // Diagnostics, opens the Diagnostics screen (log viewer/share, debug toggle).
    onOpenDiagnostics: () -> Unit,
    // Advanced, the core's page; [onReset] starts the reset once the person confirmed it, and
    // Settings stays open while the page draws it running and then what it came to.
    advanced: AdvancedPage,
    onReset: () -> Unit,
    onBack: () -> Unit,
    /**
     * The category to open straight into, or `null` for the hub.
     *
     * A shortcut from elsewhere in the app ("Calendar settings" in the calendar's own menu) lands on
     * the category it named rather than on the hub with the user hunting for it. Back still unwinds
     * through the hub, which is where a phone user expects the second press to go.
     */
    initialCategory: SettingsCategory? = null,
    // Accounts: every account from the core's snapshot, and the page open (SettingsAccounts.kt).
    accountsUi: AccountsSettingsUi = remember { AccountsSettingsUi(AccountsUiState()) },
    // Writing style and the own endpoint, null while the writing assistant is not offered.
    writingStyle: WritingStyleUi? = null,
    aiEndpoint: AiEndpointUi? = null,
) {
    val ctx = LocalContext.current
    var open by remember(initialCategory) { mutableStateOf(initialCategory) }
    var confirmingReset by remember { mutableStateOf(false) }
    val shown = SettingsCategory.shown(allodia.available, writingStyle?.page != null)
    // A category the core stops drawing while it is open (Writing style, once the writing assistant
    // is no longer offered) is left for the hub rather than kept as a title over nothing, and stays
    // left if it comes back.
    val current = open?.takeIf { it in shown }
    if (current != open) SideEffect { open = current }
    // An account's page sits under the Accounts category, so back from it returns to the list.
    val accountPage = accountsUi.openEntry().takeIf { current == SettingsCategory.ACCOUNTS }
    val stepBack = {
        when {
            accountPage != null -> accountsUi.onOpenPage(null)
            current != null -> open = null
            else -> onBack()
        }
    }

    // System back mirrors the back arrow: a page returns to its list, a detail screen to the hub,
    // the hub to the mailbox. Without this, back would leave Settings (or the app) from anywhere.
    BackHandler { stepBack() }

    // Outside the Scaffold, so the system bars are this screen's own problem (see WelcomeScreen).
    Column(modifier = Modifier.fillMaxSize().systemBarsPadding().imePadding().padding(16.dp)) {
        if (current == null) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(L10n.settings_title(ctx), style = MaterialTheme.typography.titleLarge)
                TextButton(onClick = onBack) { Text(L10n.action_done(ctx)) }
            }
            SettingsNotice(accountsUi.notices, page = null, accountsUi.noticeHandlers)
            Column(modifier = Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {
                Spacer(modifier = Modifier.height(4.dp))
                shown.forEach { category ->
                    // Diagnostics is a full-screen log viewer swapped in at the activity level
                    // (DiagnosticsScreen.kt), not an inline CategoryDetail, so its hub row opens
                    // that screen rather than a detail pane.
                    CategoryRow(category) {
                        if (category == SettingsCategory.DIAGNOSTICS) onOpenDiagnostics() else open = category
                    }
                }
                Spacer(modifier = Modifier.height(24.dp))
            }
        } else {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = stepBack) {
                    Icon(
                        painter = painterResource(R.drawable.ic_arrow_back),
                        contentDescription = L10n.a11y_back(ctx),
                    )
                }
                Text(
                    accountPage?.address ?: current.title(ctx),
                    style = MaterialTheme.typography.titleLarge,
                    maxLines = 1,
                    overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
                )
            }
            SettingsNotice(accountsUi.notices, accountPage?.id, accountsUi.noticeHandlers)
            // Each page starts at its top: an account's page is not the list scrolled further.
            val detailScroll = androidx.compose.runtime.key(current, accountPage?.id) { rememberScrollState() }
            Column(modifier = Modifier.fillMaxWidth().verticalScroll(detailScroll)) {
                Spacer(modifier = Modifier.height(8.dp))
                CategoryDetail(
                    category = current,
                    timeZone = timeZone,
                    onSetTimeZone = onSetTimeZone,
                    display = display,
                    onSetTimeFormat = onSetTimeFormat,
                    onSetAppearance = onSetAppearance,
                    onSetWeekStart = onSetWeekStart,
                    onSetVisibleHours = onSetVisibleHours,
                    newEventCalendars = newEventCalendars,
                    onSetDefaultCalendar = onSetDefaultCalendar,
                    videoconferencing = videoconferencing,
                    onSetJitsiServer = onSetJitsiServer,
                    onSetDefaultMeeting = onSetDefaultMeeting,
                    mode = mode,
                    onSetMode = onSetMode,
                    onSetDarkMessages = onSetDarkMessages,
                    quoteSettings = quoteSettings,
                    onSetQuoteStyle = onSetQuoteStyle,
                    onSetQuoteStylePerMessage = onSetQuoteStylePerMessage,
                    sendAccount = sendAccount,
                    onSetDefaultSendAccount = onSetDefaultSendAccount,
                    swipe = swipe,
                    onSetSwipeLeft = onSetSwipeLeft,
                    onSetSwipeRight = onSetSwipeRight,
                    signatures = signatures,
                    about = about,
                    analyticsEnabled = analyticsEnabled,
                    onSetAnalytics = onSetAnalytics,
                    analyticsPayloadPreview = analyticsPayloadPreview,
                    allodia = allodia,
                    onAllodia = onAllodia,
                    onAllodiaManage = onAllodiaManage,
                    allodiaSubscription = allodiaSubscription,
                    allodiaDevices = allodiaDevices,
                    onAllodiaSetUp = onAllodiaSetUp,
                    advanced = advanced,
                    onRequestReset = { confirmingReset = true },
                    accountsUi = accountsUi,
                    onOpenAccount = { id ->
                        accountsUi.onOpenPage(id)
                        open = SettingsCategory.ACCOUNTS
                    },
                    writingStyle = writingStyle,
                    aiEndpoint = aiEndpoint,
                    onOpenCategory = { open = it },
                )
                Spacer(modifier = Modifier.height(24.dp))
            }
        }
    }

    if (confirmingReset) {
        AlertDialog(
            onDismissRequest = { confirmingReset = false },
            title = { Text(L10n.reset_title(ctx)) },
            text = { Text(L10n.reset_message(ctx)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        confirmingReset = false
                        onReset()
                    },
                    colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.error),
                ) {
                    Text(L10n.reset_confirm(ctx))
                }
            },
            dismissButton = {
                TextButton(onClick = { confirmingReset = false }) { Text(L10n.action_cancel(ctx)) }
            },
        )
    }
}

// One hub row: the category's icon, its name, a one-line summary of what's inside, and a chevron
// saying "this opens a screen", so a first-time user can find a setting without opening
// everything.
@Composable
private fun CategoryRow(category: SettingsCategory, onOpen: () -> Unit) {
    val ctx = LocalContext.current
    Row(
        modifier = Modifier.fillMaxWidth().clickable(onClick = onOpen).padding(vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Icon(
            painter = painterResource(category.icon),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.primary,
        )
        Spacer(modifier = Modifier.width(16.dp))
        Column(modifier = Modifier.weight(1f)) {
            Text(category.title(ctx), style = MaterialTheme.typography.titleMedium)
            Text(
                category.summary(ctx),
                style = MaterialTheme.typography.bodyMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Icon(
            painter = painterResource(R.drawable.ic_keyboard_arrow_right),
            contentDescription = null,
            tint = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}


// A single settings group: a card with a heading, a one-line description where it has one, and its
// control(s). Mirrors the macOS SettingsView's settingsGroup GroupBox. Internal: the Diagnostics
// screen (DiagnosticsScreen.kt) shares it so its cards read as part of the same settings surface.
@Composable
internal fun SettingsGroupCard(
    heading: String,
    description: String?,
    content: @Composable () -> Unit,
) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.fillMaxWidth().padding(16.dp)) {
            Text(heading, style = MaterialTheme.typography.titleMedium)
            if (description != null) {
                Text(
                    description,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(top = 2.dp, bottom = 8.dp),
                )
            } else {
                Spacer(modifier = Modifier.height(8.dp))
            }
            content()
        }
    }
}
