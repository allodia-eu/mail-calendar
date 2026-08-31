// One Settings category's detail screen, split out of SettingsScreen.kt: the group cards for
// every category, in the same grouping as the macOS and Windows detail panels.
package eu.allodia.mailcal

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import uniffi.mailcal_bindings.AboutInfo
import uniffi.mailcal_bindings.AccountRow
import uniffi.mailcal_bindings.AllodiaAccountOffer
import uniffi.mailcal_bindings.AllodiaAccountSyncMode
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

// One category's screen: its group cards, in the same grouping as the macOS and Windows detail
// panels. The reset confirmation lives with the screen (the dialog outlives this scroll column).
@Composable
internal fun CategoryDetail(
    category: SettingsCategory,
    timeZone: TimeZoneSnapshot?,
    onSetTimeZone: (id: String) -> Unit,
    display: DisplaySettings,
    onSetTimeFormat: (TimeFormat) -> Unit,
    onSetAppearance: (Appearance) -> Unit,
    onSetWeekStart: (WeekStart) -> Unit,
    onSetVisibleHours: (Int) -> Unit,
    // Calendar, which calendar a new event is filed on. The rows carry `isDefault`, already
    // resolved by the core against what exists, so there is no fallback rule on this side.
    calendars: List<CalendarRow>,
    onSetDefaultCalendar: (account: String?, calendar: String?) -> Unit,
    mode: ViewMode,
    onSetMode: (ViewMode) -> Unit,
    quoteSettings: QuoteSettings,
    onSetQuoteStyle: (QuoteStyleKind) -> Unit,
    onSetQuoteStylePerMessage: (Boolean) -> Unit,
    accounts: List<AccountRow>,
    defaultSendAccount: String?,
    onSetDefaultSendAccount: (String?) -> Unit,
    swipe: SwipeSettings,
    onSetSwipeLeft: (SwipeActionKind) -> Unit,
    onSetSwipeRight: (SwipeActionKind) -> Unit,
    signatures: SignaturesSnapshot?,
    signatureHtml: (String) -> String?,
    onCreateSignature: (name: String, bodyHtml: String, bodyPlain: String) -> Unit,
    onUpdateSignature: (id: String, name: String, bodyHtml: String, bodyPlain: String) -> Unit,
    onDeleteSignature: (String) -> Unit,
    onSetAccountSignature: (account: String, slot: SignatureSlotKind, signature: String?) -> Unit,
    about: AboutInfo,
    analyticsEnabled: Boolean,
    onSetAnalytics: (Boolean) -> Unit,
    analyticsPayloadPreview: () -> String,
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
    onRequestReset: () -> Unit,
) {
    val ctx = LocalContext.current
    when (category) {
        // General, language, appearance, time zone, and the clock mail AND calendar both read.
        SettingsCategory.GENERAL -> {
            SettingsGroupCard(L10n.settings_language_heading(ctx), L10n.settings_language_description(ctx)) {
                LanguageSettingsCard()
            }
            Spacer(modifier = Modifier.height(8.dp))
            SettingsGroupCard(
                L10n.settings_appearance_heading(ctx),
                L10n.settings_appearance_description(ctx),
            ) {
                AppearanceSettingsRows(display = display, onSetAppearance = onSetAppearance)
            }
            Spacer(modifier = Modifier.height(8.dp))
            SettingsGroupCard(L10n.tz_picker_title(ctx), L10n.settings_timezone_description(ctx)) {
                TimeZoneSettingsRow(timeZone = timeZone, onSelect = onSetTimeZone)
            }
            Spacer(modifier = Modifier.height(8.dp))
            SettingsGroupCard(
                L10n.settings_time_format_heading(ctx),
                L10n.settings_time_format_description(ctx),
            ) {
                TimeFormatSettingsRows(display = display, onSetTimeFormat = onSetTimeFormat)
            }
        }

        // Calendar, the week's first day, and how much of the day the grid shows.
        SettingsCategory.CALENDAR -> {
            SettingsGroupCard(
                L10n.settings_week_start_heading(ctx),
                L10n.settings_week_start_description(ctx),
            ) {
                WeekStartSettingsRows(display = display, onSetWeekStart = onSetWeekStart)
            }
            Spacer(modifier = Modifier.height(8.dp))
            SettingsGroupCard(
                L10n.settings_horizon_heading(ctx),
                L10n.settings_horizon_description(ctx),
            ) {
                CalendarHorizonSettingsRows(display = display, onSetVisibleHours = onSetVisibleHours)
            }
            Spacer(modifier = Modifier.height(8.dp))
            SettingsGroupCard(
                L10n.settings_default_calendar_heading(ctx),
                L10n.settings_default_calendar_description(ctx),
            ) {
                DefaultCalendarSettingsRows(
                    calendars = calendars,
                    onSetDefaultCalendar = onSetDefaultCalendar,
                )
            }
        }

        // Reading, conversation grouping (list <-> thread) and the message-row swipe gestures.
        SettingsCategory.READING -> {
            SettingsGroupCard(L10n.settings_grouping_heading(ctx), L10n.settings_grouping_description(ctx)) {
                StrategyRow(
                    label = L10n.settings_grouping_threaded(ctx),
                    selected = mode == ViewMode.THREADED,
                ) { onSetMode(ViewMode.THREADED) }
                StrategyRow(
                    label = L10n.settings_grouping_flat(ctx),
                    selected = mode == ViewMode.FLAT,
                ) { onSetMode(ViewMode.FLAT) }
            }
            Spacer(modifier = Modifier.height(8.dp))
            SettingsGroupCard(L10n.settings_swipe_heading(ctx), L10n.settings_swipe_description(ctx)) {
                SwipeActionsCard(swipe, onSetSwipeLeft, onSetSwipeRight)
            }
        }

        // Composing, default reply/forward quote style + which account new mail is sent from.
        SettingsCategory.COMPOSING -> {
            SettingsGroupCard(L10n.quote_style_label(ctx), L10n.settings_composing_description(ctx)) {
                QuoteStyleCard(
                    quoteStyle = quoteSettings.style,
                    perMessage = quoteSettings.perMessage,
                    onSet = onSetQuoteStyle,
                    onSetPerMessage = onSetQuoteStylePerMessage,
                )
            }
            Spacer(modifier = Modifier.height(8.dp))
            SettingsGroupCard(
                L10n.settings_send_account_heading(ctx),
                L10n.settings_send_account_description(ctx),
            ) {
                DefaultSendAccountCard(accounts, defaultSendAccount, onSetDefaultSendAccount)
            }
        }

        // Signatures, the library first, then the per-account defaults: an account picker with
        // nothing to pick says nothing, so a first-time user has to write one before the defaults
        // mean anything (docs/signatures.md).
        SettingsCategory.SIGNATURES -> {
            SettingsGroupCard(
                L10n.settings_signatures_library_heading(ctx),
                L10n.settings_signatures_library_description(ctx),
            ) {
                SignatureLibraryCard(
                    signatures = signatures?.signatures.orEmpty(),
                    bodyHtmlFor = signatureHtml,
                    onCreate = onCreateSignature,
                    onUpdate = onUpdateSignature,
                    onDelete = onDeleteSignature,
                )
            }
            Spacer(modifier = Modifier.height(8.dp))
            SettingsGroupCard(
                L10n.settings_signatures_defaults_heading(ctx),
                L10n.settings_signatures_defaults_description(ctx),
            ) {
                AccountSignatureDefaultsCard(
                    accounts = signatures?.accounts.orEmpty(),
                    signatures = signatures?.signatures.orEmpty(),
                    onSet = onSetAccountSignature,
                )
            }
        }

        // Notifications, the new-mail toggle + the battery-exemption card (SettingsNotifications.kt).
        SettingsCategory.NOTIFICATIONS -> NotificationsSettings()

        // Privacy, the usage-statistics opt-in the welcome screen asked about, withdrawable
        // here in a single tap. GDPR Art. 7(3): withdrawal must be as easy as giving. Turning
        // it off deletes the install id locally and asks the backend to erase what it holds.
        SettingsCategory.PRIVACY -> {
            SettingsGroupCard(
                L10n.settings_analytics_heading(ctx),
                L10n.settings_analytics_description(ctx),
            ) {
                AnalyticsCard(analyticsEnabled, onSetAnalytics, analyticsPayloadPreview)
            }
        }

        // The account with Allodia, which is not a mail account: no mailbox, no switcher entry,
        // and its token cannot touch mail. The category is only reachable in a build that carries
        // the registration, so this draws unconditionally.
        SettingsCategory.ALLODIA -> {
            AllodiaAccountCard(
                allodia,
                onAllodiaSignIn,
                onAllodiaCreate,
                onAllodiaManage,
                onAllodiaSignOut,
            )
        }

        // Accounts, per-account fetch depth + sync behaviour, for mail accounts only, under
        // whatever the person's other devices have to say about the list itself.
        SettingsCategory.ACCOUNTS -> {
            AllodiaSyncCard(allodiaSync, onAllodiaSetUp, onAllodiaKeepLocal, onAllodiaSignIn)
            val syncAccounts = settings?.accounts.orEmpty()
            if (syncAccounts.isEmpty()) {
                Text(
                    L10n.settings_accounts_empty(ctx),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    modifier = Modifier.padding(vertical = 8.dp),
                )
            } else {
                syncAccounts.forEach { account ->
                    Card(modifier = Modifier.fillMaxWidth()) {
                        Column(modifier = Modifier.fillMaxWidth().padding(16.dp)) {
                            AccountSyncCard(
                                account = account,
                                settings = settings!!,
                                onSetSyncDepth = onSetSyncDepth,
                                onSetMessageSize = onSetMessageSize,
                                onSetStrategy = onSetStrategy,
                                onSetPollInterval = onSetPollInterval,
                                onSetPushFolder = onSetPushFolder,
                                syncMode = accountsSyncMode[account.accountId],
                                onSetSyncMode = onSetAccountSyncMode,
                            )
                        }
                    }
                    Spacer(modifier = Modifier.height(8.dp))
                }
            }
        }

        // Advanced, reset the local cache (destructive; confirmed by the dialog in SettingsScreen).
        SettingsCategory.ADVANCED -> {
            SettingsGroupCard(L10n.action_reset_database(ctx), L10n.settings_advanced_reset_description(ctx)) {
                TextButton(
                    onClick = onRequestReset,
                    colors = ButtonDefaults.textButtonColors(contentColor = MaterialTheme.colorScheme.error),
                ) {
                    Text(L10n.action_reset_database(ctx))
                }
            }
        }

        // Diagnostics has no inline detail, its hub row opens the full-screen DiagnosticsScreen
        // (DiagnosticsScreen.kt) directly, so this branch is never reached.
        SettingsCategory.DIAGNOSTICS -> Unit

        // About, the release, the support forum, and what the app is built on (SettingsAbout.kt).
        SettingsCategory.ABOUT -> AboutSection(about)
    }
}
