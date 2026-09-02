// One settings category's detail panel, the controls that live under it (docs/settings.md). Shared
// across every Apple form factor so the three chromes can't drift: the macOS sidebar (SettingsView),
// the iPad two-pane and the iPhone hub-and-spoke (SettingsHubView) all render THIS for the selected
// category. State lives in Rust (the `MailboxModel` mirrors the snapshots); this renders it and
// dispatches the setters. Kept in its own file so each stays under the 500-line limit.

import MailcalBindings
import SwiftUI

/// The detail for one category. `close` dismisses the whole settings surface (used after a
/// destructive reset). The reset confirmation lives here so it travels with the Advanced category
/// on every chrome.
struct SettingsCategoryDetail: View {
    var model: MailboxModel
    let category: SettingsCategory
    let close: () -> Void
    /// Removes an account by id. Passed in from the shell rather than calling `model.removeAccount`
    /// here, because removal is more than the model call: the shell also clears the scene's stored
    /// selection and the open message when they belong to the account going away. Two entry points
    /// (the sidebar's context menu and the button below) must not mean two behaviours, skipping
    /// that cleanup leaves a removed account's message sitting in the reading pane.
    let removeAccount: (String) -> Void

    @State private var confirmingReset = false
    /// The account a remove-confirmation is open for, or nil.
    @State private var accountToRemove: AccountSyncRow?

    var body: some View {
        switch category {
        case .general: generalDetail
        case .calendar: calendarDetail
        case .reading: readingDetail
        case .composing: composingDetail
        case .signatures: signaturesDetail
        case .notifications: notificationsDetail
        case .privacy: privacyDetail
        case .allodia: AllodiaAccountSettings(model: model)
        case .accounts: accountsDetail
        case .diagnostics: DiagnosticsSettingsView(model: model)
        case .advanced: advancedDetail
        case .about: AboutSettingsView()
        }
    }

    // MARK: General, language, appearance, time zone, and the clock mail AND calendar both read

    @ViewBuilder
    private var generalDetail: some View {
        VStack(alignment: .leading, spacing: 20) {
            settingsGroup(L10n.settings_language_heading(), L10n.settings_language_description()) {
                LanguagePicker()
            }
            settingsGroup(
                L10n.settings_appearance_heading(), L10n.settings_appearance_description()
            ) {
                AppearancePicker(model: model)
            }
            settingsGroup(L10n.tz_picker_title(), L10n.settings_timezone_description()) {
                TimeZonePicker(active: model.activeZone) { model.setTimeZone($0) }
            }
            // The 12/24-hour clock spans mail AND calendar, one app must not disagree with itself:
            // so it belongs under General rather than under Calendar. Last in the group on every
            // platform: Language → Appearance → Time zone → Time format (docs/settings.md).
            settingsGroup(
                L10n.settings_time_format_heading(), L10n.settings_time_format_description()
            ) {
                TimeFormatPicker(model: model)
            }
            // The permanent way back from the one-time offer, and the only way in for someone who
            // dismissed it. Drawn **only where the build can act on it**: a sandboxed Mac App
            // Store build cannot set the handler and iOS cannot appear in Default Apps without an
            // entitlement Apple grants by request, so the row is absent there rather than present
            // and inert (docs/os-integration.md).
            if DefaultMailApp.support != .unsupported {
                settingsGroup(
                    L10n.settings_default_mail_app_heading(),
                    L10n.settings_default_mail_app_description()
                ) {
                    DefaultMailAppRow(model: model)
                }
            }
        }
    }

    // MARK: Calendar, the week's first day, and how much of the day the grid shows

    @ViewBuilder
    private var calendarDetail: some View {
        VStack(alignment: .leading, spacing: 20) {
            settingsGroup(
                L10n.settings_week_start_heading(), L10n.settings_week_start_description()
            ) {
                WeekStartPicker(model: model)
            }
            settingsGroup(L10n.settings_horizon_heading(), L10n.settings_horizon_description()) {
                CalendarHorizonPicker(model: model)
            }
        }
    }

    // MARK: Reading, conversation grouping (list ↔ thread) + swipe actions

    @ViewBuilder
    private var readingDetail: some View {
        VStack(alignment: .leading, spacing: 20) {
            settingsGroup(L10n.settings_grouping_heading(), L10n.settings_grouping_description()) {
                Picker(L10n.settings_grouping_heading(), selection: groupingBinding) {
                    Text(L10n.settings_grouping_threaded()).tag(ViewMode.threaded)
                    Text(L10n.settings_grouping_flat()).tag(ViewMode.flat)
                }
                .radioPickerStyle()
                .labelsHidden()
            }
            settingsGroup(L10n.settings_swipe_heading(), L10n.settings_swipe_description()) {
                SwipeActionsPicker(model: model)
            }
        }
    }

