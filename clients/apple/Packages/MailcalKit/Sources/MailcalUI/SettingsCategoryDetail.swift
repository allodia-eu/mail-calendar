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
    /// that cleanup leaves a removed account's message sitting in the reading pane. Answers why
    /// the stored sign-in could not be erased, or `nil`.
    let removeAccount: (String) -> String?

    @State private var confirmingReset = false

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
        case .accounts:
            AccountsSettingsView(model: model, close: close, removeAccount: removeAccount)
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

    // MARK: Reading, conversation grouping (list ↔ thread), swipe actions, message background

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
            settingsGroup(
                L10n.settings_dark_messages_heading(), L10n.settings_dark_messages_description()
            ) {
                Toggle(L10n.settings_dark_messages_toggle(), isOn: darkMessagesBinding)
            }
        }
    }

    /// The same preference as the reading pane's sun/moon button (docs/reading-dark-mode.md).
    private var darkMessagesBinding: Binding<Bool> {
        Binding(
            get: { model.displaySettings.darkMessageBackground },
            set: { model.setDarkMessageBackground($0) }
        )
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

    // MARK: Notifications, the new-mail notification toggle

    /// The app-level "new-mail notifications" toggle. A client-side preference, the scan still runs
    /// and advances the core's marks when off, so re-enabling never floods with a backlog. Copy
    /// comes from the shared catalog, like its Android twin (SettingsNotifications.kt).
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

    private func settingsGroup(
        _ heading: String,
        _ description: String,
        @ViewBuilder content: @escaping () -> some View
    ) -> some View {
        SettingsGroup(heading: heading, description: description, content: content)
    }
}
