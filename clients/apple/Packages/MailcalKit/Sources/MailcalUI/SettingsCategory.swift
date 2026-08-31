// The settings taxonomy, the same categories, in the same order, under the same names as the
// Android hub and the Windows source-list (docs/settings.md), so a support answer like
// "Settings → Reading" is true on every platform. Shared across all Apple form factors: macOS
// renders it as a sidebar beside a detail panel, iPad as a two-pane split, iPhone as a
// hub-and-spoke (SettingsView / SettingsHubView). The per-category detail is SettingsCategoryDetail.

import MailcalBindings
import SwiftUI

/// A top-level settings category, one sidebar/source-list row, or one iPhone hub row. Grouped by
/// function (mirroring Outlook) so there is room to grow: Composing gains auto-replies, Accounts
/// gains per-account identity, etc. Declaration order is the display order everywhere.
enum SettingsCategory: String, CaseIterable, Identifiable {
    case allodia, general, calendar, reading, composing, signatures, notifications, privacy,
         accounts, advanced, diagnostics, about

    var id: String { rawValue }

    /// The categories to show on this platform, in taxonomy order. Notifications is mobile-only:
    /// the desktops have no new-mail notifications yet (docs/background-sync.md known gap), so macOS
    /// leaves that slot out (it keeps its position for when a desktop gains the feature).
    ///
    /// Allodia goes for a different reason and at runtime: a build carrying no registration has no
    /// Allodia sign-in at all, and the whole category goes rather than its contents, a row that
    /// opens an empty pane reads as a broken pane, and it is what every build from source shows.
    static var displayed: [SettingsCategory] {
        var shown = allCases
        #if os(macOS)
        shown.removeAll { $0 == .notifications }
        #endif
        if !allodiaSignInAvailable() {
            shown.removeAll { $0 == .allodia }
        }
        return shown
    }

    var title: String {
        switch self {
        // The category and the card inside it are the same thing named twice otherwise, and
        // the product's own name belongs in one string.
        case .allodia: return L10n.settings_allodia_heading()
        case .general: return L10n.settings_category_general()
        case .calendar: return L10n.settings_category_calendar()
        case .reading: return L10n.settings_category_reading()
        case .composing: return L10n.settings_category_composing()
        case .signatures: return L10n.settings_category_signatures()
        case .notifications: return L10n.settings_category_notifications()
        case .privacy: return L10n.settings_category_privacy()
        case .accounts: return L10n.settings_category_accounts()
        case .diagnostics: return L10n.settings_category_diagnostics()
        case .advanced: return L10n.settings_category_advanced()
        case .about: return L10n.settings_category_about()
        }
    }

    /// One plain line saying what's inside, the iPhone hub must answer "where would I find X?"
    /// without the user opening every category (the desktops show the detail beside the list, so
    /// they don't render it, but it uses the same catalog keys as the Android hub).
    var summary: String {
        switch self {
        case .allodia: return L10n.settings_category_allodia_summary()
        case .general: return L10n.settings_category_general_summary()
        case .calendar: return L10n.settings_category_calendar_summary()
        case .reading: return L10n.settings_category_reading_summary()
        case .composing: return L10n.settings_category_composing_summary()
        case .signatures: return L10n.settings_category_signatures_summary()
        case .notifications: return L10n.settings_category_notifications_summary()
        case .privacy: return L10n.settings_category_privacy_summary()
        case .accounts: return L10n.settings_category_accounts_summary()
        case .diagnostics: return L10n.settings_category_diagnostics_summary()
        case .advanced: return L10n.settings_category_advanced_summary()
        case .about: return L10n.settings_category_about_summary()
        }
    }

    /// An SF Symbol for the row (the native icon set; the brand's Lucide is for web/design assets,
    /// not the platform chrome). Matched by meaning to the Android/Windows sets (docs/settings.md).
    var icon: String {
        switch self {
        // The person's own account takes the person glyph; the mail accounts below take the
        // tray, which is what they are. Two rows sharing an icon defeats the only thing a
        // sidebar's icons are for, which is finding a row without reading it.
        case .allodia: return "person.crop.circle"
        case .general: return "gearshape"
        case .calendar: return "calendar"
        case .reading: return "envelope.open"
        case .composing: return "square.and.pencil"
        case .signatures: return "signature"
        case .notifications: return "bell"
        case .privacy: return "hand.raised"
        case .accounts: return "tray.2"
        case .diagnostics: return "stethoscope"
        case .advanced: return "wrench.and.screwdriver"
        case .about: return "info.circle"
        }
    }
}
