// The unified Settings screen (Android): the mobile twin of the macOS SettingsView and the
// Windows SettingsDialog. The same categories, in the same order, under the same names
// (docs/settings.md), but where the desktops show a sidebar beside a detail panel, a phone gets
// a hub-and-spoke: a list of category rows (icon, name, one-line summary of what's inside), each
// opening its own screen. The back arrow and system back both step detail → hub → mailbox. State
// lives in the Rust core (the SyncSettingsSnapshot / persisted preferences); this renders it and
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
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.AllodiaAccountSyncMode
import uniffi.mailcal_bindings.AllodiaAccountOffer
import uniffi.mailcal_bindings.Appearance
import uniffi.mailcal_bindings.CalendarRow
import uniffi.mailcal_bindings.DisplaySettings
import uniffi.mailcal_bindings.QuoteSettings
import uniffi.mailcal_bindings.QuoteStyleKind
import uniffi.mailcal_bindings.SignatureSlotKind
import uniffi.mailcal_bindings.SignaturesSnapshot
import uniffi.mailcal_bindings.SwipeActionKind
import uniffi.mailcal_bindings.SwipeSettings
import uniffi.mailcal_bindings.SyncSettingsSnapshot
import uniffi.mailcal_bindings.SyncStrategyKind
import uniffi.mailcal_bindings.TimeFormat
import uniffi.mailcal_bindings.TimeZoneSnapshot
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
    // Calendar, which calendar a new event is filed on. The rows carry `isDefault`, already
    // resolved by the core against what exists, so there is no fallback rule on this side.
    calendars: List<CalendarRow>,
    onSetDefaultCalendar: (account: String?, calendar: String?) -> Unit,
    // Reading
    mode: ViewMode,
    onSetMode: (ViewMode) -> Unit,
    // Composing
    quoteSettings: QuoteSettings,
    onSetQuoteStyle: (QuoteStyleKind) -> Unit,
    onSetQuoteStylePerMessage: (Boolean) -> Unit,
    accounts: List<AccountRow>,
    defaultSendAccount: String?,
    onSetDefaultSendAccount: (String?) -> Unit,
    swipe: SwipeSettings,
    onSetSwipeLeft: (SwipeActionKind) -> Unit,
    onSetSwipeRight: (SwipeActionKind) -> Unit,
    // Signatures, the library (write once, reuse on any account) and each account's two slots.
    // Null while the first snapshot is in flight, which reads as an empty library.
    signatures: SignaturesSnapshot?,
    signatureHtml: (String) -> String?,
    onCreateSignature: (name: String, bodyHtml: String, bodyPlain: String) -> Unit,
    onUpdateSignature: (id: String, name: String, bodyHtml: String, bodyPlain: String) -> Unit,
    onDeleteSignature: (String) -> Unit,
    onSetAccountSignature: (account: String, slot: SignatureSlotKind, signature: String?) -> Unit,
    // About, read once by the caller: it is a call into the cdylib (SettingsAbout.kt).
    about: AboutInfo,
    // Privacy
    analyticsEnabled: Boolean,
    onSetAnalytics: (Boolean) -> Unit,
    analyticsPayloadPreview: () -> String,
    // Accounts, the Allodia account first, then the per-account mail settings. The Allodia
    // state is the activity's, not this screen's: a sign-in leaves for the browser and comes back
    // through onNewIntent, so nothing this composable remembered would still be there.
    allodia: AllodiaSettings,
    onAllodiaSignIn: () -> Unit,
    onAllodiaCreate: () -> Unit,
    onAllodiaManage: () -> Unit,
    onAllodiaSignOut: () -> Unit,
    allodiaSync: AllodiaSyncState,
    onAllodiaSetUp: (AllodiaAccountOffer) -> Unit,
    onAllodiaKeepLocal: (String) -> Unit,
    accountsSyncMode: Map<String, AllodiaAccountSyncMode>,
    onSetAccountSyncMode: (account: String, mode: AllodiaAccountSyncMode) -> Unit,
    settings: SyncSettingsSnapshot?,
    onSetSyncDepth: (account: String, months: UShort) -> Unit,
    onSetMessageSize: (account: String, megabytes: UShort) -> Unit,
    onSetStrategy: (account: String, strategy: SyncStrategyKind) -> Unit,
    onSetPollInterval: (account: String, minutes: UShort) -> Unit,
    onSetPushFolder: (account: String, folder: String, subscribed: Boolean) -> Unit,
    // Diagnostics, opens the Diagnostics screen (log viewer/share, debug toggle).
    onOpenDiagnostics: () -> Unit,
    // Advanced
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
) {
    val ctx = LocalContext.current
    var open by remember(initialCategory) { mutableStateOf(initialCategory) }
    var confirmingReset by remember { mutableStateOf(false) }

    // System back mirrors the back arrow: a detail screen returns to the hub, the hub to the
    // mailbox. Without this, back would leave Settings (or the app) from anywhere inside it.
    BackHandler {
        if (open != null) open = null else onBack()
    }

    // Outside the Scaffold, so the system bars are this screen's own problem (see WelcomeScreen).
    Column(modifier = Modifier.fillMaxSize().systemBarsPadding().padding(16.dp)) {
        val current = open
        if (current == null) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(L10n.settings_title(ctx), style = MaterialTheme.typography.titleLarge)
                TextButton(onClick = onBack) { Text(L10n.action_done(ctx)) }
            }
            Column(modifier = Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {
                Spacer(modifier = Modifier.height(4.dp))
                SettingsCategory.shown(allodia.available).forEach { category ->
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
                IconButton(onClick = { open = null }) {
                    Icon(
                        painter = painterResource(R.drawable.ic_arrow_back),
                        contentDescription = L10n.a11y_back(ctx),
                    )
                }
                Text(current.title(ctx), style = MaterialTheme.typography.titleLarge)
            }
            Column(modifier = Modifier.fillMaxWidth().verticalScroll(rememberScrollState())) {
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
                    calendars = calendars,
                    onSetDefaultCalendar = onSetDefaultCalendar,
                    mode = mode,
                    onSetMode = onSetMode,
                    quoteSettings = quoteSettings,
                    onSetQuoteStyle = onSetQuoteStyle,
                    onSetQuoteStylePerMessage = onSetQuoteStylePerMessage,
                    accounts = accounts,
                    defaultSendAccount = defaultSendAccount,
                    onSetDefaultSendAccount = onSetDefaultSendAccount,
                    swipe = swipe,
                    onSetSwipeLeft = onSetSwipeLeft,
                    onSetSwipeRight = onSetSwipeRight,
                    signatures = signatures,
                    signatureHtml = signatureHtml,
                    onCreateSignature = onCreateSignature,
                    onUpdateSignature = onUpdateSignature,
                    onDeleteSignature = onDeleteSignature,
                    onSetAccountSignature = onSetAccountSignature,
                    about = about,
                    analyticsEnabled = analyticsEnabled,
                    onSetAnalytics = onSetAnalytics,
                    analyticsPayloadPreview = analyticsPayloadPreview,
                    allodia = allodia,
                    onAllodiaSignIn = onAllodiaSignIn,
                    onAllodiaCreate = onAllodiaCreate,
                    onAllodiaManage = onAllodiaManage,
                    onAllodiaSignOut = onAllodiaSignOut,
                    allodiaSync = allodiaSync,
                    onAllodiaSetUp = onAllodiaSetUp,
                    onAllodiaKeepLocal = onAllodiaKeepLocal,
                    accountsSyncMode = accountsSyncMode,
                    onSetAccountSyncMode = onSetAccountSyncMode,
                    settings = settings,
                    onSetSyncDepth = onSetSyncDepth,
                    onSetMessageSize = onSetMessageSize,
                    onSetStrategy = onSetStrategy,
                    onSetPollInterval = onSetPollInterval,
                    onSetPushFolder = onSetPushFolder,
                    onRequestReset = { confirmingReset = true },
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
                        onBack()
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


// A single settings group: a card with a heading, a one-line description, and its control(s).
// Mirrors the macOS SettingsView's settingsGroup GroupBox. Internal: the Diagnostics screen
// (DiagnosticsScreen.kt) shares it so its cards read as part of the same settings surface.
@Composable
internal fun SettingsGroupCard(
    heading: String,
    description: String,
    content: @Composable () -> Unit,
) {
    Card(modifier = Modifier.fillMaxWidth()) {
        Column(modifier = Modifier.fillMaxWidth().padding(16.dp)) {
            Text(heading, style = MaterialTheme.typography.titleMedium)
            Text(
                description,
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(top = 2.dp, bottom = 8.dp),
            )
            content()
        }
    }
}