    private var groupingBinding: Binding<ViewMode> {
        Binding(get: { model.mode }, set: { model.setMode($0) })
    }

    // MARK: Composing, default quote style + default send account

    @ViewBuilder
    private var composingDetail: some View {
        VStack(alignment: .leading, spacing: 20) {
            settingsGroup(L10n.quote_style_label(), L10n.settings_composing_description()) {
                QuoteStyleSettings(model: model)
            }
            settingsGroup(L10n.settings_send_account_heading(), L10n.settings_send_account_description()) {
                DefaultSendAccountPicker(model: model)
            }
        }
    }

    // MARK: Signatures, the reusable library, then which one each account starts with

    @ViewBuilder
    private var signaturesDetail: some View {
        VStack(alignment: .leading, spacing: 20) {
            // The library first: an account picker with nothing to pick says nothing, so a
            // first-time user needs to write a signature before the defaults below mean anything.
            settingsGroup(
                L10n.settings_signatures_library_heading(),
                L10n.settings_signatures_library_description()
            ) {
                SignatureLibraryView(model: model)
            }
            settingsGroup(
                L10n.settings_signatures_defaults_heading(),
                L10n.settings_signatures_defaults_description()
            ) {
                AccountSignatureDefaults(model: model)
            }
        }
    }

    // MARK: Notifications (mobile-only), the new-mail notification toggle

    #if os(iOS)
    /// The app-level "new-mail notifications" toggle. A client-side preference, the background sync
    /// still runs and advances the core's marks when off, so re-enabling never floods with a
    /// backlog. Copy comes from the shared catalog, like its Android twin (SettingsNotifications.kt).
    @ViewBuilder
    private var notificationsDetail: some View {
        settingsGroup(
            L10n.settings_notifications_heading(), L10n.settings_notifications_description()
        ) {
            Toggle(isOn: notificationsBinding) { EmptyView() }
                .labelsHidden()
                .frame(maxWidth: .infinity, alignment: .leading)
        }
    }

    private var notificationsBinding: Binding<Bool> {
        Binding(get: { NotificationPrefs.enabled }, set: { NotificationPrefs.enabled = $0 })
    }
    #else
    // macOS has no new-mail notifications yet, so this category is never displayed there
    // (SettingsCategory.displayed omits it); the branch exists only for switch exhaustiveness.
    private var notificationsDetail: some View { EmptyView() }
    #endif

    // MARK: Privacy, the usage-statistics opt-in, withdrawable in one click (GDPR Art. 7(3))

    @ViewBuilder
    private var privacyDetail: some View {
        settingsGroup(L10n.settings_analytics_heading(), L10n.settings_analytics_description()) {
            AnalyticsConsentControl(model: model)
        }
    }

    // MARK: Allodia account, not a mail account, so never among them
    // Only reachable in a build that carries the registration: `SettingsCategory.displayed` drops
    // the category otherwise, so this draws unconditionally.

    // MARK: Accounts, per-account fetch depth + sync behaviour, for mail accounts only

    /// The three positions an account can be shared in, and what the selected one means.
    ///
    /// A segmented picker rather than a switch and a button: the two questions underneath, is
    /// this account on my other devices, and does this device exchange changes about it, are not
    /// independent in any way somebody can act on, and splitting them produced a screen where
    /// turning the switch off changed nothing the person could see.
    ///
    /// One subtext, the selected position's: three at once is a paragraph nobody reads, and the
    /// one that matters is the one in force.
    @ViewBuilder
    private func accountSyncSection(
        _ accountId: String, _ mode: AllodiaAccountSyncMode
    ) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(L10n.settings_account_sync_heading()).font(.subheadline).bold()
            Picker(
                L10n.settings_account_sync_heading(),
                selection: Binding(
                    get: { mode },
                    set: { picked in
                        guard picked != mode else { return }
                        Task { await model.setAllodiaAccountSyncMode(accountId, picked) }
                    }
                )
            ) {
                Text(L10n.settings_account_sync_on()).tag(AllodiaAccountSyncMode.on)
                Text(L10n.settings_account_sync_paused()).tag(AllodiaAccountSyncMode.paused)
                Text(L10n.settings_account_sync_off()).tag(AllodiaAccountSyncMode.off)
            }
            .pickerStyle(.segmented)
            .labelsHidden()
            Text(syncModeHint(mode))
                .font(.caption)
                .foregroundStyle(.secondary)
                .fixedSize(horizontal: false, vertical: true)
        }
    }

    private func syncModeHint(_ mode: AllodiaAccountSyncMode) -> String {
        switch mode {
        case .on: L10n.settings_account_sync_on_hint()
        case .paused: L10n.settings_account_sync_paused_hint()
        case .off: L10n.settings_account_sync_off_hint()
        }
    }

    @ViewBuilder
    private var accountsDetail: some View {
        VStack(alignment: .leading, spacing: 20) {
            // What the person's other devices have to say, above their own accounts: an offer
            // becomes one of them.
            // Settings is a modal, and the setup sheet cannot present over it, so this asks for
            // it and closes, and `ContentView` opens it once this is off screen.
            AllodiaSyncSettings(model: model) { offer in
                model.setupStartEmail = offer.email
                model.setupStartOffer = offer
                model.addAccountWhenSettingsCloses = true
                close()
            }
            if let snapshot = model.syncSettings, !snapshot.accounts.isEmpty {
                ForEach(snapshot.accounts, id: \.accountId) { account in
                    accountSection(account, snapshot)
                }
            } else {
                Text(L10n.settings_accounts_empty())
                    .foregroundStyle(.secondary)
            }
        }
        // The second way to remove an account (the first is the sidebar's context menu). A
        // context menu is not a discoverable place to keep the only copy of "delete my
        // account", on a touch device it is a long press on a row that gives no sign it
        // holds one, which is why App Review asks where the control is.
        .alert(
            L10n.remove_account_title(),
            isPresented: Binding(
                get: { accountToRemove != nil },
                set: { if !$0 { accountToRemove = nil } }
            ),
            presenting: accountToRemove
        ) { account in
            Button(L10n.action_remove(), role: .destructive) { removeAccount(account.accountId) }
            Button(L10n.action_cancel(), role: .cancel) {}
        } message: { account in
            Text(L10n.remove_account_message(email: account.email))
        }
    }

    @ViewBuilder
    private func accountSection(_ account: AccountSyncRow, _ snapshot: SyncSettingsSnapshot) -> some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 14) {
                Text(account.email).font(.headline)

                // Where this one stands, and whether it travels. First in the card, because it
                // decides whether anything below it is anybody else's business. Absent in a build
                // with no Allodia sign-in, which draws nothing rather than a dead switch.
                if let mode = model.accountsSyncMode[account.accountId] {
                    accountSyncSection(account.accountId, mode)
                }

                // Fetch depth, how far back this account downloads mail (per-account).
                VStack(alignment: .leading, spacing: 4) {
                    Text(L10n.settings_sync_depth_heading()).font(.subheadline).bold()
                    Text(L10n.settings_sync_depth_description())
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Picker(L10n.settings_sync_depth_label(), selection: depthBinding(account)) {
                        ForEach(snapshot.syncDepths, id: \.self) { months in
                            Text(depthLabel(months)).tag(months)
                        }
                    }
                    .pickerStyle(.menu)
                    .labelsHidden()
                    .frame(maxWidth: 220, alignment: .leading)
                }

                // Message size, the largest message kept offline (per-account).
                VStack(alignment: .leading, spacing: 4) {
                    Text(L10n.settings_message_size_heading()).font(.subheadline).bold()
                    Text(L10n.settings_message_size_description())
                        .font(.caption)
                        .foregroundStyle(.secondary)
                    Picker(L10n.settings_message_size_label(), selection: sizeBinding(account)) {
                        ForEach(snapshot.messageSizeLimitsMb, id: \.self) { megabytes in
                            Text(sizeLabel(megabytes)).tag(megabytes)
                        }
                    }
                    .pickerStyle(.menu)
                    .labelsHidden()
                    .frame(maxWidth: 220, alignment: .leading)
                }

                Divider()

                // Sync behaviour, push (IMAP IDLE, when supported) vs. interval polling.
                if account.idleSupported {
                    Picker(L10n.settings_sync_strategy_label(), selection: strategyBinding(account)) {
                        Text(L10n.settings_sync_strategy_push()).tag(SyncStrategyKind.push)
                        Text(L10n.settings_sync_strategy_poll()).tag(SyncStrategyKind.poll)
                    }
                    .radioPickerStyle()
                } else {
                    Text(L10n.settings_sync_idle_unsupported())
                        .font(.callout)
                        .foregroundStyle(.secondary)
                }
                switch account.strategy {
                case .push: pushFolders(account)
                case .poll: pollInterval(account, snapshot)
                }

                Divider()

                // Last in the card, and the only destructive control in Settings: a remove sits
                // below the things you might actually adjust, not among them.
                Button(role: .destructive) {
                    accountToRemove = account
                } label: {
                    Label(L10n.action_remove_account(), systemImage: "trash")
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
    }

    @ViewBuilder
    private func pollInterval(_ account: AccountSyncRow, _ snapshot: SyncSettingsSnapshot) -> some View {
        Picker(L10n.settings_sync_interval_label(), selection: intervalBinding(account)) {
            ForEach(snapshot.pollIntervals, id: \.self) { minutes in
                Text(L10n.settings_sync_interval_minutes(count: Int(minutes))).tag(minutes)
            }
        }
        .frame(maxWidth: 260)
    }

    @ViewBuilder
    private func pushFolders(_ account: AccountSyncRow) -> some View {
        VStack(alignment: .leading, spacing: 6) {
            Text(L10n.settings_sync_folders_heading()).font(.subheadline).bold()
            Text(L10n.settings_sync_folders_note(count: Int(model.syncSettings?.maxPushFolders ?? 5)))
                .font(.caption)
                .foregroundStyle(.secondary)
            ForEach(account.folders, id: \.key) { folder in
                Toggle(
                    folderLabel(role: folder.role, name: folder.name),
                    isOn: folderBinding(account, folder)
                )
                    .disabled(!folder.subscribed && account.atPushLimit)
            }
        }
    }

    /// The label for a fetch-depth option: a month count, or "All time" for the `0` sentinel.
    private func depthLabel(_ months: UInt16) -> String {
        months == 0 ? L10n.sync_depth_all() : L10n.sync_depth_months(count: Int(months))
    }

    private func depthBinding(_ account: AccountSyncRow) -> Binding<UInt16> {
        Binding(
            get: { account.syncDepthMonths },
            set: { model.setAccountSyncDepth(account.accountId, $0) }
        )
    }

    private func sizeLabel(_ megabytes: UInt16) -> String {
        megabytes == 0
            ? L10n.message_size_unlimited()
            : L10n.message_size_megabytes(count: Int(megabytes))
    }

    private func sizeBinding(_ account: AccountSyncRow) -> Binding<UInt16> {
        Binding(
            get: { account.messageSizeLimitMb },
            set: { model.setAccountMessageSizeLimit(account.accountId, $0) }
        )
    }

    private func strategyBinding(_ account: AccountSyncRow) -> Binding<SyncStrategyKind> {
        Binding(get: { account.strategy }, set: { model.setSyncStrategy(account.accountId, $0) })
    }

    private func intervalBinding(_ account: AccountSyncRow) -> Binding<UInt16> {
        Binding(get: { account.pollIntervalMins }, set: { model.setPollInterval(account.accountId, $0) })
    }

    private func folderBinding(_ account: AccountSyncRow, _ folder: SyncFolderRow) -> Binding<Bool> {
        Binding(get: { folder.subscribed }, set: { model.setPushFolder(account.accountId, folder.key, $0) })
    }

    // MARK: Advanced, AI assistant access (desktop) + reset database (destructive)
    //
    // Both belong to this category's character rather than its old one-line description:
    // powerful, expert-facing, off by default, capable of damage (docs/settings.md row 8). The
    // MCP panel renders nothing on a platform with no endpoint, so iOS needs no `#if` here.

    @ViewBuilder
    private var advancedDetail: some View {
        VStack(alignment: .leading, spacing: 20) {
            McpSettingsView(model: model)
            resetGroup
        }
    }

    @ViewBuilder
    private var resetGroup: some View {
        settingsGroup(L10n.action_reset_database(), L10n.settings_advanced_reset_description()) {
            Button(L10n.action_reset_database(), role: .destructive) { confirmingReset = true }
        }
        // An `alert`, not a `confirmationDialog`: iPadOS presents the latter as a popover, and a
        // popover DROPS the `.cancel`-role button, so this read as one destructive button with no
        // way out. See the remove-account alert in Mailcal.swift for the full note.
        .alert(
            L10n.reset_title(),
            isPresented: $confirmingReset
        ) {
            Button(L10n.reset_confirm(), role: .destructive) {
                model.reset()
                close()
            }
            Button(L10n.action_cancel(), role: .cancel) {}
        } message: {
            Text(L10n.reset_message())
        }
    }

    // MARK: A labelled GroupBox section (heading + description + content)

    @ViewBuilder
    private func settingsGroup(
        _ heading: String,
        _ description: String,
        @ViewBuilder content: () -> some View
    ) -> some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                Text(heading).font(.headline)
                Text(description).font(.callout).foregroundStyle(.secondary)
                content().padding(.top, 2)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(6)
        }
    }
}
